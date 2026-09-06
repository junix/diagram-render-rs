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
            "<article data-search=\"{title} {format} {file_name}\"><h2>{title}</h2><p>{format} · {:.0}×{:.0} CSS px</p><object data=\"{svg_name}\" type=\"image/svg+xml\" aria-label=\"{title} SVG\"></object><details><summary>2× PNG proof</summary><img src=\"{png_name}\" alt=\"{title} PNG\"></details></article>",
            rendered.scene_width, rendered.scene_height
        );
        for warning in rendered.warnings {
            eprintln!("{file_name}: {warning}");
        }
    }
    html.push_str(GALLERY_SCRIPT);
    html.push_str("</main></body></html>");
    fs::write(output.join("gallery.html"), html)?;
    println!(
        "rendered {} diagram families into {}",
        CASES.len(),
        output.display()
    );
    Ok(())
}

fn gallery_header() -> String {
    r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>diagram-render-rs gallery</title><style>
    :root{color-scheme:light dark;font-family:Inter,ui-sans-serif,system-ui,sans-serif;--page:#0b1220;--glow:#183255;--ink:#e8eef8;--lead:#aec1dd;--muted:#9fb1c9;--card:#101b2dcc;--line:#32445f;--shadow:#02071399;background:var(--page);color:var(--ink)}
    @media (prefers-color-scheme:light){:root{--page:#f3f7fd;--glow:#dbe8fa;--ink:#152238;--lead:#435a7d;--muted:#556789;--card:#fffffff2;--line:#c5d2e6;--shadow:#2a3f6626}}
    body{margin:0;background:radial-gradient(circle at top,var(--glow) 0,var(--page) 48rem);min-height:100vh}
    header{max-width:1440px;margin:auto;padding:48px 28px 22px}h1{font-size:clamp(2rem,5vw,4rem);margin:0}.lead{color:var(--lead);max-width:72ch}
    main{max-width:1440px;margin:auto;padding:18px 28px 64px;display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,520px),1fr));gap:26px}
    article{border:1px solid var(--line);border-radius:20px;padding:20px;background:var(--card);box-shadow:0 18px 50px var(--shadow)}h2{margin:0 0 5px}p,summary{color:var(--muted)}
    .toolbar{display:flex;gap:14px;align-items:center;flex-wrap:wrap;margin-top:20px}
    .toolbar input{font:inherit;color:var(--ink);background:var(--card);border:1px solid var(--line);border-radius:10px;padding:10px 14px;width:min(480px,100%)}
    .toolbar input::placeholder{color:var(--muted)}
    .toolbar #count{color:var(--muted)}
    article[hidden]{display:none}
    object,img{display:block;width:100%;height:auto;min-height:280px;max-height:720px;object-fit:contain;border-radius:12px;margin-top:18px;background-color:#fff;background-image:linear-gradient(45deg,#edf1f6 25%,transparent 25%),linear-gradient(-45deg,#edf1f6 25%,transparent 25%),linear-gradient(45deg,transparent 75%,#edf1f6 75%),linear-gradient(-45deg,transparent 75%,#edf1f6 75%);background-size:24px 24px;background-position:0 0,0 12px,12px -12px,-12px 0}
    details{margin-top:16px}summary{cursor:pointer}
    </style></head><body><header><h1>Typed AST → SVG / PNG</h1><p class="lead">Seven format-specific renderers share only a finite drawing scene and raster backend. The checkerboard exposes the transparent canvas contract.</p><div class="toolbar"><input id="filter" type="search" placeholder="Filter by name, format, or input file" aria-label="Filter diagrams"><span id="count" aria-live="polite">7 of 7 diagrams</span></div></header><main>"#.to_owned()
}

const GALLERY_SCRIPT: &str = r#"<script>
(function(){
  var input=document.getElementById('filter'),count=document.getElementById('count');
  var cards=[].slice.call(document.querySelectorAll('main article'));
  function apply(){
    var needle=(input.value||'').trim().toLowerCase(),shown=0;
    cards.forEach(function(card){
      var hit=!needle||(card.getAttribute('data-search')||'').toLowerCase().indexOf(needle)>=0;
      card.hidden=!hit;
      if(hit)shown++;
    });
    count.textContent=shown?shown+' of '+cards.length+' diagrams':'No diagrams match';
  }
  input.addEventListener('input',apply);
  apply();
})();
</script>"#;
