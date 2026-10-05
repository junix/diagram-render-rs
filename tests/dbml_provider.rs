//! Invoke the actual provider and independently parsed native CLI.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tempfile::TempDir;
const SAMPLE: &str = include_str!("../examples/dbml-authored/users-posts.json");
const SOURCE: &str = include_str!("../examples/dbml-authored/users-posts.dbml");
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn call(dir: &Path, input: &[u8], extra: &[&str]) -> Output {
    fs::write(dir.join("input.json"), input).unwrap();
    Command::new(env!("CARGO_BIN_EXE_plot-provider-dbml"))
        .arg("render-svg")
        .arg("--input")
        .arg(dir.join("input.json"))
        .arg("--resource-pins")
        .arg(json!({"input":hash(input)}).to_string())
        .arg("--output")
        .arg(dir.join("figure.svg"))
        .arg("--receipt")
        .arg(dir.join("receipt.json"))
        .args(extra)
        .output()
        .unwrap()
}
fn checked(dir: &Path, input: &[u8], extra: &[&str]) -> Vec<u8> {
    let output = call(dir, input, extra);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    fs::read(dir.join("figure.svg")).unwrap()
}
fn rejects(bytes: &[u8], extra: &[&str]) {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("figure.svg"), b"prior svg").unwrap();
    fs::write(dir.path().join("receipt.json"), b"prior receipt").unwrap();
    let result = call(dir.path(), bytes, extra);
    assert!(
        !result.status.success(),
        "unexpected acceptance: {}",
        String::from_utf8_lossy(bytes)
    );
    assert!(result.stdout.is_empty());
    assert_eq!(
        fs::read(dir.path().join("figure.svg")).unwrap(),
        b"prior svg"
    );
    assert_eq!(
        fs::read(dir.path().join("receipt.json")).unwrap(),
        b"prior receipt"
    );
    assert!(fs::read_dir(dir.path()).unwrap().all(|e| {
        !e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".dbml-provider-")
    }));
}
fn sample() -> Value {
    serde_json::from_str(SAMPLE).unwrap()
}
#[test]
fn native_parity_all_themes_and_backgrounds_and_typed_receipt() {
    let dir = TempDir::new().unwrap();
    let source = dir.path().join("source.dbml");
    fs::write(&source, SOURCE).unwrap();
    let themes = std::iter::once("light")
        .chain(std::iter::once("dark"))
        .chain(diagram_theme::Theme::NAMES);
    for theme in themes {
        for painted in [false, true] {
            let mut args = vec!["--theme", theme];
            if painted {
                args.extend(["--background", "#123aEF"]);
            }
            let svg = checked(dir.path(), SAMPLE.as_bytes(), &args);
            let native = Command::new(env!("CARGO_BIN_EXE_diagram-render-rs"))
                .arg(&source)
                .args(["--format", "dbml", "--deny-warnings", "--quiet"])
                .args(&args)
                .output()
                .unwrap();
            assert!(
                native.status.success(),
                "{}",
                String::from_utf8_lossy(&native.stderr)
            );
            assert_eq!(svg, native.stdout, "{theme}, painted={painted}");
            assert_eq!(svg, checked(dir.path(), SAMPLE.as_bytes(), &args));
            let receipt: Value =
                serde_json::from_slice(&fs::read(dir.path().join("receipt.json")).unwrap())
                    .unwrap();
            assert_eq!(
                receipt["artifact_receipt"]["inputs"],
                json!([{"role":"input","sha256":hash(SAMPLE.as_bytes()),"bytes":SAMPLE.len()}])
            );
            assert_eq!(
                receipt["artifact_receipt"]["primary"],
                json!({"artifact_id":"figure","role":"primary","argument":"output","kind":"svg","sha256":hash(&svg),"bytes":svg.len()})
            );
            assert_eq!(receipt["options"]["theme"], theme);
            assert_eq!(receipt["counts"], json!({"tables":2,"columns":5,"refs":1}));
            assert_eq!(receipt["warnings"], json!([]));
            let serialized = receipt.to_string();
            assert!(!serialized.contains(dir.path().to_str().unwrap()));
            assert!(!serialized.contains("/tmp/"));
        }
    }
}
#[test]
fn authored_semantic_changes_change_native_output() {
    let dir = TempDir::new().unwrap();
    let original = checked(dir.path(), SAMPLE.as_bytes(), &[]);
    for mutate in [0, 1, 2, 3] {
        let mut value = sample();
        match mutate {
            0 => value["tables"][0]["name"] = json!("Users"),
            1 => value["tables"][1]["columns"][2]["data_type"] = json!("varchar"),
            2 => value["refs"][0]["cardinality"] = json!("one-to-many"),
            _ => value["refs"] = json!([]),
        }
        if mutate == 0 {
            value["refs"][0]["to"]["table"] = json!("Users");
        }
        assert_ne!(
            original,
            checked(dir.path(), value.to_string().as_bytes(), &[])
        );
    }
}
#[test]
fn closed_objects_and_values_refuse_unknown_missing_null_or_duplicate_fields() {
    for pointer in [
        "",
        "/tables/0",
        "/tables/0/columns/0",
        "/refs/0",
        "/refs/0/from",
    ] {
        let value = sample();
        let object = value.pointer(pointer).unwrap().as_object().unwrap();
        let fields: Vec<_> = object.keys().cloned().collect();
        let mut unknown = value.clone();
        unknown
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unrepresented".into(), json!(true));
        rejects(unknown.to_string().as_bytes(), &[]);
        for field in fields {
            let mut missing = value.clone();
            missing
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(&field);
            rejects(missing.to_string().as_bytes(), &[]);
            let mut null = value.clone();
            null.pointer_mut(pointer).unwrap()[&field] = Value::Null;
            rejects(null.to_string().as_bytes(), &[]);
        }
    }
    for raw in [
        SAMPLE.replacen(
            "\"schema_version\":",
            "\"schema_version\":\"dbml.authored/v1\",\"schema_version\":",
            1,
        ),
        SAMPLE.replacen(
            "\"name\": \"users\"",
            "\"name\":\"users\",\"\\u006eame\":\"users\"",
            1,
        ),
        format!("{SAMPLE} {{}}"),
        "[".repeat(17) + &"]".repeat(17),
    ] {
        rejects(raw.as_bytes(), &[]);
    }
    rejects(b"\xff", &[]);
    rejects(br#"{"schema_version":"\ud800"}"#, &[]);
    rejects(&vec![b' '; 65537], &[]);
}
#[test]
fn profile_limits_and_unrepresented_semantics_fail_closed() {
    type Mutation = Box<dyn Fn(&mut Value)>;
    let mutations: Vec<Mutation> = vec![
        Box::new(|v| v["schema_version"] = json!("dbml.authored/v2")),
        Box::new(|v| v["tables"] = json!([])),
        Box::new(|v| {
            let t = v["tables"][0].clone();
            v["tables"].as_array_mut().unwrap().push(t);
        }),
        Box::new(|v| v["tables"][1]["name"] = json!("users")),
        Box::new(|v| v["tables"][0]["columns"] = json!([])),
        Box::new(|v| {
            v["tables"][0]["columns"] = json!(
                (0..17)
                    .map(|i| json!({"name":format!("c{i}"),"data_type":"int","flags":[]}))
                    .collect::<Vec<_>>()
            );
        }),
        Box::new(|v| v["tables"][0]["columns"][1]["name"] = json!("id")),
        Box::new(|v| v["tables"][0]["columns"][0]["flags"] = json!(["pk", "pk"])),
        Box::new(|v| v["tables"][0]["columns"][0]["flags"] = json!(["ref"])),
        Box::new(|v| v["refs"][0]["from"]["table"] = json!("missing")),
        Box::new(|v| v["refs"][0]["from"]["column"] = json!("missing")),
        Box::new(|v| v["refs"][0]["from"] = json!({"table":"users","column":"id"})),
        Box::new(|v| v["refs"][0]["cardinality"] = json!("optional")),
        Box::new(|v| {
            let r = v["refs"][0].clone();
            v["refs"].as_array_mut().unwrap().push(r);
        }),
        Box::new(|v| v["tables"][0]["columns"][0]["data_type"] = json!("varchar(30)")),
        Box::new(|v| v["tables"][0]["columns"][0]["data_type"] = json!("a".repeat(17))),
        Box::new(|v| v["tables"][0]["name"] = json!("a".repeat(25))),
        Box::new(|v| {
            v["tables"][0]["columns"][0]["name"] = json!("a".repeat(12));
            v["tables"][0]["columns"][0]["data_type"] = json!("a".repeat(10));
        }),
    ];
    for mutate in mutations {
        let mut value = sample();
        mutate(&mut value);
        rejects(value.to_string().as_bytes(), &[]);
    }
    for invalid in [
        "a b", "a\nb", "\0", "\u{7f}", "\u{85}", "\u{202e}", "é", "界", "a\u{301}", "<script>",
        "a&b", "a\"b", "a.b", "1bad", "",
    ] {
        for pointer in [
            "/tables/0/name",
            "/tables/0/columns/0/name",
            "/tables/0/columns/0/data_type",
            "/refs/0/from/table",
            "/refs/0/from/column",
        ] {
            let mut value = sample();
            *value.pointer_mut(pointer).unwrap() = json!(invalid);
            rejects(value.to_string().as_bytes(), &[]);
        }
    }
    for args in [
        &["--theme", "unknown"][..],
        &["--background", "red"],
        &["--background", "url(https://example.com/a)"],
        &["--background", "#fff"],
        &["--background", "#12345678"],
        &["--background", "#12xx34"],
    ] {
        rejects(SAMPLE.as_bytes(), args);
    }
}
#[test]
fn exact_limits_and_all_cardinality_labels_are_supported() {
    let dir = TempDir::new().unwrap();
    let mut value = sample();
    value["refs"] = json!([]);
    value["tables"].as_array_mut().unwrap().truncate(1);
    value["tables"][0]["name"] = json!("a".repeat(12));
    value["tables"][0]["columns"] = json!(
        (0..16)
            .map(|i| json!({"name":format!("c{i}"),"data_type":"int","flags":[]}))
            .collect::<Vec<_>>()
    );
    value["tables"][0]["columns"][0] =
        json!({"name":"a".repeat(12),"data_type":"a".repeat(4),"flags":[]});
    checked(dir.path(), value.to_string().as_bytes(), &[]);
    for (card, label) in [
        ("many-to-one", "N:1"),
        ("one-to-many", "1:N"),
        ("one-to-one", "1:1"),
        ("many-to-many", "N:N"),
    ] {
        let mut v = sample();
        v["refs"][0]["cardinality"] = json!(card);
        let svg = checked(dir.path(), v.to_string().as_bytes(), &[]);
        assert!(
            String::from_utf8(svg)
                .unwrap()
                .contains(&format!("uid {label} id"))
        );
    }
}
#[test]
fn describes_exact_embedded_capability_and_builtin_readiness() {
    let result = Command::new(env!("CARGO_BIN_EXE_plot-provider-dbml"))
        .args(["describe", "--json"])
        .output()
        .unwrap();
    assert!(result.status.success());
    let description: Value = serde_json::from_slice(&result.stdout).unwrap();
    let command: Value =
        serde_json::from_str(include_str!("../assets/dbml-render-svg-command-v1.json")).unwrap();
    assert_eq!(description["commands"][0], command);
    let result = Command::new(env!("CARGO_BIN_EXE_plot-provider-dbml"))
        .args(["doctor", "--json"])
        .output()
        .unwrap();
    assert!(result.status.success());
    let doctor: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(doctor["ok"], true);
}

#[test]
fn pins_and_destination_aliases_fail_before_any_output_replacement() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.json");
    let svg = dir.path().join("figure.svg");
    let receipt = dir.path().join("receipt.json");
    fs::write(&input, SAMPLE).unwrap();
    fs::write(&svg, b"old svg").unwrap();
    fs::write(&receipt, b"old receipt").unwrap();
    let good = json!({"input":hash(SAMPLE.as_bytes())}).to_string();
    let run = |pins: &str, output: &Path, receipt: &Path| {
        Command::new(env!("CARGO_BIN_EXE_plot-provider-dbml"))
            .arg("render-svg")
            .arg("--input")
            .arg(&input)
            .arg("--resource-pins")
            .arg(pins)
            .arg("--output")
            .arg(output)
            .arg("--receipt")
            .arg(receipt)
            .output()
            .unwrap()
    };
    for pins in [
        "{}".into(),
        json!({"input":"0".repeat(64)}).to_string(),
        json!({"input":hash(SAMPLE.as_bytes()),"other":"x"}).to_string(),
        format!(
            "{{\"input\":\"{}\",\"input\":\"{}\"}}",
            hash(SAMPLE.as_bytes()),
            hash(SAMPLE.as_bytes())
        ),
        " ".repeat(4097),
    ] {
        assert!(!run(&pins, &svg, &receipt).status.success());
    }
    for (out, rec) in [(&input, &receipt), (&svg, &input), (&svg, &svg)] {
        assert!(!run(&good, out, rec).status.success());
    }
    let missing = dir.path().join("missing/../input.json");
    assert!(!run(&good, &missing, &receipt).status.success());
    assert!(!dir.path().join("missing").exists());
    let directory = dir.path().join("directory");
    fs::create_dir(&directory).unwrap();
    assert!(!run(&good, &svg, &directory).status.success());
    #[cfg(unix)]
    {
        let symlink = dir.path().join("input-link");
        std::os::unix::fs::symlink(&input, &symlink).unwrap();
        assert!(!run(&good, &symlink, &receipt).status.success());
        let hardlink = dir.path().join("input-hardlink");
        fs::hard_link(&input, &hardlink).unwrap();
        assert!(!run(&good, &hardlink, &receipt).status.success());
        let outputs = dir.path().join("output-hardlink");
        fs::hard_link(&svg, &outputs).unwrap();
        assert!(!run(&good, &svg, &outputs).status.success());
    }
    assert_eq!(fs::read(&input).unwrap(), SAMPLE.as_bytes());
    assert_eq!(fs::read(&svg).unwrap(), b"old svg");
    assert_eq!(fs::read(&receipt).unwrap(), b"old receipt");
}

