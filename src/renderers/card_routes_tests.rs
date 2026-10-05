use super::*;

fn cards(count: usize) -> BTreeMap<String, Rect> {
    let columns = ((count as f32).sqrt().ceil() as usize).clamp(1, 4);
    (0..count)
        .map(|i| {
            (
                format!("n{i}"),
                Rect::new(
                    42.0 + (i % columns) as f32 * 350.0,
                    72.0 + (i / columns) as f32 * 179.0,
                    258.0,
                    97.0,
                ),
            )
        })
        .collect()
}
fn connector(from: &str, to: &str) -> Connector {
    Connector {
        from: from.into(),
        to: to.into(),
        label: None,
        kind: super::super::ConnectorKind::Directed,
        dashed: false,
    }
}
fn route(
    router: &mut Router<'_>,
    index: usize,
    from: &str,
    to: &str,
    width: f32,
) -> Result<(Vec<Point>, Option<Point>)> {
    let a = router.cards[from];
    let b = router.cards[to];
    router.route(
        index,
        &connector(from, to),
        a,
        b,
        width,
        super::super::connector_points(a, b, width),
    )
}

#[test]
fn skipped_column_route_and_label_avoid_middle_card() {
    let cards = cards(5);
    let mut router = Router::new(&cards, 1042.0, 432.0, 1).unwrap();
    let (points, label) = route(&mut router, 0, "n0", "n2", 67.7).unwrap();
    assert!(
        points
            .windows(2)
            .all(|p| !crosses(p[0], p[1], expand(cards["n1"], 8.0)))
    );
    assert!(!overlaps(
        label_rect(label.unwrap(), 67.7),
        expand(cards["n1"], 8.0)
    ));
    assert!(
        points
            .windows(2)
            .all(|p| p[0].x == p[1].x || p[0].y == p[1].y)
    );
    assert!(outward_port(points[0], points[1], cards["n0"]));
    assert!(outward_port(
        *points.last().unwrap(),
        points[points.len() - 2],
        cards["n2"]
    ));
}

#[test]
fn clear_legacy_geometry_is_exactly_retained() {
    let cards = cards(5);
    let mut router = Router::new(&cards, 1042.0, 432.0, 1).unwrap();
    let legacy = super::super::connector_points(cards["n0"], cards["n1"], 46.0);
    let expected = (legacy.clone(), Some(polyline_middle(&legacy)));
    assert_eq!(route(&mut router, 0, "n0", "n1", 46.0).unwrap(), expected);
}

#[test]
fn clear_line_with_hidden_label_is_repaired() {
    let cards = cards(2);
    let mut router = Router::new(&cards, 692.0, 253.0, 1).unwrap();
    // Deliberately force the short direct legacy route with an over-wide pill.
    let a = cards["n0"];
    let b = cards["n1"];
    let legacy = vec![Point::new(300.0, 120.5), Point::new(392.0, 120.5)];
    let (points, label) = router
        .route(0, &connector("n0", "n1"), a, b, 230.0, legacy.clone())
        .unwrap();
    assert_ne!(points, legacy);
    assert!(label.unwrap().y > 169.0);
}

#[test]
fn repeated_reverse_and_self_routes_are_deterministic_and_separate() {
    for pair in [("n0", "n2"), ("n0", "n0"), ("n0", "n1")] {
        let make = || {
            let cards = cards(5);
            let mut router = Router::new(&cards, 1042.0, 432.0, 3).unwrap();
            let a = route(&mut router, 0, pair.0, pair.1, 46.0).unwrap();
            let b = route(&mut router, 1, pair.1, pair.0, 46.0).unwrap();
            let c = route(&mut router, 2, pair.0, pair.1, 46.0).unwrap();
            assert!(!same_route(&a.0, &b.0));
            assert!(!same_route(&a.0, &c.0));
            assert!(!same_route(&b.0, &c.0));
            for (i, left) in router.labels.iter().enumerate() {
                for right in router.labels.iter().skip(i + 1) {
                    assert!(!overlaps(*left, *right));
                }
            }
            (a, b, c)
        };
        assert_eq!(make(), make());
    }
}

#[test]
fn one_card_wide_self_loop_stays_below_title_and_inside_canvas() {
    let cards = cards(1);
    let mut router = Router::new(&cards, 342.0, 253.0, 1).unwrap();
    let (points, at) = route(&mut router, 0, "n0", "n0", 250.0).unwrap();
    assert!(points.iter().all(|p| contains(router.frame, *p)));
    let label = expand(label_rect(at.unwrap(), 250.0), 1.0);
    assert!(contains(router.frame, Point::new(label.x, label.y)));
    assert!(contains(
        router.frame,
        Point::new(label.x + label.width, label.y + label.height)
    ));
}

#[test]
fn all_pairs_of_twelve_cards_clear_every_nonendpoint_and_escape_normally() {
    let mut cards = cards(12);
    // Uneven card heights, preserving the row clearance of the actual layout.
    for (i, card) in cards.values_mut().enumerate() {
        card.height = if i % 3 == 0 { 70.0 } else { 97.0 };
    }
    for from in cards.keys() {
        for to in cards.keys() {
            let mut router = Router::new(&cards, 1392.0, 611.0, 1).unwrap();
            let (points, _) = route(&mut router, 0, from, to, 46.0)
                .unwrap_or_else(|e| panic!("{from}->{to}: {e}"));
            for (id, card) in &cards {
                if id != from && id != to {
                    assert!(
                        points
                            .windows(2)
                            .all(|p| !crosses(p[0], p[1], expand(*card, 8.0))),
                        "{from}->{to} crosses {id}"
                    );
                }
            }
        }
    }
}

