//! Verified immutable explicit inputs and filesystem boundary checks.
use super::provider_receipt::{InputReceipt, digest};
use super::provider_render::error;
use anyhow::Result;
use serde::Deserialize;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};

pub(crate) const MAX_INPUT_BYTES: usize = 64 * 1024;
const MAX_PINS_BYTES: usize = 4 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pins {
    input: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Identity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(not(unix))]
    handle: same_file::Handle,
}
impl Identity {
    pub(crate) fn read(path: &Path) -> Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(path)?;
            Ok(Self {
                device: metadata.dev(),
                inode: metadata.ino(),
            })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {
                handle: same_file::Handle::from_path(path)?,
            })
        }
    }
}

pub(crate) struct Input {
    original: PathBuf,
    canonical: PathBuf,
    parent: PathBuf,
    directory: PathBuf,
    directory_identity: Identity,
    identity: Identity,
    pub(crate) bytes: Vec<u8>,
    sha256: String,
}
impl Input {
    pub(crate) fn read(
        path: &Path,
        resource_pins: &str,
        output: &Path,
        receipt: &Path,
    ) -> Result<Self> {
        for p in [path, output, receipt] {
            local_path(p)?;
        }
        if resource_pins.len() > MAX_PINS_BYTES {
            return Err(error("resource_pins exceeds 4 KiB"));
        }
        super::provider_json::validate(resource_pins.as_bytes())?;
        let pins: Pins = serde_json::from_str(resource_pins)?;
        if pins.input.len() != 64
            || !pins
                .input
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(error(
                "resource_pins requires lowercase 64-character SHA256",
            ));
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let directory = fs::canonicalize(&parent)?;
        let directory_identity = Identity::read(&directory)?;
        super::provider_paths::check_destinations(path, Some(output), Some(receipt))?;
        let canonical = canonical_resource(path, &directory)?;
        let identity = Identity::read(&canonical)?;
        let bytes = read_bounded_regular(path, MAX_INPUT_BYTES)?;
        if canonical_resource(path, &directory)? != canonical || Identity::read(path)? != identity {
            return Err(error("input identity changed during read"));
        }
        let sha256 = digest(&bytes);
        if sha256 != pins.input {
            return Err(error("input raw byte SHA256 differs from resource_pins"));
        }
        let input = Self {
            original: path.into(),
            canonical,
            parent,
            directory,
            directory_identity,
            identity,
            bytes,
            sha256,
        };
        input.check_destinations(output, receipt)?;
        Ok(input)
    }
    fn check_destinations(&self, output: &Path, receipt: &Path) -> Result<()> {
        super::provider_paths::check_destinations(&self.original, Some(output), Some(receipt))?;
        // Hub also checks its selected executable. Native execution protects its
        // own executable even when invoked without Hub.
        let executable = std::env::current_exe()?;
        super::provider_paths::check_destinations(&executable, Some(output), Some(receipt))?;
        super::provider_paths::check_distinct_sources(&self.original, &executable)?;
        Ok(())
    }
    pub(crate) fn recheck(&self, output: &Path, receipt: &Path) -> Result<()> {
        if fs::canonicalize(&self.parent)? != self.directory
            || Identity::read(&self.directory)? != self.directory_identity
            || canonical_resource(&self.original, &self.directory)? != self.canonical
            || Identity::read(&self.original)? != self.identity
        {
            return Err(error(
                "input or bundle directory identity changed before publication",
            ));
        }
        let bytes = read_bounded_regular(&self.original, MAX_INPUT_BYTES)?;
        if digest(&bytes) != self.sha256 || bytes != self.bytes {
            return Err(error("input bytes changed before publication"));
        }
        self.check_destinations(output, receipt)
    }
    pub(crate) fn receipt(&self) -> InputReceipt {
        InputReceipt {
            role: "input".into(),
            sha256: self.sha256.clone(),
            bytes: self.bytes.len() as u64,
        }
    }
}
pub(crate) fn local_path(path: &Path) -> Result<()> {
    let value = path.to_str().ok_or_else(|| error("paths must be UTF-8"))?;
    if value.is_empty()
        || value.len() > 4096
        || value == "-"
        || value.contains('\0')
        || value.contains("://")
    {
        return Err(error(
            "require explicit local paths of 1..4096 UTF-8 bytes; no URL or stdin",
        ));
    }
    Ok(())
}
fn canonical_resource(path: &Path, directory: &Path) -> Result<PathBuf> {
    let resolved = fs::canonicalize(path)?;
    if resolved.parent() != Some(directory) || !fs::metadata(&resolved)?.is_file() {
        return Err(error(
            "input must be a regular file directly inside its original bundle directory",
        ));
    }
    Ok(resolved)
}
pub(crate) fn read_bounded_regular(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let file = open_regular(path)?;
    if file.metadata()?.len() > limit as u64 {
        return Err(error(format!("file exceeds {limit} byte budget")));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(error(format!("file exceeds {limit} byte budget")));
    }
    Ok(bytes)
}
fn open_regular(path: &Path) -> Result<File> {
    let resolved = fs::canonicalize(path)?;
    if !fs::metadata(&resolved)?.is_file() {
        return Err(error("input must be a regular file"));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(resolved)?;
    if !file.metadata()?.is_file() {
        return Err(error("input must be a regular file"));
    }
    Ok(file)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn input(dir: &Path) -> Input {
        let path = dir.join("input.json");
        fs::write(&path, b"{}").unwrap();
        Input::read(
            &path,
            &serde_json::to_string(&BTreeMap::from([("input", digest(b"{}"))])).unwrap(),
            &dir.join("svg"),
            &dir.join("receipt"),
        )
        .unwrap()
    }
    #[test]
    fn recheck_detects_bytes_file_and_directory_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let value = input(dir.path());
        fs::write(&value.original, b"{ }").unwrap();
        assert!(
            value
                .recheck(&dir.path().join("svg"), &dir.path().join("receipt"))
                .is_err()
        );
        fs::write(&value.original, b"{}").unwrap();
        value
            .recheck(&dir.path().join("svg"), &dir.path().join("receipt"))
            .unwrap();
        fs::rename(&value.original, dir.path().join("old")).unwrap();
        fs::write(&value.original, b"{}").unwrap();
        assert!(
            value
                .recheck(&dir.path().join("svg"), &dir.path().join("receipt"))
                .is_err()
        );
        let sub = dir.path().join("bundle");
        fs::create_dir(&sub).unwrap();
        let value = input(&sub);
        fs::rename(&sub, dir.path().join("prior-bundle")).unwrap();
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("input.json"), b"{}").unwrap();
        assert!(
            value
                .recheck(&dir.path().join("svg"), &dir.path().join("receipt"))
                .is_err()
        );
    }
}
