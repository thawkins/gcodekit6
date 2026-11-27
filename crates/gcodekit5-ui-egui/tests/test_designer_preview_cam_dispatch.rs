//! The toolpath preview builds through the designer crate's CAM dispatch, so it follows
//! the export's per-shape laser parameters (feed and power) as well as its geometry.

use gcodekit5_designer::designer_state::MachineMode;
use gcodekit5_designer::model::Shape;
use gcodekit5_designer::toolpath::ToolpathSegmentType;

use crate::test_designer_canvas::{harness, pane_with_rects};
use crate::test_designer_toolpath::wait_for_preview;

#[test]
fn test_preview_uses_shape_laser_feed_like_export() {
    let mut pane = pane_with_rects(&[(20.0, 20.0, 40.0, 30.0)]);
    let state = pane.state_mut();
    state.set_machine_mode(MachineMode::Laser2D);
    state.tool_settings.feed_rate = 1234.0;
    for obj in state.canvas.shapes_mut() {
        obj.use_global_laser = false;
        obj.laser_params.use_global = false;
        obj.laser_params.feed_rate = 777.0;
        if let Shape::Rectangle(r) = &mut obj.shape {
            r.laser_params.use_global = false;
        }
    }
    let mut h = harness(pane);
    h.state_mut().state_mut().show_toolpaths = true;
    wait_for_preview(&mut h);
    let feeds: Vec<f64> = h
        .state()
        .preview_toolpaths()
        .iter()
        .flat_map(|tp| tp.segments.iter())
        .filter(|s| s.segment_type != ToolpathSegmentType::RapidMove)
        .map(|s| s.feed_rate)
        .collect();
    assert!(!feeds.is_empty(), "the rectangle is previewed");
    assert!(feeds.iter().all(|&f| (f - 777.0).abs() < 1e-9), "{feeds:?}");
}
