//! The Drill Press page: GTK's defaults render, invalid input is shown at the field and
//! disables the actions, and the program is byte-identical to the GTK page's for the same
//! input (fixtures captured from the GTK path by `capture_cam_fixtures`).

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::panes::cam_tools::drill_press::DrillPressParams;
use gcodekit5_ui_egui::panes::cam_tools::{CamTool, CamToolsPane};

use crate::test_cam_tools::cam_app;
use crate::test_designer_files_app::{default_state, frames};

/// A fixture captured from the GTK page.
pub fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/cam")
        .join(name);
    std::fs::read_to_string(&path).expect("read a CAM fixture")
}

/// The app with `tool`'s page open, picked as the user would.
pub fn open_tool(tool: CamTool) -> Harness<'static, GcodeKitApp> {
    let (mut h, _script) = cam_app(default_state());
    let title = h
        .state()
        .state()
        .translator
        .tr(tool.title_msgid())
        .into_owned();
    h.get_by_label(&title).click();
    frames(&mut h, 1);
    h.get_by_label("Open").click();
    frames(&mut h, 2);
    assert_eq!(h.state().cam_tools().borrow().open_tool(), Some(tool));
    h
}

pub fn pane<'a>(h: &'a Harness<'static, GcodeKitApp>) -> std::cell::RefMut<'a, CamToolsPane> {
    h.state().cam_tools().borrow_mut()
}

#[test]
fn test_cam_drill_press_default_page_renders() {
    let h = open_tool(CamTool::DrillPress);
    for label in [
        "Hole Diameter:",
        "Tool Diameter:",
        "Center X:",
        "Top Z (Surface):",
        "Peck Depth (0 for none):",
        "Plunge Rate (mm/min):",
        "Safe Z:",
        "Generate",
        "Send to Editor",
        "Preview",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label} not shown");
    }
    assert!(pane(&h).drill_press_mut().parameters().is_ok());
}

#[test]
fn test_cam_drill_press_generate_matches_gtk_defaults() {
    let mut h = open_tool(CamTool::DrillPress);
    h.get_by_label("Generate").click();
    frames(&mut h, 2);
    assert_eq!(
        pane(&h).output(),
        Some(fixture("drill_press_default.gcode").as_str())
    );
    let page = DrillPressParams::with_center(MeasurementSystem::Metric, (100.0, 100.0));
    assert_eq!(page.generate(), Ok(fixture("drill_press_default.gcode")));
}

#[test]
fn test_cam_drill_press_generate_matches_gtk_custom() {
    let mut page = DrillPressParams::with_center(MeasurementSystem::Metric, (0.0, 0.0));
    page.hole_diameter = "12".into();
    page.tool_diameter = "6".into();
    page.top_z = "0".into();
    page.bottom_z = "-5".into();
    page.peck_depth = "0".into();
    page.safe_z = "3".into();
    page.x = "50".into();
    page.y = "25".into();
    page.plunge_rate = "200".into();
    page.feed_rate = "800".into();
    page.spindle_speed = "12000".into();
    page.home_before = true;
    assert_eq!(page.generate(), Ok(fixture("drill_press_custom.gcode")));
}

#[test]
fn test_cam_drill_press_send_to_editor_hands_over_the_gtk_program() {
    let mut h = open_tool(CamTool::DrillPress);
    h.get_by_label("Send to Editor").click();
    frames(&mut h, 3);
    assert_eq!(
        h.state().editor().borrow().buffer().text(),
        fixture("drill_press_default.gcode")
    );
}

/// Makes one field of the page unsafe.
type Breaker = fn(&mut DrillPressParams);

/// Each unsafe value is reported at its field, and Generate makes nothing.
#[test]
fn test_cam_drill_press_invalid_input_shows_error_and_disables_generate() {
    let cases: [(Breaker, &str); 6] = [
        (|p| p.peck_depth = "-1".into(), "Must not be negative"),
        (|p| p.tool_diameter = "0".into(), "Must be greater than 0"),
        (|p| p.hole_diameter = "-3".into(), "Must be greater than 0"),
        (|p| p.bottom_z = "1".into(), "Must be below Top Z"),
        (|p| p.safe_z = "-1".into(), "Must be above Top Z"),
        (|p| p.feed_rate = "fast".into(), "Not a number"),
    ];
    for (break_it, message) in cases {
        let mut h = open_tool(CamTool::DrillPress);
        break_it(pane(&h).drill_press_mut());
        frames(&mut h, 2);
        assert!(h.query_by_label(message).is_some(), "{message} not shown");
        h.get_by_label("Generate").click();
        frames(&mut h, 2);
        assert_eq!(
            pane(&h).output(),
            None,
            "{message}: Generate made a program"
        );
    }
}

#[test]
fn test_cam_drill_press_follows_the_measurement_system() {
    let mut page = DrillPressParams::with_center(MeasurementSystem::Metric, (100.0, 100.0));
    page.set_units(MeasurementSystem::Imperial);
    assert_eq!(page.hole_diameter, "0.394");
    let params = page.parameters().expect("valid");
    assert!(
        (params.bottom_z - -10.0).abs() < 0.01,
        "{}",
        params.bottom_z
    );
}

/// Typing in a page's field edits it and runs no shortcut ("+" is the visualizer's Zoom
/// In).
#[test]
fn test_cam_field_typing_runs_no_shortcut() {
    let mut h = open_tool(CamTool::DrillPress);
    pane(&h).drill_press_mut().hole_diameter = "12.345".into();
    frames(&mut h, 2);
    let field = || {
        egui_kittest::kittest::By::new()
            .role(egui::accesskit::Role::TextInput)
            .value("12.345")
    };
    h.get(field()).click();
    frames(&mut h, 1);
    h.get(field()).type_text("+");
    frames(&mut h, 2);
    assert_eq!(pane(&h).drill_press_mut().hole_diameter, "12.345+");
    assert_eq!(h.state().last_dispatched(), None);
}
