//! Tests for `model::circular_array_step` and `Shape::rotate`.

use gcodekit5_designer::model::{DesignCircle, DesignerShape, Point, Shape, circular_array_step};

#[test]
fn test_circular_array_step_full_circle_has_count_gaps() {
    // 6 items around a full circle: the last gap closes the circle.
    assert!((circular_array_step(360.0, 6) - 60.0).abs() < 1e-9);
    assert!((circular_array_step(-360.0, 4) + 90.0).abs() < 1e-9);
}

#[test]
fn test_circular_array_step_partial_arc_ends_on_the_arc_end() {
    // 3 items over 90 degrees sit at 0, 45 and 90: two gaps.
    let step = circular_array_step(90.0, 3);
    assert!((step - 45.0).abs() < 1e-9);
    assert!(
        (step * 2.0 - 90.0).abs() < 1e-9,
        "last item must land on the arc end"
    );
}

#[test]
fn test_circular_array_step_degenerate_counts() {
    assert_eq!(circular_array_step(180.0, 0), 0.0);
    assert_eq!(circular_array_step(180.0, 1), 0.0);
}

#[test]
fn test_shape_rotate_uses_degrees_not_radians() {
    let mut shape = Shape::Circle(DesignCircle::new(Point::new(10.0, 0.0), 2.0));
    shape.rotate(90.0, 0.0, 0.0);

    let (x1, y1, x2, y2) = shape.bounds();
    let cx = (x1 + x2) / 2.0;
    let cy = (y1 + y2) / 2.0;

    assert!(cx.abs() < 1e-5, "expected cx ~ 0.0, got {cx}");
    assert!((cy - 10.0).abs() < 1e-5, "expected cy ~ 10.0, got {cy}");
}
