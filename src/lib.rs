//! Typed diagram AST to SVG/PNG rendering.
//!
//! Each source language keeps a dedicated interpretation/layout module. The
//! modules meet only at [`Scene`], a drawing-primitive boundary with no graph
//! or source-language semantics.

#![forbid(unsafe_code)]

mod error;
mod raster;
mod renderers;
mod svg;

pub mod scene;
pub mod theme;

pub use diagram_ast_parser::{Format as DiagramFormat, ast::Document};
pub use error::{RenderError, Result};
pub use scene::Scene;
pub use theme::{Theme, ThemePreset};

/// Requested output representation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OutputFormat {
    /// Scalable Vector Graphics.
    #[default]
    Svg,
    /// Portable Network Graphics rasterized from the generated SVG.
    Png,
}

/// Rendering controls shared by every AST family.
#[derive(Clone, Debug)]
pub struct RenderOptions {
    /// PNG scale when [`width`](Self::width) is not supplied.
    pub scale: f32,
    /// Exact PNG width in pixels. Takes precedence over [`scale`](Self::scale).
    pub width: Option<u32>,
    /// `None` keeps the SVG/PNG canvas transparent.
    pub background: Option<String>,
    /// Resolved drawing palette and typography.
    pub theme: Theme,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            scale: 1.0,
            width: None,
            background: None,
            theme: Theme::light(),
        }
    }
}

/// One rendering result. SVG is always retained, including for PNG requests.
#[derive(Clone, Debug)]
pub struct Rendered {
    pub svg: String,
    pub png: Option<Vec<u8>>,
    pub scene_width: f32,
    pub scene_height: f32,
    pub pixel_dimensions: Option<(u32, u32)>,
    /// Explicit degradations such as unresolved model relationships.
    pub warnings: Vec<String>,
}

/// Render an already parsed typed AST.
pub fn render_document(
    document: &Document,
    output: OutputFormat,
    options: &RenderOptions,
) -> Result<Rendered> {
    validate_options(options)?;
    let plan = renderers::render(document, &options.theme);
    plan.scene.validate().map_err(RenderError::InvalidScene)?;
    let svg = svg::scene_to_svg(
        &plan.scene,
        options.background.as_deref(),
        &options.theme.font_family,
    );

    if output == OutputFormat::Svg {
        return Ok(Rendered {
            scene_width: plan.scene.width,
            scene_height: plan.scene.height,
            svg,
            png: None,
            pixel_dimensions: None,
            warnings: plan.warnings,
        });
    }

    let raster = raster::svg_to_png(&svg, options.scale, options.width)?;
    Ok(Rendered {
        scene_width: plan.scene.width,
        scene_height: plan.scene.height,
        svg,
        png: Some(raster.bytes),
        pixel_dimensions: Some((raster.width, raster.height)),
        warnings: plan.warnings,
    })
}

/// Parse source text with `diagram-ast-parser`, then render its typed AST.
pub fn render_source(
    format: DiagramFormat,
    source: &str,
    output: OutputFormat,
    options: &RenderOptions,
) -> Result<Rendered> {
    let document = diagram_ast_parser::parse(format, source)?;
    render_document(&document, output, options)
}

/// Convenience wrapper for SVG output from an AST.
pub fn render_svg(document: &Document, options: &RenderOptions) -> Result<String> {
    Ok(render_document(document, OutputFormat::Svg, options)?.svg)
}

/// Convenience wrapper for PNG output from an AST.
pub fn render_png(document: &Document, options: &RenderOptions) -> Result<Vec<u8>> {
    render_document(document, OutputFormat::Png, options)?
        .png
        .ok_or_else(|| RenderError::Png("PNG output was not produced".to_owned()))
}

fn validate_options(options: &RenderOptions) -> Result<()> {
    if !options.scale.is_finite() || !(0.05..=16.0).contains(&options.scale) {
        return Err(RenderError::InvalidOption(
            "scale must be finite and between 0.05 and 16.0".to_owned(),
        ));
    }
    if options.width == Some(0) {
        return Err(RenderError::InvalidOption(
            "PNG width must be greater than zero".to_owned(),
        ));
    }
    Ok(())
}

/// Crate version from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
