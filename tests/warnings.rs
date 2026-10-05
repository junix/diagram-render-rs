//! Report known degradation without changing geometry; optionally refuse publication.
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use diagram_render_rs::{
    DiagramFormat, Document, OutputFormat, RenderOptions, render_document, render_source,
};

const INLINE: &str = "Table users { id int [pk] }\nTable posts { user_id int [ref: > users.id] }";
const EXPLICIT: &str =
    "Table users { id int [pk] }\nTable posts { user_id int }\nRef: posts.user_id > users.id";

fn warning(count: usize) -> String {
    format!(
        "DBML contains {count} inline column ref setting(s) in tables/partials; \
         these settings are not rendered as connectors. Use explicit Ref declarations for connectors."
    )
}

#[test]
fn dbml_counts_authored_column_ref_settings_only() {
    for (source, count) in [
        ("Table users { id int [pk] }", 0),
        (EXPLICIT, 0),
        (INLINE, 1),
        ("Table users { id int [ReF: > users.id] }", 1),
        (
            "Table users { id int [ref: > users.id, ref: < users.id] }",
            2,
        ),
        (
            "TablePartial unused { id int [ref: > users.id] }\nTable users { id int }",
            1,
        ),
        (
            "TablePartial shared { id int [ref: > users.id] }\nTable users {\n ~shared\n}\nTable posts {\n ~shared\n}",
            1,
        ),
        (
            "Table users { id int [ref: > users.id] }\nTablePartial shared { id int [REF: < users.id] }",
            2,
        ),
        (
            "Table users [ref: ignored] { id int [note: 'ref: > users.id', reference: ignored] }",
            0,
        ),
        ("// ref: > users.id\nTable users { id int }", 0),
    ] {
        let options = RenderOptions::default();
        let from_source = render_source(DiagramFormat::Dbml, source, OutputFormat::Svg, &options)
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        let document = diagram_ast_parser::parse(DiagramFormat::Dbml, source).unwrap();
        let json = serde_json::to_string(&document).unwrap();
        let round_trip: Document = serde_json::from_str(&json).unwrap();
        let from_ast = render_document(&round_trip, OutputFormat::Svg, &options).unwrap();
        let expected = if count == 0 {
            vec![]
        } else {
            vec![warning(count)]
        };
        assert_eq!(from_source.warnings, expected, "{source}");
        assert_eq!(from_ast.warnings, expected, "{source}");
        assert_eq!(from_source.svg, from_ast.svg, "{source}");
        assert_eq!(
            from_source.warnings,
            render_document(&round_trip, OutputFormat::Svg, &options)
                .unwrap()
                .warnings
        );
    }
}

#[test]
fn inline_warning_does_not_change_svg_or_duplicate_explicit_connectors() {
    let options = RenderOptions::default();
    let plain = INLINE.replace(" [ref: > users.id]", "");
    let render = |source: &str| {
        render_source(DiagramFormat::Dbml, source, OutputFormat::Svg, &options).unwrap()
    };
    assert_eq!(render(INLINE).svg, render(&plain).svg);
    let both = format!("{INLINE}\nRef: posts.user_id > users.id");
    assert_eq!(render(&both).svg, render(EXPLICIT).svg);
    assert_eq!(render(&both).warnings, vec![warning(1)]);
}

fn cli(source: &str, args: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_diagram-render-rs"))
        .arg("-")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run CLI");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "diagram-render-warnings-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cli_default_warns_and_quiet_only_suppresses_nonfatal_diagnostics() {
    let normal = cli(INLINE, &["-f", "dbml"]);
    assert!(normal.status.success());
    assert_eq!(
        String::from_utf8(normal.stderr).unwrap(),
        format!("warning: {}\n", warning(1))
    );
    let quiet = cli(INLINE, &["-f", "dbml", "--quiet"]);
    assert!(quiet.status.success());
    assert!(quiet.stderr.is_empty());
    assert_eq!(normal.stdout, quiet.stdout);
    assert!(String::from_utf8(normal.stdout).unwrap().contains("<svg"));
}

#[test]
fn deny_warnings_preserves_stdout_and_existing_or_missing_files_even_when_quiet() {
    let directory = Directory::new();
    let existing = directory.0.join("existing.svg");
    let missing = directory.0.join("missing-parent/output.svg");
    let sentinel = b"keep this output unchanged";
    fs::write(&existing, sentinel).unwrap();
    for quiet in [false, true] {
        for format in ["svg", "png"] {
            for path in [None, Some(&existing), Some(&missing)] {
                let mut args = vec!["-f", "dbml", "--deny-warnings", "-T", format];
                if quiet {
                    args.push("--quiet");
                }
                if let Some(path) = path {
                    args.extend(["-o", path.to_str().unwrap()]);
                }
                let output = cli(INLINE, &args);
                assert_eq!(output.status.code(), Some(1));
                assert!(output.stdout.is_empty());
                let stderr = String::from_utf8(output.stderr).unwrap();
                assert!(
                    stderr.contains("--deny-warnings refused output: 1 renderer warning(s)"),
                    "{stderr}"
                );
                assert!(stderr.contains(&warning(1)), "{stderr}");
                assert!(!stderr.contains("wrote "));
                assert_eq!(fs::read(&existing).unwrap(), sentinel);
                assert!(!missing.parent().unwrap().exists());
            }
        }
    }
}

#[test]
fn deny_warnings_also_gates_ast_json_and_existing_d2_warnings() {
    let document = diagram_ast_parser::parse(DiagramFormat::Dbml, INLINE).unwrap();
    let json = serde_json::to_string(&document).unwrap();
    let ast = cli(&json, &["--ast-json", "--deny-warnings", "--quiet"]);
    assert_eq!(ast.status.code(), Some(1));
    assert!(ast.stdout.is_empty());
    assert!(String::from_utf8(ast.stderr).unwrap().contains(&warning(1)));

    let source = "...@other.d2\na: A";
    let normal = cli(source, &["-f", "d2"]);
    assert!(
        normal.status.success(),
        "{}",
        String::from_utf8_lossy(&normal.stderr)
    );
    assert!(String::from_utf8_lossy(&normal.stderr).contains("D2 import"));
    let denied = cli(source, &["-f", "d2", "--deny-warnings", "--quiet"]);
    assert_eq!(denied.status.code(), Some(1));
    assert!(denied.stdout.is_empty());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("D2 import"));
}

#[test]
fn deny_warnings_allows_warning_free_stdout_and_file_output() {
    let directory = Directory::new();
    let path = directory.0.join("schema.svg");
    let normal = cli(EXPLICIT, &["-f", "dbml", "--quiet"]);
    let strict = cli(EXPLICIT, &["-f", "dbml", "--deny-warnings", "--quiet"]);
    assert!(strict.status.success());
    assert!(strict.stderr.is_empty());
    assert_eq!(strict.stdout, normal.stdout);
    let file = cli(
        EXPLICIT,
        &[
            "-f",
            "dbml",
            "--deny-warnings",
            "--quiet",
            "-o",
            path.to_str().unwrap(),
        ],
    );
    assert!(file.status.success());
    assert!(file.stdout.is_empty());
    assert_eq!(fs::read(path).unwrap(), normal.stdout);
}
