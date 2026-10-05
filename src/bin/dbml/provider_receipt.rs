//! Closed path-free typed evidence bound to original input and actual SVG bytes.
use super::provider_model::Counts;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const MAX_RECEIPT_BYTES: usize = 16 * 1024;
pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct InputReceipt {
    pub role: String,
    pub sha256: String,
    pub bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Primary {
    artifact_id: String,
    role: String,
    argument: String,
    kind: String,
    sha256: String,
    bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ArtifactReceipt {
    schema_version: String,
    inputs: Vec<InputReceipt>,
    primary: Primary,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Component {
    id: String,
    version: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Revisions {
    renderer_code: String,
    parser: String,
    theme: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Canvas {
    width: f32,
    height: f32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Options {
    theme: String,
    #[serde(deserialize_with = "required_background")]
    background: Option<String>,
    warning_policy: String,
}
fn required_background<'de, D: serde::Deserializer<'de>>(
    de: D,
) -> std::result::Result<Option<String>, D::Error> {
    Option::<String>::deserialize(de)
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Fidelity {
    input_contract: String,
    layout: String,
    references: String,
    typography: String,
    source_text_parsed: bool,
    source_spans_preserved: bool,
    database_semantics_validated: bool,
    fonts_embedded: bool,
    fonts_pinned: bool,
    measured_shaping_verified: bool,
    glyph_coverage_verified: bool,
    cross_machine_pixel_parity: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema_version: String,
    profile: String,
    provider: Component,
    engine: Component,
    dependency_revisions: Revisions,
    artifact_receipt: ArtifactReceipt,
    counts: Counts,
    canvas: Canvas,
    options: Options,
    warnings: Vec<String>,
    fidelity: Fidelity,
}
pub(crate) fn build(
    input: InputReceipt,
    counts: Counts,
    rendered: &diagram_render_rs::Rendered,
    theme: &str,
    background: Option<String>,
) -> Result<Vec<u8>> {
    let receipt = Receipt {
        schema_version: "dbml.render-receipt/v1".into(),
        profile: super::PROFILE.into(),
        provider: Component {
            id: super::PROVIDER_ID.into(),
            version: super::version(),
        },
        engine: Component {
            id: "diagram-render-rs".into(),
            version: diagram_render_rs::VERSION.into(),
        },
        dependency_revisions: Revisions {
            renderer_code: super::RENDERER_REVISION.into(),
            parser: super::PARSER_REVISION.into(),
            theme: super::THEME_REVISION.into(),
        },
        artifact_receipt: ArtifactReceipt {
            schema_version: "plot.artifact-receipt-core/v1".into(),
            inputs: vec![input],
            primary: Primary {
                artifact_id: "figure".into(),
                role: "primary".into(),
                argument: "output".into(),
                kind: "svg".into(),
                sha256: digest(rendered.svg.as_bytes()),
                bytes: rendered.svg.len() as u64,
            },
        },
        counts,
        canvas: Canvas {
            width: rendered.scene_width,
            height: rendered.scene_height,
        },
        options: Options {
            theme: theme.into(),
            background,
            warning_policy: "deny".into(),
        },
        warnings: rendered.warnings.clone(),
        fidelity: Fidelity {
            input_contract: "closed-authored-dbml-json/v1".into(),
            layout: "native-table-cards".into(),
            references: "directed-table-card-connectors; cardinality-label-only; no-column-ports"
                .into(),
            typography: "native-font-family-viewer-resolved".into(),
            source_text_parsed: false,
            source_spans_preserved: false,
            database_semantics_validated: false,
            fonts_embedded: false,
            fonts_pinned: false,
            measured_shaping_verified: false,
            glyph_coverage_verified: false,
            cross_machine_pixel_parity: false,
        },
    };
    let bytes = serde_json::to_vec_pretty(&receipt)?;
    ensure!(
        bytes.len() <= MAX_RECEIPT_BYTES,
        "receipt exceeds 16 KiB output budget"
    );
    super::provider_json::validate(&bytes)?;
    ensure!(
        serde_json::from_slice::<Receipt>(&bytes)? == receipt,
        "receipt typed round-trip failed"
    );
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipt_objects_are_closed_and_duplicates_rejected() {
        for input in [
            r#"{"role":"input","sha256":"x","bytes":1,"path":"/tmp/input"}"#,
            r#"{"role":"input","role":"other","sha256":"x","bytes":1}"#,
        ] {
            assert!(serde_json::from_str::<InputReceipt>(input).is_err());
        }
        assert!(
            serde_json::from_str::<Options>(r#"{"theme":"light","warning_policy":"deny"}"#)
                .is_err()
        );
    }
}
