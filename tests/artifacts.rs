const ARTIFACTS: &[(&str, &str, &[u8])] = &[
    (
        "schema",
        include_str!("../examples/rendered/schema.svg"),
        include_bytes!("../examples/rendered/schema.png"),
    ),
    (
        "timing",
        include_str!("../examples/rendered/timing.svg"),
        include_bytes!("../examples/rendered/timing.png"),
    ),
    (
        "architecture",
        include_str!("../examples/rendered/architecture.svg"),
        include_bytes!("../examples/rendered/architecture.png"),
    ),
    (
        "workspace",
        include_str!("../examples/rendered/workspace.svg"),
        include_bytes!("../examples/rendered/workspace.png"),
    ),
    (
        "model",
        include_str!("../examples/rendered/model.svg"),
        include_bytes!("../examples/rendered/model.png"),
    ),
    (
        "classes",
        include_str!("../examples/rendered/classes.svg"),
        include_bytes!("../examples/rendered/classes.png"),
    ),
    (
        "flow",
        include_str!("../examples/rendered/flow.svg"),
        include_bytes!("../examples/rendered/flow.png"),
    ),
];

#[test]
fn checked_in_gallery_artifacts_are_accessible_and_decodable() {
    for (name, svg, png) in ARTIFACTS {
        assert!(svg.starts_with("<svg"), "{name}");
        assert!(svg.contains("role=\"img\""), "{name}");
        assert!(svg.contains("<title"), "{name}");
        assert!(!svg.contains("NaN"), "{name}");

        let pixmap = resvg::tiny_skia::Pixmap::decode_png(png)
            .unwrap_or_else(|error| panic!("{name} PNG did not decode: {error}"));
        assert!(pixmap.width() > 200, "{name}");
        assert!(pixmap.height() > 200, "{name}");
        assert_eq!(
            pixmap.pixel(0, 0).expect("corner pixel").alpha(),
            0,
            "{name} PNG canvas must be transparent"
        );
    }
}
