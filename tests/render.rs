use diagram_render_rs::{
    DiagramFormat, OutputFormat, RenderError, RenderOptions, render_document, render_source,
};

const CASES: &[(DiagramFormat, &str, &str)] = &[
    (
        DiagramFormat::Dbml,
        include_str!("../examples/inputs/schema.dbml"),
        "users",
    ),
    (
        DiagramFormat::WaveDrom,
        include_str!("../examples/inputs/timing.json5"),
        "Bus transaction",
    ),
    (
        DiagramFormat::D2,
        include_str!("../examples/inputs/architecture.d2"),
        "Client",
    ),
    (
        DiagramFormat::Structurizr,
        include_str!("../examples/inputs/workspace.dsl"),
        "Payments",
    ),
    (
        DiagramFormat::LikeC4,
        include_str!("../examples/inputs/model.c4"),
        "Customer",
    ),
    (
        DiagramFormat::Nomnoml,
        include_str!("../examples/inputs/classes.nomnoml"),
        "PostgreSQL",
    ),
    (
        DiagramFormat::Pikchr,
        include_str!("../examples/inputs/flow.pikchr"),
        "Rust",
    ),
];

#[test]
fn every_ast_family_renders_to_accessible_svg_and_transparent_png() {
    for (format, source, expected_text) in CASES {
        let rendered = render_source(
            *format,
            source,
            OutputFormat::Png,
            &RenderOptions::default(),
        )
        .unwrap_or_else(|error| panic!("{format} render failed: {error}"));
        assert!(rendered.svg.starts_with("<svg"), "{format}");
        assert!(rendered.svg.contains("role=\"img\""), "{format}");
        assert!(rendered.svg.contains("<title"), "{format}");
        assert!(rendered.svg.contains(expected_text), "{format}");
        assert!(!rendered.svg.contains("NaN"), "{format}");
        assert!(!rendered.svg.contains("data-canvas-background"), "{format}");
        assert!(rendered.scene_width > 100.0, "{format}");
        assert!(rendered.scene_height > 100.0, "{format}");
        let png = rendered.png.expect("PNG requested");
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']), "{format}");
        let pixmap = resvg::tiny_skia::Pixmap::decode_png(&png).expect("valid PNG");
        assert_eq!(
            pixmap.pixel(0, 0).expect("corner pixel").alpha(),
            0,
            "{format} canvas must be transparent"
        );
    }
}

#[test]
fn serialized_ast_round_trip_skips_source_parsing() {
    let document = diagram_ast_parser::parse(DiagramFormat::Dbml, CASES[0].1).expect("parse");
    let json = serde_json::to_string(&document).expect("serialize AST");
    let decoded = serde_json::from_str(&json).expect("deserialize AST");
    let rendered = render_document(&decoded, OutputFormat::Svg, &RenderOptions::default())
        .expect("render AST");
    assert!(rendered.svg.contains("commerce"));
    assert!(rendered.png.is_none());
}

#[test]
fn requested_png_width_is_exact() {
    let cases: &[(DiagramFormat, &str, &[u32])] = &[
        (DiagramFormat::D2, CASES[2].1, &[640]),
        (
            DiagramFormat::WaveDrom,
            "{signal:[{wave:'0'}]}",
            &[43, 44, 45, 660, 701, 702, 703],
        ),
        (
            DiagramFormat::WaveDrom,
            "{signal:[{wave:'0101010101'}]}",
            &[44, 45, 46, 702],
        ),
        (
            DiagramFormat::WaveDrom,
            "{signal:[{wave:'01010101010101010101'}]}",
            &[62, 63, 64, 1162],
        ),
    ];
    for &(format, source, widths) in cases {
        for &width in widths {
            let options = RenderOptions {
                width: Some(width),
                ..RenderOptions::default()
            };
            let rendered = render_source(format, source, OutputFormat::Png, &options)
                .unwrap_or_else(|error| panic!("{format} at width {width}: {error}"));
            let dimensions = rendered.pixel_dimensions.expect("PNG dimensions");
            assert_eq!(dimensions.0, width, "{format} at width {width}");
            let png = rendered.png.expect("PNG requested");
            let decoded = resvg::tiny_skia::Pixmap::decode_png(&png).expect("valid PNG");
            assert_eq!(
                (decoded.width(), decoded.height()),
                dimensions,
                "{format} at width {width}"
            );
        }
    }
}