#[test]
fn input_byte_budget_is_inclusive_and_invalid_input_does_not_create_output_parents() {
    let dir = TempDir::new().unwrap();
    let mut padded = SAMPLE.as_bytes().to_vec();
    padded.resize(65536, b' ');
    checked(dir.path(), &padded, &[]);
    padded.push(b' ');
    rejects(&padded, &[]);
    let input = dir.path().join("bad.json");
    fs::write(&input, b"{}").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_plot-provider-dbml"))
        .arg("render-svg")
        .arg("--input")
        .arg(input)
        .arg("--resource-pins")
        .arg(json!({"input":hash(b"{}")}).to_string())
        .arg("--output")
        .arg(dir.path().join("missing/svg"))
        .arg("--receipt")
        .arg(dir.path().join("missing/receipt"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!dir.path().join("missing").exists());
}

#[test]
fn conservative_wide_glyph_boundaries_are_explicit_and_fail_closed() {
    let directory = TempDir::new().unwrap();
    let wide: Value =
        serde_json::from_str(include_str!("../examples/dbml-authored/wide-labels.json")).unwrap();
    for cardinality in ["many-to-one", "one-to-many", "one-to-one", "many-to-many"] {
        for reverse in [false, true] {
            let mut value = wide.clone();
            value["refs"][0]["cardinality"] = json!(cardinality);
            if reverse {
                let from = value["refs"][0]["from"].clone();
                value["refs"][0]["from"] = value["refs"][0]["to"].clone();
                value["refs"][0]["to"] = from;
            }
            let svg = checked(directory.path(), value.to_string().as_bytes(), &[]);
            let text = String::from_utf8(svg).unwrap();
            assert!(!text.contains('…'));
            assert!(text.contains(&"W".repeat(12)));
        }
    }
    // The formerly accepted 24-W title/body and 18-cell reference visibly
    // overflow native card/pill geometry in the local Inkscape proof.
    for mutate in [0, 1, 2] {
        let mut value = wide.clone();
        match mutate {
            0 => value["tables"][0]["name"] = json!("W".repeat(13)),
            1 => value["tables"][0]["columns"][0]["data_type"] = json!("W".repeat(5)),
            _ => {
                value["tables"][1]["columns"][0]["name"] = json!("WWWW");
                value["refs"][0]["from"]["column"] = json!("WWWW");
            }
        }
        rejects(value.to_string().as_bytes(), &[]);
    }
    for flag in ["pk", "unique", "not null"] {
        let mut value = sample();
        value["tables"][0]["columns"][0]["flags"] = json!([flag]);
        checked(directory.path(), value.to_string().as_bytes(), &[]);
    }
    let mut maximal = sample();
    maximal["refs"] = json!([]);
    for table in maximal["tables"].as_array_mut().unwrap() {
        table["columns"] = json!(
            (0..16)
                .map(|i| json!({"name":format!("c{i}"),"data_type":"int","flags":[]}))
                .collect::<Vec<_>>()
        );
    }
    checked(directory.path(), maximal.to_string().as_bytes(), &[]);
    let receipt: Value =
        serde_json::from_slice(&fs::read(directory.path().join("receipt.json")).unwrap()).unwrap();
    assert_eq!(receipt["counts"]["columns"], 32);
}
