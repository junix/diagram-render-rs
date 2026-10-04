//! Exercise the actual shared output gate, including the paired PNG exemption.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use diagram_render_rs::{
    DiagramFormat, OutputFormat, RenderOptions, Rendered, Theme, render_source,
};
use diagram_theme::Theme as Palette;
use diagram_theme::output::{Note, Report, check_files, declares_author_canvas};

// Unlabelled edges isolate the canvas contract. Labelled connectors and every
// gallery family are covered separately by gallery_contract.
const SOURCE: &str = "a -> b";

struct OutputPair {
    directory: PathBuf,
    svg: PathBuf,
    png: PathBuf,
}

impl OutputPair {
    fn new(rendered: &Rendered) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "diagram-render-canvas-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).expect("create isolated output directory");
        let pair = Self {
            svg: directory.join("output.svg"),
            png: directory.join("output.png"),
            directory,
        };
        std::fs::write(&pair.svg, &rendered.svg).expect("write rendered SVG");
        std::fs::write(&pair.png, rendered.png.as_ref().expect("PNG requested"))
            .expect("write rendered PNG");
        pair
    }

    fn check(&self, palette: Palette) -> Report {
        check_files(palette, Some(&self.svg), Some(&self.png))
    }
}

impl Drop for OutputPair {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn render(palette: Palette, background: Option<&str>) -> Rendered {
    render_source(
        DiagramFormat::D2,
        SOURCE,
        OutputFormat::Png,
        &RenderOptions {
            theme: Theme::from_tokens(palette.tokens()),
            background: background.map(str::to_owned),
            ..RenderOptions::default()
        },
    )
    .expect("render canvas fixture")
}

fn assert_fully_checked(report: &Report, context: &str) {
    assert!(!report.failed(), "{context}: {:?}", report.notes);
    assert!(
        !report
            .notes
            .iter()
            .any(|note| matches!(note, Note::Skip { .. })),
        "{context}: incomplete gate coverage: {:?}",
        report.notes
    );
}

#[test]
fn default_transparent_svg_and_png_pass_the_shared_gate_in_every_canonical_theme() {
    for palette in Palette::ALL {
        let rendered = render(palette, None);
        let pair = OutputPair::new(&rendered);
        let report = pair.check(palette);
        assert_fully_checked(&report, palette.name());
        assert!(!rendered.svg.contains("data-canvas-background"));
        assert!(!declares_author_canvas(&rendered.svg));
        assert!(
            report.notes.is_empty(),
            "unexpected exemption: {:?}",
            report.notes
        );
    }
}

#[test]
fn explicit_background_svg_and_png_pass_the_shared_gate_in_every_canonical_theme() {
    // Deliberately off-palette: an explicit canvas must get both the geometry
    // and palette exemptions, and carry that authorization into the PNG gate.
    for palette in Palette::ALL {
        let rendered = render(palette, Some("#123456"));
        let pair = OutputPair::new(&rendered);
        let report = pair.check(palette);
        assert_fully_checked(&report, palette.name());
        assert!(declares_author_canvas(&rendered.svg));
        assert_eq!(
            rendered
                .svg
                .matches("data-canvas-background=\"author\"")
                .count(),
            1
        );
        assert!(
            report.notes.iter().any(|note| matches!(
                note, Note::Exempt { detail } if detail.contains("full-bleed")
            )),
            "{} did not report the author canvas: {:?}",
            palette.name(),
            report.notes
        );
        let pixmap =
            resvg::tiny_skia::Pixmap::decode_png(rendered.png.as_ref().expect("PNG requested"))
                .expect("valid PNG");
        let corner = pixmap.pixel(0, 0).expect("corner pixel");
        assert_eq!(
            (corner.red(), corner.green(), corner.blue(), corner.alpha()),
            (0x12, 0x34, 0x56, 255)
        );
    }
}

#[test]
fn the_old_boolean_marker_fails_the_shared_svg_and_png_canvas_rules() {
    let palette = Palette::DEFAULT;
    let mut rendered = render(palette, Some("#123456"));
    rendered.svg = rendered.svg.replace(
        "data-canvas-background=\"author\"",
        "data-canvas-background=\"true\"",
    );
    assert!(!declares_author_canvas(&rendered.svg));
    let pair = OutputPair::new(&rendered);
    let report = pair.check(palette);
    assert!(report.failed(), "the old marker must fail the shared gate");
    for expected in ["full-bleed", "palette", "canvas"] {
        assert!(
            report.notes.iter().any(|note| matches!(
                note, Note::Fail { rule, .. } if *rule == expected
            )),
            "missing {expected} failure: {:?}",
            report.notes
        );
    }
}
