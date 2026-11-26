//! The Drill Press page: GTK's defaults render, invalid input is shown at the field and
//! disables the actions, and the program is byte-identical to the GTK page's for the same
//! input (fixtures captured from the GTK path by `capture_cam_fixtures`).
//!
//! The fixtures in `tests/fixtures/cam/` are frozen references: `capture_cam_fixtures`, which
//! wrote them from the GTK pages, was in the GTK crate deleted in `26740e3` (its source is at
//! `6841d84`). Change a fixture only deliberately, by hand, in the commit that changes the
//! generator's output and says why.

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

/// [`open_tool`] in a window tall enough to show a long page whole, so every button can be
/// clicked where it is drawn.
pub fn open_tool_tall(tool: CamTool) -> Harness<'static, GcodeKitApp> {
    let mut h = open_tool(tool);
    h.set_size(egui::vec2(1280.0, 2400.0));
    frames(&mut h, 3);
    h
}

/// Clicks the button `label`, scrolling the page to it first (a tall page leaves its buttons
/// below the fold), then lets the app handle the request.
pub fn click(h: &mut Harness<'static, GcodeKitApp>, label: &str) {
    h.get_by_label(label).scroll_to_me();
    // The scroll lands some frames later: wait until the button has stayed put for three.
    let mut rect = h.get_by_label(label).rect();
    let mut still = 0;
    for _ in 0..20 {
        frames(h, 1);
        let now = h.get_by_label(label).rect();
        still = if now == rect { still + 1 } else { 0 };
        rect = now;
        if still == 3 {
            break;
        }
    }
    assert!(
        still >= 3,
        "{label} still moving 20 frames after scrolling to it (now at {rect:?})"
    );
    h.get_by_label(label).click();
    frames(h, 3);
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
    let cases: [(Breaker, &str); 8] = [
        (|p| p.peck_depth = "-1".into(), "Must not be negative"),
        (
            |p| p.tool_diameter = "12".into(),
            "Must not be larger than the hole",
        ),
        (|p| p.spindle_speed = "0".into(), "Must be greater than 0"),
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

/// GTK refused to drill while the device is in laser mode (`$32=1`): the program starts the
/// spindle with `M3` and plunges, which would fire a laser in place.
#[test]
fn test_cam_drill_press_refuses_laser_mode() {
    let mut page = DrillPressParams::with_center(MeasurementSystem::Metric, (100.0, 100.0));
    page.laser_mode = true;
    let errors = page.parameters().expect_err("laser mode is refused");
    assert!(errors.iter().any(|e| e.field.is_none()));
    assert_eq!(page.generate().ok(), None);
}

/// The pane reads `$32` from the device status every frame and shows the refusal.
#[test]
fn test_cam_drill_press_pane_follows_the_laser_mode_setting() {
    let _guard = crate::global_env_lock();
    gcodekit5_app::device_status::update_grbl_setting(32, "1".to_string());
    let mut h = open_tool(CamTool::DrillPress);
    let shown = h.query_by_label(LASER_MODE_REFUSAL).is_some();
    h.get_by_label("Generate").click();
    frames(&mut h, 2);
    let output = pane(&h).output().map(str::to_string);
    gcodekit5_app::device_status::update_grbl_setting(32, "0".to_string());
    assert!(shown, "laser-mode refusal not shown");
    assert_eq!(output, None, "Generate made a program in laser mode");
    frames(&mut h, 2);
    assert!(h.query_by_label(LASER_MODE_REFUSAL).is_none());
}

/// The refusal shown while the device is in laser mode.
const LASER_MODE_REFUSAL: &str =
    "The device is in laser mode ($32=1). Switch it to CNC mode to use the Drill Press.";

/// The generated program is withdrawn as soon as the input changes.
#[test]
fn test_cam_generated_output_clears_when_the_input_changes() {
    let mut h = open_tool(CamTool::DrillPress);
    h.get_by_label("Generate").click();
    frames(&mut h, 2);
    assert!(pane(&h).output().is_some());
    pane(&h).drill_press_mut().hole_diameter = "11".into();
    frames(&mut h, 2);
    assert_eq!(pane(&h).output(), None);
    assert!(h.query_by_label("Generated G-Code").is_none());
}

/// A long generated program is not laid out whole every frame: only the rows in view are
/// drawn, so no widget holds the whole text.
#[test]
fn test_cam_generated_output_draws_only_the_rows_in_view() {
    let mut h = open_tool_tall(CamTool::DrillPress);
    {
        let mut cam = pane(&h);
        let page = cam.drill_press_mut();
        page.bottom_z = "-50".into();
        page.peck_depth = "0.05".into();
    }
    frames(&mut h, 2);
    h.get_by_label("Generate").click();
    frames(&mut h, 3);
    let output = pane(&h).output().map(str::to_string).unwrap_or_default();
    let lines = output.lines().count();
    assert!(lines > 1000, "only {lines} lines generated");
    // A label keeps its text in `value`, other widgets in `label`.
    let text = |n: &egui_kittest::kittest::AccessKitNode<'_>| n.value().or_else(|| n.label());
    let first = output.lines().next().unwrap_or_default().to_string();
    assert!(
        h.query_all_by(|n| text(n).as_deref() == Some(first.as_str()))
            .next()
            .is_some(),
        "the first line {first:?} is not shown"
    );
    assert!(
        h.query_all_by(|n| text(n).is_some_and(|t| t.lines().count() > 100))
            .next()
            .is_none(),
        "a widget holds the whole program"
    );
    let shown: std::collections::HashSet<&str> = output.lines().collect();
    let rows = h
        .query_all_by(|n| text(n).is_some_and(|t| shown.contains(t.as_str())))
        .count();
    assert!(rows < 100, "{rows} rows drawn");
}
