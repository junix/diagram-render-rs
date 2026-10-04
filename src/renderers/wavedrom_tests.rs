use std::collections::BTreeMap;

use super::*;

#[test]
fn negative_clock_continuation_keeps_its_polarity() {
    let lane = WaveLane {
        name: None,
        wave: Some("n.".to_owned()),
        data: Vec::new(),
        node: None,
        phase: None,
        period: None,
        extra: serde_json::Map::new(),
    };
    let mut scene = Scene::new(200.0, 100.0, "clock");
    draw_lane(
        &mut scene,
        &lane,
        0.0,
        50.0,
        2,
        &Theme::light(),
        &mut BTreeMap::new(),
    );
    let starts = scene
        .primitives
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Polyline { points, .. } if points.len() == 5 => Some(points[0].y),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(starts, vec![37.0, 37.0]);
}

#[test]
fn edge_parser_separates_node_names_from_the_label() {
    assert_eq!(edge_parts("a~>b transfer"), Some(('a', 'b', "transfer")));
    assert_eq!(edge_parts("a-b"), Some(('a', 'b', "")));
    assert_eq!(edge_parts("a"), None);
}

fn register_fields(widths: &[Option<u64>]) -> Vec<WaveRegisterField> {
    widths
        .iter()
        .map(|&bits| WaveRegisterField {
            bits,
            name: None,
            attr: None,
            field_type: None,
            extra: serde_json::Map::new(),
        })
        .collect()
}

fn register_geometry(widths: &[Option<u64>]) -> (Vec<Rect>, Vec<String>) {
    let mut scene = Scene::new(660.0, 220.0, "register");
    draw_register(
        &mut scene,
        &register_fields(widths),
        74.0,
        660.0,
        &Theme::light(),
    )
    .expect("valid register");
    scene.validate().expect("finite geometry");
    let rects: Vec<_> = scene
        .primitives
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Rect { rect, .. } => Some(*rect),
            _ => None,
        })
        .collect();
    assert_eq!(rects.len(), widths.len());
    for rect in &rects {
        assert!(rect.x >= 34.0 && rect.x <= 626.0, "{rect:?}");
        assert!(rect.width >= 0.0 && rect.height >= 0.0, "{rect:?}");
        assert!(rect.x + rect.width <= 626.001, "{rect:?}");
    }
    for pair in rects.windows(2) {
        assert!((pair[0].x + pair[0].width - pair[1].x).abs() < 0.001);
    }
    if let Some(last) = rects.last() {
        assert!((last.x + last.width - 626.0).abs() < 0.001);
        assert!((rects.iter().map(|rect| rect.width).sum::<f32>() - 592.0).abs() < 0.1);
    }
    let ranges = scene
        .primitives
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Text { at, text, .. } if at.y == 166.0 => Some(text.clone()),
            _ => None,
        })
        .collect();
    (rects, ranges)
}

#[test]
fn register_missing_zero_and_positive_widths_use_one_consistent_total() {
    for width in [None, Some(0), Some(1)] {
        let (rects, ranges) = register_geometry(&[width]);
        assert_eq!(rects[0].width, 592.0);
        assert_eq!(ranges, ["0"]);
    }
    let (rects, ranges) = register_geometry(&[Some(0), Some(0), Some(0)]);
    assert!(
        rects
            .iter()
            .all(|rect| (rect.width - 592.0 / 3.0).abs() < 0.001)
    );
    assert_eq!(ranges, ["2", "1", "0"]);

    let (rects, ranges) = register_geometry(&[Some(0), None, Some(2)]);
    assert_eq!(
        rects.iter().map(|rect| rect.width).collect::<Vec<_>>(),
        [148.0, 148.0, 296.0]
    );
    assert_eq!(ranges, ["3", "2", "1:0"]);

    let (rects, ranges) = register_geometry(&[Some(2), Some(6)]);
    assert_eq!(
        rects.iter().map(|rect| rect.width).collect::<Vec<_>>(),
        [148.0, 444.0]
    );
    assert_eq!(ranges, ["7:6", "5:0"]);
}

#[test]
fn register_empty_and_extreme_valid_widths_stay_bounded() {
    let (rects, ranges) = register_geometry(&[]);
    assert!(rects.is_empty() && ranges.is_empty());
    let (_, ranges) = register_geometry(&[Some(u64::MAX)]);
    assert_eq!(ranges, [format!("{}:0", u64::MAX - 1)]);
    let (_, ranges) = register_geometry(&[Some(u64::MAX - 1), Some(1)]);
    assert_eq!(ranges, [format!("{}:1", u64::MAX - 1), "0".to_owned()]);
    register_geometry(&[Some(1), Some(u64::MAX - 1)]);
    register_geometry(&vec![Some(1); 4096]);
    register_geometry(&[Some(123456789), Some(987654321), Some(1)]);
}

#[test]
fn register_overflow_is_rejected_before_drawing_any_fields() {
    for tail in [Some(1), Some(0), None, Some(u64::MAX)] {
        let mut scene = Scene::new(660.0, 220.0, "register");
        let error = draw_register(
            &mut scene,
            &register_fields(&[Some(u64::MAX), tail]),
            74.0,
            660.0,
            &Theme::light(),
        )
        .expect_err("normalized sum exceeds u64::MAX");
        assert!(matches!(error, RenderError::InvalidScene(_)));
        assert!(scene.primitives.is_empty());
    }
}
