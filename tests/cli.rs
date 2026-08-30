use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn cli_renders_png_selected_by_output_extension() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let output = std::env::temp_dir().join(format!(
        "diagram-render-rs-cli-{}-{nonce}.png",
        std::process::id()
    ));
    let status = Command::new(env!("CARGO_BIN_EXE_diagram-render-rs"))
        .args([
            "examples/inputs/schema.dbml",
            "--format",
            "dbml",
            "--output",
        ])
        .arg(&output)
        .arg("--quiet")
        .status()
        .expect("run CLI");
    assert!(status.success());
    let png = fs::read(&output).expect("read PNG");
    assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    fs::remove_file(output).expect("remove temporary PNG");
}
