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
