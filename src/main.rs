use std::fs;
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};
use diagram_render_rs::{
    DiagramFormat, Document, OutputFormat, RenderOptions, ThemePreset, render_document,
    render_source,
};

#[derive(Debug, Parser)]
#[command(
    name = "diagram-render-rs",
    version,
    about = "Render typed DBML, WaveDrom, D2, Structurizr, LikeC4, nomnoml, and Pikchr ASTs to SVG or PNG"
)]
struct Cli {
    /// Input source/AST JSON file, or `-` for stdin.
    #[arg(default_value = "-")]
    input: String,

    /// Output path. Without it, image bytes are written to stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Source language. Ignored for --ast-json.
    #[arg(short = 'f', long, default_value = "auto")]
    format: DiagramFormat,

    /// Treat input as serialized diagram_ast_parser::ast::Document JSON.
    #[arg(long)]
    ast_json: bool,

    /// Output format. `auto` uses the output extension and otherwise SVG.
    #[arg(short = 'T', long, value_enum, default_value_t = FormatArg::Auto)]
    output_format: FormatArg,

    /// Built-in light or dark palette.
    #[arg(long, value_enum, default_value_t = ThemeArg::Light)]
    theme: ThemeArg,

    /// Optional SVG/PNG canvas color. Omit for transparency.
    #[arg(long)]
    background: Option<String>,

    /// Override the SVG font-family stack.
    #[arg(long)]
    font_family: Option<String>,

    /// PNG pixel scale. Ignored for SVG and when --width is supplied.
    #[arg(long, default_value_t = 2.0)]
    scale: f32,

    /// Exact PNG output width in pixels.
    #[arg(long)]
    width: Option<u32>,

    /// Suppress output summaries and non-fatal renderer warnings.
    #[arg(short, long)]
    quiet: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
enum FormatArg {
    #[default]
    Auto,
    Svg,
    Png,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
enum ThemeArg {
    #[default]
    Light,
    Dark,
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("diagram-render-rs: {error}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> std::result::Result<(), String> {
    let input = read_input(&cli.input)?;
    let output_format = resolve_output_format(cli.output_format, cli.output.as_deref());
    let preset = match cli.theme {
        ThemeArg::Light => ThemePreset::Light,
        ThemeArg::Dark => ThemePreset::Dark,
    };
    let mut theme = preset.resolve();
    if let Some(font_family) = cli.font_family {
        theme.font_family = font_family;
    }
    let options = RenderOptions {
        scale: cli.scale,
        width: cli.width,
        background: cli.background,
        theme,
    };
    let rendered = if cli.ast_json {
        let document: Document = serde_json::from_str(&input).map_err(|error| error.to_string())?;
        render_document(&document, output_format, &options)
    } else {
        render_source(cli.format, &input, output_format, &options)
    }
    .map_err(|error| error.to_string())?;

    let bytes = match output_format {
        OutputFormat::Svg => rendered.svg.as_bytes(),
        OutputFormat::Png => rendered
            .png
            .as_deref()
            .ok_or_else(|| "PNG output was not produced".to_owned())?,
    };
    write_output(cli.output.as_deref(), bytes)?;

    if !cli.quiet {
        for warning in &rendered.warnings {
            eprintln!("warning: {warning}");
        }
        if let Some(path) = &cli.output {
            let dimensions = rendered.pixel_dimensions.map_or_else(
                || {
                    format!(
                        "{:.0}x{:.0} CSS px",
                        rendered.scene_width, rendered.scene_height
                    )
                },
                |(width, height)| format!("{width}x{height}px"),
            );
            eprintln!("wrote {} ({dimensions})", path.display());
        }
    }
    Ok(())
}

fn read_input(input: &str) -> std::result::Result<String, String> {
    if input == "-" {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|error| format!("failed to read stdin: {error}"))?;
        return Ok(source);
    }
    fs::read_to_string(input).map_err(|error| format!("failed to read {input}: {error}"))
}

fn resolve_output_format(requested: FormatArg, output: Option<&Path>) -> OutputFormat {
    match requested {
        FormatArg::Svg => OutputFormat::Svg,
        FormatArg::Png => OutputFormat::Png,
        FormatArg::Auto => match output
            .and_then(Path::extension)
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("png") => OutputFormat::Png,
            _ => OutputFormat::Svg,
        },
    }
}

fn write_output(path: Option<&Path>, bytes: &[u8]) -> std::result::Result<(), String> {
    if let Some(path) = path {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        return fs::write(path, bytes)
            .map_err(|error| format!("failed to write {}: {error}", path.display()));
    }
    io::stdout()
        .lock()
        .write_all(bytes)
        .map_err(|error| format!("failed to write stdout: {error}"))
}
