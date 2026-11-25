//! The Spoilboard Surfacing page: GTK's defaults render, invalid input is shown at the
//! field and disables the actions, the program is byte-identical to the GTK page's (fixtures
//! from `capture_cam_fixtures`), and a device with fewer than 3 axes is refused.

use egui_kittest::kittest::Queryable;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_ui_egui::panes::cam_tools::CamTool;
use gcodekit5_ui_egui::panes::cam_tools::spoilboard_surfacing::SpoilboardSurfacingParams;

use crate::test_cam_drill_press::{fixture, open_tool, pane};
use crate::test_designer_files_app::frames;

/// Makes one field of the page unsafe.
type Breaker = fn(&mut SpoilboardSurfacingParams);

#[test]
fn test_cam_surfacing_default_page_renders() {
    let h = open_tool(CamTool::SpoilboardSurfacing);
    for label in [
        "Width:",
        "Height:",
        "Tool Diameter:",
        "Cut Depth:",
        "Stepover (%):",
        "Feed Rate (mm/min):",
        "Spindle Speed (RPM):",
        "Safe Z Height:",
        "Home Device Before Start",
        "Generate",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label} not shown");
    }
}

#[test]
fn test_cam_surfacing_generate_matches_gtk_defaults() {
    let mut h = open_tool(CamTool::SpoilboardSurfacing);
    h.get_by_label("Generate").click();
    frames(&mut h, 2);
    assert_eq!(
        pane(&h).output(),
        Some(fixture("spoilboard_surfacing_default.gcode").as_str())
    );
}

/// GTK's imperial case: the lengths are typed in inches.
#[test]
fn test_cam_surfacing_generate_matches_gtk_custom_imperial() {
    let mut page = SpoilboardSurfacingParams::new(MeasurementSystem::Imperial);
    page.width = "12".into();
    page.height = "8".into();
    page.tool_diameter = "1".into();
    page.feed_rate = "1200".into();
    page.spindle_speed = "16000".into();
    page.cut_depth = "0.02".into();
    page.stepover_percent = "50".into();
    page.safe_z = "0.25".into();
    page.home_before = true;
    assert_eq!(
        page.generate(),
        Ok(fixture("spoilboard_surfacing_custom.gcode"))
    );
}

#[test]
fn test_cam_surfacing_invalid_input_shows_error_and_disables_generate() {
    let cases: [(Breaker, &str); 6] = [
        (|p| p.cut_depth = "-0.5".into(), "Must be greater than 0"),
        (|p| p.tool_diameter = "0".into(), "Must be greater than 0"),
        (|p| p.stepover_percent = "150".into(), "Out of range"),
        (|p| p.width = "-1".into(), "Must be greater than 0"),
        (|p| p.safe_z = "x".into(), "Not a valid length"),
        (|p| p.feed_rate = "0".into(), "Must be greater than 0"),
    ];
    for (break_it, message) in cases {
        let mut h = open_tool(CamTool::SpoilboardSurfacing);
        break_it(pane(&h).spoilboard_surfacing_mut());
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

/// GTK refused surfacing on a device with fewer than 3 axes.
#[test]
fn test_cam_surfacing_needs_three_axes() {
    let mut page = SpoilboardSurfacingParams::new(MeasurementSystem::Metric);
    page.num_axes = 2;
    let errors = page.parameters().expect_err("two axes are refused");
    assert!(errors.iter().any(|e| e.field.is_none()));
}
