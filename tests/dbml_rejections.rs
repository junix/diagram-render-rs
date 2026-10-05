//! The CLI must reject lossy DBML before emitting or replacing an SVG.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

struct OutputDirectory(PathBuf);

impl OutputDirectory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "diagram-render-dbml-rejections-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create isolated output directory");
        Self(path)
    }
}

impl Drop for OutputDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn render(source: &str, destination: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_diagram-render-rs"));
    command.args(["-", "--format", "dbml", "--output-format", "svg", "--quiet"]);
    if let Some(path) = destination {
        command.arg("--output").arg(path);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run renderer");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    child.wait_with_output().expect("collect renderer output")
}

#[test]
fn malformed_dbml_never_emits_creates_or_overwrites_svg() {
    for (source, diagnostic) in [
        (
            "Table valid { id int }\nRef { a.id > b.id; c.id > d.id }",
            "Ref block must contain exactly one relationship",
        ),
        (
            "Table users { id int { ignored text } }",
            "column must not have a braced body",
        ),
        (
            "Table users as u extra { id int }",
            "unexpected token after table alias",
        ),
    ] {
        let directory = OutputDirectory::new();
        let missing = directory.0.join("new.svg");
        let existing = directory.0.join("existing.svg");
        let sentinel = b"<svg><!-- existing output must survive --></svg>\n";
        fs::write(&existing, sentinel).unwrap();
        for destination in [None, Some(missing.as_path()), Some(existing.as_path())] {
            let output = render(source, destination);
            assert_eq!(output.status.code(), Some(1), "{source}");
            assert!(output.stdout.is_empty(), "partial SVG emitted for {source}");
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert!(stderr.starts_with("diagram-render-rs: "), "{stderr}");
            assert!(stderr.contains(diagnostic), "{stderr}");
            assert!(!missing.exists(), "invalid source created an SVG");
            assert_eq!(fs::read(&existing).unwrap(), sentinel);
        }
    }
}

fn valid_svg(source: &str) -> Vec<u8> {
    let output = render(source, None);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(String::from_utf8_lossy(&output.stdout).contains("<svg"));
    output.stdout
}

#[test]
fn valid_single_ref_forms_keep_identical_svg() {
    let tables = "Table a { id int [pk] }\nTable b { id int [pk] }\n";
    for (inline, block) in [
        ("Ref: a.id > b.id", "Ref { a.id > b.id }"),
        ("Ref link: a.id > b.id", "Ref link { a.id > b.id }"),
    ] {
        assert_eq!(
            valid_svg(&format!("{tables}{inline}")),
            valid_svg(&format!("{tables}{block}"))
        );
    }
}

#[test]
fn valid_alias_empty_settings_and_array_types_keep_identical_svg() {
    for alias in ["u", "\"user alias\""] {
        let columns = "id int [pk]\n tags text[]\n values int[][] [not null]";
        let without = format!("Table users as {alias} {{\n {columns}\n}}");
        let empty = format!("Table users as {alias} [] {{\n {columns}\n}}");
        let expected = valid_svg(&without);
        assert_eq!(valid_svg(&empty), expected);
        let svg = String::from_utf8(expected).unwrap();
        assert!(svg.contains("text[]"));
        assert!(svg.contains("int[][]"));
    }
}