#[test]
fn invalid_requested_png_width_is_rejected() {
    for width in [0, 32, 10_561, u32::MAX] {
        let options = RenderOptions {
            width: Some(width),
            ..RenderOptions::default()
        };
        let error = render_source(
            DiagramFormat::WaveDrom,
            "{signal:[{wave:'0'}]}",
            OutputFormat::Png,
            &options,
        )
        .expect_err("requested width must be bounded");
        assert!(
            matches!(error, RenderError::InvalidOption(_)),
            "width {width}"
        );
    }
}

#[test]
fn explicit_background_is_emitted() {
    let options = RenderOptions {
        background: Some("#ffffff".to_owned()),
        ..RenderOptions::default()
    };
    let rendered =
        render_source(DiagramFormat::D2, "a -> b", OutputFormat::Svg, &options).expect("render");
    assert!(rendered.svg.contains("data-canvas-background=\"true\""));
}

#[test]
fn unsafe_raster_scale_is_rejected() {
    let options = RenderOptions {
        scale: 100.0,
        ..RenderOptions::default()
    };
    let error = render_source(DiagramFormat::D2, "a -> b", OutputFormat::Png, &options)
        .expect_err("scale must be bounded");
    assert!(matches!(error, RenderError::InvalidOption(_)));
}

#[test]
fn svg_text_is_xml_escaped() {
    let rendered = render_source(
        DiagramFormat::D2,
        "a: \"<unsafe & visible>\"",
        OutputFormat::Svg,
        &RenderOptions::default(),
    )
    .expect("render");
    assert!(rendered.svg.contains("&lt;unsafe &amp; visible&gt;"));
    assert!(!rendered.svg.contains(">unsafe & visible<"));
}

#[test]
fn implemented_feature_surface_retains_value_sensitive_details() {
    let dbml = render_source(
        DiagramFormat::Dbml,
        include_str!("../e2e/fixtures/dbml_extended.dbml"),
        OutputFormat::Svg,
        &RenderOptions::default(),
    )
    .expect("extended DBML render");
    for cardinality in ["N:1", "1:N", "1:1", "N:N"] {
        assert!(dbml.svg.contains(cardinality), "missing {cardinality}");
    }

    let wavedrom = render_source(
        DiagramFormat::WaveDrom,
        include_str!("../e2e/fixtures/wavedrom_symbols.json5"),
        OutputFormat::Svg,
        &RenderOptions::default(),
    )
    .expect("WaveDrom symbol render");
    assert!(wavedrom.svg.contains("transfer"));
    assert!(wavedrom.svg.contains("End of symbols"));

    let pikchr = render_source(
        DiagramFormat::Pikchr,
        include_str!("../e2e/fixtures/pikchr_surface.pikchr"),
        OutputFormat::Svg,
        &RenderOptions::default(),
    )
    .expect("Pikchr surface render");
    assert!(
        pikchr.scene_height > 300.0,
        "direction changes must expand the canvas"
    );
    for label in ["Box", "Circle", "Diamond", "Cylinder", "Down", "Left", "Up"] {
        assert!(pikchr.svg.contains(label), "missing {label}");
    }
}

#[test]
fn d2_edge_operators_keep_their_arrowhead_cardinality() {
    for (source, expected_arrowheads) in
        [("a -> b", 1), ("a <- b", 1), ("a -- b", 0), ("a <-> b", 2)]
    {
        let rendered = render_source(
            DiagramFormat::D2,
            source,
            OutputFormat::Svg,
            &RenderOptions::default(),
        )
        .unwrap_or_else(|error| panic!("{source} failed: {error}"));
        assert_eq!(
            rendered.svg.matches("<polygon").count(),
            expected_arrowheads,
            "{source}"
        );
    }
}
