use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use diagram_render_rs::{DiagramFormat, OutputFormat, RenderOptions, render_source};

const CASES: &[(&str, &str, DiagramFormat)] = &[
    ("DBML schema", "schema.dbml", DiagramFormat::Dbml),
    ("WaveDrom timing", "timing.json5", DiagramFormat::WaveDrom),
    ("D2 architecture", "architecture.d2", DiagramFormat::D2),
    (
        "Structurizr workspace",
        "workspace.dsl",
        DiagramFormat::Structurizr,
    ),
    ("LikeC4 model", "model.c4", DiagramFormat::LikeC4),
    ("nomnoml classes", "classes.nomnoml", DiagramFormat::Nomnoml),
    ("Pikchr flow", "flow.pikchr", DiagramFormat::Pikchr),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("examples/rendered"));
    fs::create_dir_all(&output)?;
    let input_root = Path::new("examples/inputs");
    let options = RenderOptions {
        scale: 2.0,
        ..RenderOptions::default()
    };
    let mut html = gallery_header();

    for (title, file_name, format) in CASES {
        let source = fs::read_to_string(input_root.join(file_name))?;
        let rendered = render_source(*format, &source, OutputFormat::Png, &options)?;
        let stem = Path::new(file_name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("example has no UTF-8 stem")?;
        let svg_name = format!("{stem}.svg");
        let png_name = format!("{stem}.png");
        fs::write(output.join(&svg_name), &rendered.svg)?;
        fs::write(
            output.join(&png_name),
            rendered.png.as_deref().ok_or("PNG output missing")?,
        )?;
        let _ = write!(
            html,
            "<article><h2>{title}</h2><p>{format} · {:.0}×{:.0} CSS px</p><object data=\"{svg_name}\" type=\"image/svg+xml\" aria-label=\"{title} SVG\"></object><details><summary>2× PNG proof</summary><img src=\"{png_name}\" alt=\"{title} PNG\"></details></article>",
            rendered.scene_width, rendered.scene_height
        );
        for warning in rendered.warnings {
            eprintln!("{file_name}: {warning}");
        }
    }
    html.push_str("</main></body></html>");
    fs::write(output.join("index.html"), html)?;
    println!(
        "rendered {} diagram families into {}",
        CASES.len(),
        output.display()
    );
    Ok(())
}

fn gallery_header() -> String {
    r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>diagram-render-rs gallery</title><style>
    :root{color-scheme:light dark;font-family:Inter,ui-sans-serif,system-ui,sans-serif;background:#0b1220;color:#e8eef8}
    body{margin:0;background:radial-gradient(circle at top,#183255 0,#0b1220 48rem);min-height:100vh}
    header{max-width:1440px;margin:auto;padding:48px 28px 22px}h1{font-size:clamp(2rem,5vw,4rem);margin:0}.lead{color:#aec1dd;max-width:72ch}
    main{max-width:1440px;margin:auto;padding:18px 28px 64px;display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,520px),1fr));gap:26px}
    article{border:1px solid #32445f;border-radius:20px;padding:20px;background:#101b2dcc;box-shadow:0 18px 50px #02071399}h2{margin:0 0 5px}p,summary{color:#9fb1c9}
    object,img{display:block;width:100%;height:auto;min-height:280px;max-height:720px;object-fit:contain;border-radius:12px;margin-top:18px;background-color:#fff;background-image:linear-gradient(45deg,#edf1f6 25%,transparent 25%),linear-gradient(-45deg,#edf1f6 25%,transparent 25%),linear-gradient(45deg,transparent 75%,#edf1f6 75%),linear-gradient(-45deg,transparent 75%,#edf1f6 75%);background-size:24px 24px;background-position:0 0,0 12px,12px -12px,-12px 0}
    details{margin-top:16px}summary{cursor:pointer}
    </style></head><body><header><h1>Typed AST → SVG / PNG</h1><p class="lead">Seven format-specific renderers share only a finite drawing scene and raster backend. The checkerboard exposes the transparent canvas contract.</p></header><main>"#.to_owned()
}
