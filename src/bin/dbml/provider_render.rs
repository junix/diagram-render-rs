use super::{
    provider_io::Input,
    provider_model::Authored,
    provider_publication::{StagedFile, publish},
};
use anyhow::{Result, ensure};
use clap::Args;
use diagram_render_v1::{OutputFormat, RenderOptions, Theme, render_document};
use diagram_theme::{Resolved, cli::theme_value_parser};
use std::path::PathBuf;

pub(crate) const MAX_SVG_BYTES: usize = 256 * 1024;
#[derive(Args)]
pub(crate) struct Options {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    resource_pins: String,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    receipt: PathBuf,
    #[arg(long, default_value="light", value_parser=theme_value_parser(diagram_render_v1::theme::LEGACY,"plot-provider-dbml"))]
    theme: Resolved,
    /// Optional literal #RRGGBB canvas color. Omit for transparency.
    #[arg(long)]
    background: Option<String>,
}
pub(crate) fn error(message: impl Into<String>) -> anyhow::Error {
    anyhow::anyhow!(message.into())
}
pub(crate) fn run(options: Options) -> Result<()> {
    if let Some(background) = &options.background {
        ensure!(
            background.len() == 7
                && background.starts_with('#')
                && background[1..].bytes().all(|b| b.is_ascii_hexdigit()),
            "background must be a literal #RRGGBB color"
        );
    }
    let input = Input::read(
        &options.input,
        &options.resource_pins,
        &options.output,
        &options.receipt,
    )?;
    let authored = Authored::parse(&input.bytes)?;
    let native = authored.native();
    let render_options = RenderOptions {
        theme: Theme::resolved(options.theme),
        background: options.background.clone(),
        ..RenderOptions::default()
    };
    let rendered = render_document(&native, OutputFormat::Svg, &render_options)?;
    validate_rendered(&rendered)?;
    let receipt = super::provider_receipt::build(
        input.receipt(),
        authored.counts(),
        &rendered,
        match options.theme {
            Resolved::Canonical(theme) => theme.name(),
            Resolved::Legacy(name) => name,
        },
        options.background,
    )?;
    // No output directory is created until parsing, rendering and receipt checking succeed.
    input.recheck(&options.output, &options.receipt)?;
    let svg_stage = StagedFile::new(&options.output)?;
    let receipt_stage = StagedFile::new(&options.receipt)?;
    svg_stage.write(rendered.svg.as_bytes())?;
    receipt_stage.write(&receipt)?;
    ensure!(
        super::provider_io::read_bounded_regular(svg_stage.path(), MAX_SVG_BYTES)?
            == rendered.svg.as_bytes(),
        "staged SVG bytes changed"
    );
    ensure!(
        super::provider_io::read_bounded_regular(
            receipt_stage.path(),
            super::provider_receipt::MAX_RECEIPT_BYTES
        )? == receipt,
        "staged receipt bytes changed"
    );
    input.recheck(&options.output, &options.receipt)?;
    publish(vec![svg_stage, receipt_stage])
}
fn validate_rendered(rendered: &diagram_render_v1::Rendered) -> Result<()> {
    ensure!(
        rendered.warnings.is_empty(),
        "DBML provider refuses {} native warning(s)",
        rendered.warnings.len()
    );
    ensure!(
        !rendered.svg.is_empty() && rendered.svg.len() <= MAX_SVG_BYTES,
        "SVG exceeds 256 KiB output budget or is empty"
    );
    ensure!(
        rendered.scene_width.is_finite()
            && rendered.scene_height.is_finite()
            && rendered.scene_width > 0.0
            && rendered.scene_height > 0.0,
        "invalid SVG dimensions"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_native_warnings_fail_closed() {
        let warning = diagram_render_v1::render_source(
            diagram_render_v1::DiagramFormat::Dbml,
            "Table x { id int [ref: > y.id] }",
            OutputFormat::Svg,
            &RenderOptions::default(),
        )
        .unwrap();
        assert!(validate_rendered(&warning).is_err());
        let mut oversized = warning.clone();
        oversized.warnings.clear();
        oversized.svg = "x".repeat(MAX_SVG_BYTES + 1);
        assert!(validate_rendered(&oversized).is_err());
    }
}
