use super::*;

#[test]
fn wide_same_row_labels_route_through_the_row_gap() {
    let left = Rect::new(42.0, 72.0, CARD_WIDTH, 211.0);
    let right = Rect::new(392.0, 72.0, CARD_WIDTH, 135.0);
    let points = connector_points(left, right, 216.0);
    assert_eq!(points.len(), 4);
    assert!(points[1].y > left.y + left.height);
    assert_eq!(points[1].y, points[2].y);
    let label = polyline_middle(&points);
    assert!(label.y > left.y + left.height);
    assert!(label.x > left.x + left.width && label.x < right.x);
}

#[test]
fn polyline_middle_uses_distance_instead_of_vertex_index() {
    let points = [
        Point::new(0.0, 0.0),
        Point::new(0.0, 10.0),
        Point::new(100.0, 10.0),
        Point::new(100.0, 20.0),
    ];
    assert_eq!(polyline_middle(&points), Point::new(50.0, 10.0));
}
