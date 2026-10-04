//! Gate real CLI gallery pairs with the renderer's declared shared-theme revision.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use diagram_render_rs::{DiagramFormat, OutputFormat, RenderOptions, Theme, render_source};
use diagram_theme::Theme as Palette;
use diagram_theme::output::{Note, Report, check_files, check_svg};

// Keep all seven source families represented, including the labelled connectors
// that the focused canvas fixture deliberately does not exercise.
const CASES: &[(&str, &str, bool)] = &[
    ("schema.dbml", "dbml", true),
    ("timing.json5", "wavedrom", false),
    ("architecture.d2", "d2", true),
    ("workspace.dsl", "structurizr", true),
    ("model.c4", "likec4", true),
    ("classes.nomnoml", "nomnoml", false),
    ("flow.pikchr", "pikchr", false),
];

struct OutputPair {
    directory: PathBuf,
    svg: PathBuf,
    png: PathBuf,
}

impl OutputPair {
    fn render(file: &str, format: &str, palette: Palette) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "diagram-render-gallery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).expect("create isolated output directory");
        let pair = Self {
            svg: directory.join("output.svg"),
            png: directory.join("output.png"),
            directory,
        };
        for output in [&pair.svg, &pair.png] {
            let result = Command::new(env!("CARGO_BIN_EXE_diagram-render-rs"))
                .args([
                    &format!("examples/inputs/{file}"),
                    "--format",
                    format,
                    "--theme",
                    palette.name(),
                    "--quiet",
                    "--output",
                ])
                .arg(output)
                .output()
                .expect("run CLI");
            assert!(
                result.status.success(),
                "{} {file} {}: {}",
                palette.name(),
                output.display(),
                String::from_utf8_lossy(&result.stderr)
            );
        }
        pair
    }
}

impl Drop for OutputPair {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn assert_clean(report: &Report, context: &str) {
    assert!(!report.failed(), "{context}: {:?}", report.notes);
    // A pass with skipped geometry or an author exemption would hide a
    // regression in these fully measurable, transparent, theme-only fixtures.
    assert!(report.notes.is_empty(), "{context}: {:?}", report.notes);
}

#[test]
fn every_gallery_cli_pair_passes_the_pinned_gate_in_every_canonical_theme() {
    for palette in Palette::ALL {
        for (file, format, has_labels) in CASES {
            let pair = OutputPair::render(file, format, palette);
            let context = format!("{} {file}", palette.name());
            let svg = std::fs::read_to_string(&pair.svg).expect("read rendered SVG");
            assert!(!svg.contains("data-canvas-background"), "{context}");
            if *has_labels {
                // Palette membership is about the token color. Preserve the
                // renderer's existing label opacity instead of making it opaque
                // merely to satisfy an older, alpha-exact installed checker.
                let pill = format!(
                    "fill=\"{}\" fill-opacity=\"0.980\"",
                    palette.tokens().group.hex()
                );
                assert!(
                    svg.contains(&pill),
                    "{context}: missing translucent label pill"
                );
            }
            assert_clean(
                &check_files(palette, Some(&pair.svg), Some(&pair.png)),
                &context,
            );
        }
    }
}

#[test]
fn pill_opacity_does_not_exempt_foreign_colors_or_full_canvas_fills() {
    for palette in Palette::ALL {
        let rendered = render_source(
            DiagramFormat::Dbml,
            include_str!("../examples/inputs/schema.dbml"),
            OutputFormat::Svg,
            &RenderOptions {
                theme: Theme::from_tokens(palette.tokens()),
                ..RenderOptions::default()
            },
        )
        .expect("render labelled fixture");
        let pill = format!(
            "fill=\"{}\" fill-opacity=\"0.980\"",
            palette.tokens().group.hex()
        );
        assert!(rendered.svg.contains(&pill), "{}", palette.name());
        let mut clean = Report::default();
        check_svg(&rendered.svg, palette, &mut clean);
        assert_clean(&clean, palette.name());

        let foreign = rendered
            .svg
            .replacen(&pill, "fill=\"#ff00ff\" fill-opacity=\"0.980\"", 1);
        let full_canvas = rendered.svg.replace(
            "</g></svg>",
            &format!(
                "<rect x=\"0\" y=\"0\" width=\"{:.2}\" height=\"{:.2}\" {pill}/></g></svg>",
                rendered.scene_width, rendered.scene_height
            ),
        );
        for (source, expected_rule) in [(foreign, "palette"), (full_canvas, "full-bleed")] {
            let mut report = Report::default();
            check_svg(&source, palette, &mut report);
            assert!(
                report.notes.iter().any(|note| matches!(
                    note, Note::Fail { rule, .. } if *rule == expected_rule
                )),
                "{} must reject {expected_rule}: {:?}",
                palette.name(),
                report.notes
            );
        }
    }
}