#[test]
fn route_and_label_fail_closed_on_exhausted_work() {
    let cards = cards(5);
    let mut router = Router::new(&cards, 1042.0, 432.0, 1).unwrap();
    router.work = 0;
    let error = route(&mut router, 0, "n0", "n2", 46.0)
        .unwrap_err()
        .to_string();
    assert!(error.contains("card connector 1 (n0 -> n2)"));
    assert!(error.contains("work budget"));
}

#[test]
fn routing_limits_precede_allocation_but_allow_many_unconnected_cards() {
    let cards = cards(MAX_CARDS + 1);
    assert!(Router::new(&cards, 1400.0, 12000.0, 1).is_err());
    assert!(Router::new(&cards, 1400.0, 12000.0, 0).is_ok());
    assert!(Router::new(&BTreeMap::new(), 100.0, 100.0, MAX_CONNECTORS + 1).is_err());
    let obstacles = (0..100)
        .map(|i| Rect::new(10.0 + i as f32 * 3.0, 10.0 + i as f32 * 3.0, 1.0, 1.0))
        .collect::<Vec<_>>();
    let mut total = TOTAL_WORK;
    let mut budget = Budget {
        edge: EDGE_WORK,
        total: &mut total,
    };
    assert_eq!(
        visibility_route(
            Point::new(1.0, 1.0),
            Point::new(499.0, 499.0),
            &obstacles,
            &[],
            Rect::new(0.0, 0.0, 500.0, 500.0),
            &mut budget
        ),
        Err("routing visibility-grid budget exceeded")
    );
}

#[test]
fn no_route_has_an_explicit_result_and_diagonal_intersection_is_exact() {
    let mut total = TOTAL_WORK;
    let mut budget = Budget {
        edge: EDGE_WORK,
        total: &mut total,
    };
    let result = visibility_route(
        Point::new(20.0, 20.0),
        Point::new(80.0, 80.0),
        &[Rect::new(10.0, 10.0, 30.0, 30.0)],
        &[],
        Rect::new(0.0, 0.0, 100.0, 100.0),
        &mut budget,
    )
    .unwrap();
    assert_eq!(result, None);
    let r = Rect::new(10.0, 10.0, 10.0, 10.0);
    assert!(crosses(Point::new(0.0, 0.0), Point::new(30.0, 30.0), r));
    assert!(!crosses(Point::new(0.0, 0.0), Point::new(30.0, 9.0), r));
    assert!(!crosses(Point::new(0.0, 10.0), Point::new(30.0, 10.0), r));
}

#[test]
fn wide_labels_try_alternate_ports_for_variable_height_cross_row_cards() {
    for count in [4_usize, 5, 6, 12] {
        let columns = ((count as f32).sqrt().ceil() as usize).clamp(1, 4);
        let heights = (0..count)
            .map(|i| 78.0 + ((i * 7 + 1) % 13).max(1) as f32 * 19.0)
            .collect::<Vec<_>>();
        let mut positions = BTreeMap::new();
        let mut y = 72.0;
        for row in 0..count.div_ceil(columns) {
            let mut height = 0.0_f32;
            for column in 0..columns {
                let i = row * columns + column;
                if i >= count {
                    break;
                }
                height = height.max(heights[i]);
                positions.insert(
                    format!("n{i}"),
                    Rect::new(42.0 + 350.0 * column as f32, y, 258.0, heights[i]),
                );
            }
            y += height + 82.0;
        }
        let width = 84.0 + columns as f32 * 258.0 + (columns - 1) as f32 * 92.0;
        for from in positions.keys() {
            for to in positions.keys() {
                let mut router = Router::new(&positions, width, y + 2.0, 1).unwrap();
                route(&mut router, 0, from, to, 250.0)
                    .unwrap_or_else(|e| panic!("{count} cards {from}->{to}: {e}"));
            }
        }
    }
}

#[test]
fn wide_pairs_and_loops_use_free_lanes_and_mixed_ports() {
    for (first, second) in [
        (("n0", "n1"), ("n0", "n1")),
        (("n0", "n1"), ("n1", "n0")),
        (("n0", "n1"), ("n0", "n2")),
        (("n0", "n2"), ("n0", "n1")),
    ] {
        let cards = cards(4);
        let mut router = Router::new(&cards, 692.0, 432.0, 2).unwrap();
        route(&mut router, 0, first.0, first.1, 250.0).unwrap();
        route(&mut router, 1, second.0, second.1, 250.0)
            .unwrap_or_else(|e| panic!("{first:?} then {second:?}: {e}"));
    }
}

#[test]
fn an_occluding_wide_loop_can_exhaust_accessible_ports_explicitly() {
    let cards = cards(4);
    let mut router = Router::new(&cards, 692.0, 432.0, 2).unwrap();
    route(&mut router, 0, "n0", "n0", 250.0).unwrap();
    let error = route(&mut router, 1, "n0", "n0", 250.0)
        .unwrap_err()
        .to_string();
    assert!(error.contains("card connector 2 (n0 -> n0)"));
    assert!(error.contains("no clear"));
}
