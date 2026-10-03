use super::*;

fn empty_svg(width: f32, height: f32) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"/>"#)
}

#[test]
fn explicit_width_preserves_pixels_and_scaled_height() {
    // Cover float-rounding witnesses, fractional source dimensions, the
    // inclusive effective-scale bounds, and the inclusive dimension cap.
    for (source_width, source_height, requested, expected_height) in [
        (660.0, 220.0, 43, 15),
        (660.0, 220.0, 44, 15),
        (660.0, 220.0, 45, 15),
        (660.0, 220.0, 701, 234),
        (660.0, 220.0, 702, 235),
        (660.0, 220.0, 703, 235),
        (100.5, 20.25, 100, 21),
        (20.0, 10.0, 1, 1),
        (20.0, 10.0, 320, 160),
        (2048.0, 1.0, 32_768, 16),
    ] {
        let svg = empty_svg(source_width, source_height);
        let raster = svg_to_png(&svg, 1.0, Some(requested)).unwrap_or_else(|error| {
            panic!("{source_width}x{source_height} at width {requested}: {error}")
        });
        assert_eq!((raster.width, raster.height), (requested, expected_height));
        let decoded = resvg::tiny_skia::Pixmap::decode_png(&raster.bytes).expect("valid PNG");
        assert_eq!(
            (decoded.width(), decoded.height()),
            (requested, expected_height),
            "{source_width}x{source_height} at width {requested}"
        );
    }
}

#[test]
fn scale_only_still_ceil_rounds_both_dimensions() {
    let raster = svg_to_png(&empty_svg(100.5, 20.25), 1.5, None).expect("render by scale");
    assert_eq!((raster.width, raster.height), (151, 31));
    let decoded = resvg::tiny_skia::Pixmap::decode_png(&raster.bytes).expect("valid PNG");
    assert_eq!((decoded.width(), decoded.height()), (151, 31));
}

#[test]
fn explicit_width_keeps_effective_scale_limits() {
    let svg = empty_svg(660.0, 220.0);
    for requested in [0, 32, 10_561, u32::MAX] {
        let Err(RenderError::InvalidOption(message)) = svg_to_png(&svg, 1.0, Some(requested))
        else {
            panic!("width {requested} must fail the effective-scale check");
        };
        assert_eq!(message, "effective PNG scale must be between 0.05 and 16.0");
    }
}

#[test]
fn explicit_width_keeps_allocation_limits() {
    for (source_width, source_height, requested, expected_error) in [
        (
            32_769.0,
            1.0,
            32_769,
            "rendered image exceeds 32768px dimension limit (32769x1)",
        ),
        (
            1.0,
            32_769.0,
            1,
            "rendered image exceeds 32768px dimension limit (1x32769)",
        ),
        (
            10_001.0,
            10_000.0,
            10_001,
            "rendered image exceeds 100000000 pixel limit (10001x10000)",
        ),
    ] {
        let svg = empty_svg(source_width, source_height);
        let Err(RenderError::Png(message)) = svg_to_png(&svg, 1.0, Some(requested)) else {
            panic!("{source_width}x{source_height} at width {requested} must exceed the budget");
        };
        assert_eq!(message, expected_error);
    }
}
