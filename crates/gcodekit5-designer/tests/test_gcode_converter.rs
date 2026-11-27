//! Tests for the toolpath segment helpers in `gcode_converter`.

use gcodekit5_designer::gcode_converter::{create_arc_segment, create_linear_segment, point_to_2d};
use gcodekit5_designer::toolpath::ToolpathSegmentType;

#[test]
fn test_point_conversion() {
    let p2d = point_to_2d(10.5, 20.5);
    assert_eq!(p2d.x, 10.5);
    assert_eq!(p2d.y, 20.5);
}

#[test]
fn test_linear_segment_creation() {
    let segment = create_linear_segment(0.0, 0.0, 0.0, 10.0, 10.0, -5.0, false, 100.0, 3000);

    assert_eq!(segment.segment_type, ToolpathSegmentType::LinearMove);
    assert_eq!(segment.z_depth, Some(-5.0));
    assert_eq!(segment.end.x, 10.0);
    assert_eq!(segment.end.y, 10.0);
}

#[test]
fn test_rapid_segment_creation() {
    let segment = create_linear_segment(0.0, 0.0, 5.0, 10.0, 10.0, 5.0, true, 100.0, 3000);

    assert_eq!(segment.segment_type, ToolpathSegmentType::RapidMove);
}

#[test]
fn test_arc_segment_creation() {
    let segment = create_arc_segment(0.0, 0.0, -5.0, 10.0, 0.0, -5.0, 5.0, 0.0, true, 100.0, 3000);

    assert_eq!(segment.segment_type, ToolpathSegmentType::ArcCW);
    assert!(segment.center.is_some());
    assert_eq!(segment.z_depth, Some(-5.0));
}
