//! Locks the shared helpers in `gcodekit5_designer::geometry` (audit T36, DESIGNER-9):
//! point-to-segment distance with its degenerate-length threshold, even-odd
//! point-in-polygon, and the two polygon preparers with their per-caller tolerances.

use gcodekit5_designer::geometry::{
    point_in_polygon, point_segment_distance, prepare_polygon, ray_crosses_edge, to_pline,
};
use gcodekit5_designer::model::{DesignLine, DesignPolygon, DesignerShape, Point};

fn pts(coords: &[(f64, f64)]) -> Vec<Point> {
    coords.iter().map(|&(x, y)| Point::new(x, y)).collect()
}

fn coords(pline: &cavalier_contours::polyline::Polyline) -> Vec<(f64, f64)> {
    pline.vertex_data.iter().map(|v| (v.x, v.y)).collect()
}

#[test]
fn test_point_segment_distance_projects_and_clamps() {
    let a = Point::new(0.0, 0.0);
    let b = Point::new(10.0, 0.0);
    assert_eq!(
        point_segment_distance(&Point::new(5.0, 3.0), &a, &b, 0.0),
        3.0
    );
    assert_eq!(
        point_segment_distance(&Point::new(13.0, 4.0), &a, &b, 0.0),
        5.0
    );
    assert_eq!(
        point_segment_distance(&Point::new(-3.0, 4.0), &a, &b, 0.0),
        5.0
    );
    // A zero-length segment is the point `a`.
    assert_eq!(
        point_segment_distance(&Point::new(3.0, 4.0), &a, &a, 0.0),
        5.0
    );
}

#[test]
fn test_point_segment_distance_threshold_treats_short_segment_as_point() {
    let a = Point::new(0.0, 0.0);
    let b = Point::new(1e-10, 0.0);
    let p = Point::new(2e-10, 0.0);
    // Without a threshold the far end point is the nearest point.
    let projected = point_segment_distance(&p, &a, &b, 0.0);
    assert!((projected - 1e-10).abs() < 1e-20, "{projected}");
    // With the hole-pocket threshold the segment collapses to `a`.
    assert_eq!(point_segment_distance(&p, &a, &b, 1e-18), 2e-10);
}

#[test]
fn test_line_distance_to_point_uses_segment_distance() {
    let line = DesignLine::new(Point::new(0.0, 0.0), Point::new(10.0, 0.0));
    assert_eq!(line.distance_to_point(&Point::new(5.0, -2.0)), 2.0);
    assert_eq!(line.distance_to_point(&Point::new(14.0, 3.0)), 5.0);
}

#[test]
fn test_point_in_polygon_even_odd() {
    // Concave "U" shape.
    let u = pts(&[
        (0.0, 0.0),
        (30.0, 0.0),
        (30.0, 30.0),
        (20.0, 30.0),
        (20.0, 10.0),
        (10.0, 10.0),
        (10.0, 30.0),
        (0.0, 30.0),
    ]);
    assert!(point_in_polygon(&Point::new(5.0, 20.0), &u));
    assert!(point_in_polygon(&Point::new(15.0, 5.0), &u));
    assert!(!point_in_polygon(&Point::new(15.0, 20.0), &u));
    assert!(!point_in_polygon(&Point::new(40.0, 5.0), &u));
    assert!(!point_in_polygon(&Point::new(1.0, 1.0), &[]));
}

#[test]
fn test_ray_crosses_edge() {
    let p = Point::new(0.0, 5.0);
    assert!(ray_crosses_edge(
        &p,
        &Point::new(3.0, 0.0),
        &Point::new(3.0, 10.0)
    ));
    assert!(!ray_crosses_edge(
        &p,
        &Point::new(-3.0, 0.0),
        &Point::new(-3.0, 10.0)
    ));
    assert!(!ray_crosses_edge(
        &p,
        &Point::new(3.0, 6.0),
        &Point::new(3.0, 10.0)
    ));
}

#[test]
fn test_polygon_contains_point_unchanged() {
    let hex = DesignPolygon::new(Point::new(50.0, 50.0), 10.0, 6);
    assert!(hex.contains_point(Point::new(50.0, 50.0), 0.0));
    assert!(hex.contains_point(Point::new(58.0, 50.0), 0.0));
    assert!(!hex.contains_point(Point::new(50.0, 59.5), 0.0));
    assert!(!hex.contains_point(Point::new(70.0, 50.0), 0.0));
}

#[test]
fn test_prepare_polygon_dedupes_and_orients_clockwise() {
    // CCW square with a near-duplicate vertex and a repeated closing vertex.
    let square = pts(&[
        (0.0, 0.0),
        (10.0, 0.0),
        (10.005, 0.0),
        (10.0, 10.0),
        (0.0, 10.0),
        (0.0, 0.0),
    ]);
    let pline = prepare_polygon(&square, 0.01);
    assert!(pline.is_closed);
    assert_eq!(
        coords(&pline),
        vec![(0.0, 10.0), (10.0, 10.0), (10.0, 0.0), (0.0, 0.0)]
    );
    // A tighter tolerance keeps the near-duplicate vertex.
    assert_eq!(prepare_polygon(&square, 0.001).vertex_data.len(), 5);
}

#[test]
fn test_prepare_polygon_keeps_degenerate_input_and_drops_one_closing_vertex() {
    let line = prepare_polygon(&pts(&[(0.0, 0.0), (5.0, 0.0)]), 0.01);
    assert_eq!(coords(&line), vec![(0.0, 0.0), (5.0, 0.0)]);
    // Only one trailing vertex near the start is dropped.
    let tail = pts(&[
        (0.0, 0.0),
        (10.0, 0.0),
        (10.0, 10.0),
        (0.0, 0.008),
        (0.008, 0.0),
    ]);
    assert_eq!(prepare_polygon(&tail, 0.01).vertex_data.len(), 4);
}

#[test]
fn test_to_pline_orientation_and_degenerate_input() {
    let cw = pts(&[(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0)]);
    let ccw = to_pline(&cw, true, 0.01).expect("square is valid");
    assert!(ccw.is_closed);
    assert_eq!(
        coords(&ccw),
        vec![(10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)]
    );
    let kept = to_pline(&cw, false, 0.01).expect("square is valid");
    assert_eq!(
        coords(&kept),
        vec![(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0)]
    );
    assert!(to_pline(&pts(&[(0.0, 0.0), (1.0, 1.0)]), true, 0.01).is_none());
    assert!(to_pline(&pts(&[(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)]), true, 0.01).is_none());
}

#[test]
fn test_to_pline_drops_every_trailing_vertex_near_start() {
    let tail = pts(&[
        (0.0, 0.0),
        (10.0, 0.0),
        (10.0, 10.0),
        (0.0, 0.008),
        (0.008, 0.0),
    ]);
    let pline = to_pline(&tail, true, 0.01).expect("triangle is valid");
    assert_eq!(pline.vertex_data.len(), 3);
}
