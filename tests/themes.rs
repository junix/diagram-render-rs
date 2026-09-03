//! The ADR-1137 theme contract as this binary owes it: the shared vocabulary,
//! the mapping onto twelve drawing colors, and transparency in every theme.

use std::process::Command;

use diagram_render_rs::theme::{LEGACY, PROFILE};
use diagram_render_rs::{DiagramFormat, OutputFormat, RenderOptions, Theme, render_source};
use diagram_theme::cli::{listing_json, listing_plain};
use diagram_theme::{FONT_SANS, Resolved, Theme as Palette, resolve};

const FIXTURE: &str = include_str!("../examples/inputs/schema.dbml");

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_diagram-render-rs"))
        .args(args)
        .output()
        .expect("run CLI")
}

#[test]
fn themes_prints_the_fourteen_bare_lines_two_justfiles_loop_over() {
    let output = cli(&["themes"]);
    assert!(output.status.success());
    let listing = String::from_utf8(output.stdout).expect("UTF-8 listing");
    assert_eq!(listing, listing_plain());
    assert_eq!(listing.lines().count(), 14);
}

#[test]
fn themes_json_publishes_this_renderers_profile_verbatim() {
    let output = cli(&["themes", "--json"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8 document"),
        listing_json(&PROFILE)
    );
}

#[test]
fn the_input_path_still_wins_the_first_positional() {
    // `themes` is matched ahead of clap, so the bare-path form the README and
    // the e2e harness use must keep working.
    let output = cli(&["examples/inputs/schema.dbml", "--format", "dbml", "--quiet"]);
    assert!(output.status.success());
    assert!(output.stdout.starts_with(b"<svg"));
}

#[test]
fn an_unknown_theme_reports_the_shared_error_and_this_binarys_legacy_names() {
    let output = cli(&["--theme", "nord", "examples/inputs/schema.dbml"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown theme `nord`"), "{stderr}");
    assert!(
        stderr.contains("also accepted here: light, dark"),
        "{stderr}"
    );
    assert!(
        stderr.contains("run `diagram-render-rs themes` for the full list"),
        "{stderr}"
    );
}

#[test]
fn the_legacy_names_stay_their_own_palettes_and_the_default_stays_light() {
    assert_eq!(PROFILE.default_theme, "light");
    let names: Vec<&str> = LEGACY.iter().map(|entry| entry.name).collect();
    assert_eq!(names, ["light", "dark"]);
    assert_eq!(Theme::resolved(Resolved::Legacy("light")), Theme::light());
    assert_eq!(Theme::resolved(Resolved::Legacy("dark")), Theme::dark());
    // Aliasing them onto azure would change the checked-in gallery and the
    // palette `preview-rs` calls these constructors for.
    assert_ne!(
        Theme::light(),
        Theme::from_tokens(Palette::DEFAULT.tokens())
    );
}

#[test]
fn the_mapping_is_the_one_adr_1137_fixed() {
    let tokens = Palette::DEFAULT.tokens();
    let theme = Theme::from_tokens(tokens);
    assert_eq!(theme.font_family, FONT_SANS);
    assert_eq!(theme.foreground, tokens.ink.hex());
    assert_eq!(theme.muted, tokens.muted.hex());
    assert_eq!(theme.line, tokens.edge.hex());
    assert_eq!(theme.surface, tokens.panel.hex());
    assert_eq!(theme.surface_alt, tokens.group.hex());
    assert_eq!(theme.accent, tokens.accent.hex());
    assert_eq!(theme.accent_soft, tokens.accent_soft.hex());
    assert_eq!(theme.positive, tokens.pos.hex());
    assert_eq!(theme.warning, tokens.warn.hex());
    assert_eq!(theme.danger, tokens.neg.hex());
    assert_eq!(theme.grid, tokens.grid.hex());
    for (slot, token) in tokens.series().iter().enumerate() {
        assert_eq!(theme.series(slot), token.hex());
    }
}

#[test]
fn the_declared_unmapped_roles_never_reach_a_drawing_color() {
    for palette in Palette::ALL {
        let tokens = palette.tokens();
        let theme = Theme::from_tokens(tokens);
        let drawn: Vec<&str> = [
            theme.foreground.as_str(),
            theme.muted.as_str(),
            theme.line.as_str(),
            theme.surface.as_str(),
            theme.surface_alt.as_str(),
            theme.accent.as_str(),
            theme.accent_soft.as_str(),
            theme.positive.as_str(),
            theme.warning.as_str(),
            theme.danger.as_str(),
            theme.grid.as_str(),
        ]
        .into_iter()
        .chain(theme.series.iter().map(String::as_str))
        .collect();
        // `observable_unmapped` drops the roles the registry makes
        // byte-identical to a mapped one, which no value-based check could
        // ever be honest about.
        for role in PROFILE.observable_unmapped(palette) {
            let hex = tokens.role(role).expect("a contract role").hex();
            assert!(!drawn.contains(&hex), "{} draws {role}", palette.name());
        }
    }
}

#[test]
fn the_two_occluding_surfaces_are_opaque_in_every_theme() {
    // `draw_connectors` routes polylines under the cards and knocks them out
    // with a `surface_alt` pill; card bodies occlude with `surface`. Both are
    // six-digit hex, never an eight-digit wash.
    for palette in Palette::ALL {
        let theme = Theme::from_tokens(palette.tokens());
        for (role, value) in [
            ("surface", &theme.surface),
            ("surface_alt", &theme.surface_alt),
        ] {
            assert_eq!(value.len(), 7, "{} {role} = {value}", palette.name());
            assert!(value.starts_with('#'), "{} {role}", palette.name());
        }
    }
}

#[test]
fn the_series_ramp_has_eight_distinct_slots_and_cycles() {
    for theme in [
        Theme::light(),
        Theme::dark(),
        Theme::from_tokens(Palette::DEFAULT.tokens()),
    ] {
        let mut distinct = theme.series.to_vec();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), 8);
        // A category never falls back on `muted`, which is what the previous
        // five-slot cycle drew at slot 4.
        assert!(!theme.series.contains(&theme.muted));
        assert_eq!(theme.series(8), theme.series(0));
        assert_eq!(theme.series(13), theme.series(5));
    }
}

#[test]
fn resolve_accepts_the_fourteen_and_this_binarys_two() {
    let legacy: Vec<&'static str> = LEGACY.iter().map(|entry| entry.name).collect();
    for palette in Palette::ALL {
        assert_eq!(
            resolve(palette.name(), &legacy, "diagram-render-rs"),
            Ok(Resolved::Canonical(palette))
        );
    }
    assert!(resolve("azure-light", &legacy, "diagram-render-rs").is_err());
}

#[test]
fn every_theme_renders_a_canvas_that_is_clear_at_twelve_sample_points() {
    for palette in Palette::ALL {
        let options = RenderOptions {
            scale: 1.0,
            theme: Theme::from_tokens(palette.tokens()),
            ..RenderOptions::default()
        };
        let rendered = render_source(DiagramFormat::Dbml, FIXTURE, OutputFormat::Png, &options)
            .unwrap_or_else(|error| panic!("{} render failed: {error}", palette.name()));
        assert!(
            !rendered.svg.contains("data-canvas-background"),
            "{} painted a canvas",
            palette.name()
        );
        let pixmap = resvg::tiny_skia::Pixmap::decode_png(&rendered.png.expect("PNG requested"))
            .expect("valid PNG");
        let (right, bottom) = (pixmap.width() - 1, pixmap.height() - 1);
        // Four corners, four edge midpoints, and four points inset 2% from the
        // corners: the inset ones are the holes a corners-only check leaves.
        let (inset_x, inset_y) = (pixmap.width() / 50, pixmap.height() / 50);
        let samples = [
            (0, 0),
            (right, 0),
            (0, bottom),
            (right, bottom),
            (right / 2, 0),
            (right / 2, bottom),
            (0, bottom / 2),
            (right, bottom / 2),
            (inset_x, inset_y),
            (right - inset_x, inset_y),
            (inset_x, bottom - inset_y),
            (right - inset_x, bottom - inset_y),
        ];
        for (x, y) in samples {
            assert_eq!(
                pixmap.pixel(x, y).expect("sampled pixel").alpha(),
                0,
                "{} is opaque at {x},{y}",
                palette.name()
            );
        }
        // An all-clear image would pass the loop above vacuously.
        assert!(
            pixmap.pixels().iter().any(|pixel| pixel.alpha() == 255),
            "{} drew nothing",
            palette.name()
        );
    }
}

#[test]
fn each_family_renders_differently_in_its_two_modes() {
    let mut previous = None;
    for palette in Palette::ALL {
        let theme = Theme::from_tokens(palette.tokens());
        let svg = render_source(
            DiagramFormat::Dbml,
            FIXTURE,
            OutputFormat::Svg,
            &RenderOptions {
                theme: theme.clone(),
                ..RenderOptions::default()
            },
        )
        .expect("render")
        .svg;
        // Nothing adapts to the host page: the drawing carries the ink of the
        // theme that was asked for, and the -dark half of a family is a
        // different drawing rather than a synonym.
        assert!(svg.contains(&theme.foreground), "{}", palette.name());
        if palette.is_dark() {
            assert_ne!(previous.take(), Some(svg), "{}", palette.name());
        } else {
            previous = Some(svg);
        }
    }
}
