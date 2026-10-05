//! Public native/CLI contract for obstacle-aware card connectors.
use diagram_render_rs::{
    DiagramFormat, Document, OutputFormat, RenderOptions, render_document, render_source,
};
use std::io::Write;
use std::process::{Command, Stdio};

const SKIP: &str = "A\nB\nC\nD\nE\nA -> C: skips B\n";

#[test]
fn source_and_ast_replay_keep_repaired_routes_and_labels_deterministic() {
    for source in [
        SKIP,
        "A\nB\nC\nD\nE\nA -> C: first\nC -> A: reverse\nA -> C: third",
        "A -> A: a deliberately wide self loop label",
        "A\nB\nC\nD\nE\nA -> E: 跨行连接\n",
    ] {
        let options = RenderOptions::default();
        let native = render_source(DiagramFormat::D2, source, OutputFormat::Svg, &options).unwrap();
        let document = diagram_ast_parser::parse(DiagramFormat::D2, source).unwrap();
        let replay: Document =
            serde_json::from_str(&serde_json::to_string(&document).unwrap()).unwrap();
        let replayed = render_document(&replay, OutputFormat::Svg, &options).unwrap();
        assert_eq!(native.svg, replayed.svg);
        assert_eq!(
            native.svg,
            render_source(DiagramFormat::D2, source, OutputFormat::Svg, &options)
                .unwrap()
                .svg
        );
        assert!(native.warnings.is_empty());
    }
    let svg = render_source(
        DiagramFormat::D2,
        SKIP,
        OutputFormat::Svg,
        &RenderOptions::default(),
    )
    .unwrap()
    .svg;
    assert!(svg.contains("skips B"));
    assert!(!svg.contains("points=\"300.00,120.50 742.00,120.50\""));
}

fn large_source() -> String {
    let mut source = (0..257).map(|i| format!("n{i}\n")).collect::<String>();
    source.push_str("n0 -> n2");
    source
}

#[test]
fn routing_failure_preserves_existing_missing_and_stdout_outputs_for_svg_and_png() {
    let root = tempfile::tempdir().unwrap();
    let existing = root.path().join("existing.svg");
    let missing = root.path().join("missing/output.svg");
    std::fs::write(&existing, b"unchanged").unwrap();
    for format in ["svg", "png"] {
        for output in [None, Some(&existing), Some(&missing)] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_diagram-render-rs"));
            command.args(["-f", "d2", "-T", format, "--quiet"]);
            if let Some(path) = output {
                command.arg("-o").arg(path);
            }
            let mut child = command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(large_source().as_bytes())
                .unwrap();
            let result = child.wait_with_output().unwrap();
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
            assert!(
                String::from_utf8_lossy(&result.stderr).contains("card routing budget exceeded")
            );
            assert_eq!(std::fs::read(&existing).unwrap(), b"unchanged");
            assert!(!missing.parent().unwrap().exists());
        }
    }
}

#[test]
fn unconnected_large_diagrams_keep_their_existing_behavior() {
    let source = large_source().replace("n0 -> n2", "");
    assert!(
        render_source(
            DiagramFormat::D2,
            &source,
            OutputFormat::Svg,
            &RenderOptions::default()
        )
        .is_ok()
    );
}

#[test]
fn no_fit_failure_names_the_edge_and_also_preserves_output() {
    let source = "A\nB\nC\nD\nA -> A: This label is intentionally quite long\nA -> A: This label is intentionally quite long\n";
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("output.svg");
    std::fs::write(&output, b"keep no-fit output").unwrap();
    for format in ["svg", "png"] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_diagram-render-rs"))
            .args(["-f", "d2", "-T", format, "--quiet", "-o"])
            .arg(&output)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(source.as_bytes())
            .unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(error.contains("card connector 2 (A -> A)"), "{error}");
        assert!(error.contains("no clear"));
        assert_eq!(std::fs::read(&output).unwrap(), b"keep no-fit output");
    }
}
