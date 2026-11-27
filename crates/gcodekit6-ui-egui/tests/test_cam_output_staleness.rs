//! The generated program stays while the page's input is the one it was made from, and is
//! withdrawn while it is not: compared by value, so putting the input back shows it again.

use egui_kittest::kittest::Queryable;
use gcodekit6_ui_egui::panes::cam_tools::CamTool;

use crate::test_cam_drill_press::{open_tool, pane};
use crate::test_designer_files_app::frames;

/// The Drill Press page with a program generated from its defaults.
fn generated() -> egui_kittest::Harness<'static, gcodekit6_ui_egui::GcodeKitApp> {
    let mut h = open_tool(CamTool::DrillPress);
    h.get_by_label("Generate").click();
    frames(&mut h, 2);
    assert!(pane(&h).output().is_some(), "Generate made a program");
    h
}

#[test]
fn test_cam_output_stays_while_the_input_is_unchanged() {
    let mut h = generated();
    let before = pane(&h).output().map(str::to_string);
    frames(&mut h, 10);
    assert_eq!(pane(&h).output().map(str::to_string), before);
    assert!(h.query_by_label("Generated G-Code").is_some());
}

#[test]
fn test_cam_output_hides_when_the_axis_count_changes() {
    let mut h = generated();
    pane(&h).set_num_axes(4);
    frames(&mut h, 2);
    assert_eq!(pane(&h).output(), None);
    pane(&h).set_num_axes(3);
    frames(&mut h, 2);
    assert!(pane(&h).output().is_some(), "same input again shows it");
}

#[test]
fn test_cam_output_returns_when_the_input_is_put_back() {
    let mut h = generated();
    let original = pane(&h).drill_press_mut().hole_diameter.clone();
    pane(&h).drill_press_mut().hole_diameter = "11".into();
    frames(&mut h, 2);
    assert_eq!(pane(&h).output(), None);
    pane(&h).drill_press_mut().hole_diameter = original;
    frames(&mut h, 2);
    assert!(pane(&h).output().is_some());
}
