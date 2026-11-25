//! The Spoilboard Grid page: GTK's defaults render, invalid input is shown at the field and
//! disables the actions, and the program is byte-identical to the GTK page's (fixtures from
//! `capture_cam_fixtures`).

use egui_kittest::kittest::Queryable;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_ui_egui::panes::cam_tools::CamTool;
use gcodekit5_ui_egui::panes::cam_tools::spoilboard_grid::{LaserMode, SpoilboardGridParams};

use crate::test_cam_drill_press::{fixture, open_tool, pane};
use crate::test_designer_files_app::frames;

/// Makes one field of the page unsafe.
type Breaker = fn(&mut SpoilboardGridParams);

#[test]
fn test_cam_grid_default_page_renders() {
    let h = open_tool(CamTool::SpoilboardGrid);
    for label in [
        "Width:",
        "Height:",
        "Grid Spacing:",
        "Laser Power (S):",
        "Feed Rate (mm/min):",
        "Laser Mode:",
        "Generate",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label} not shown");
    }
    assert!(
        h.query_by_value("M4 (Dynamic Power)").is_some(),
        "M4 is not the mode shown"
    );
}

#[test]
fn test_cam_grid_generate_matches_gtk_defaults() {
    let mut h = open_tool(CamTool::SpoilboardGrid);
    h.get_by_label("Generate").click();
    frames(&mut h, 2);
    assert_eq!(
        pane(&h).output(),
        Some(fixture("spoilboard_grid_default.gcode").as_str())
    );
}

#[test]
fn test_cam_grid_generate_matches_gtk_custom() {
    let mut page = SpoilboardGridParams::new(MeasurementSystem::Metric);
    page.width = "200".into();
    page.height = "150".into();
    page.grid_spacing = "25".into();
    page.feed_rate = "1500".into();
    page.laser_power = "500".into();
    page.laser_mode = LaserMode::M3;
    page.home_before = true;
    assert_eq!(page.generate(), Ok(fixture("spoilboard_grid_custom.gcode")));
}

#[test]
fn test_cam_grid_invalid_input_shows_error_and_disables_generate() {
    let cases: [(Breaker, &str); 5] = [
        (|p| p.grid_spacing = "0".into(), "Must be greater than 0"),
        (|p| p.width = "-10".into(), "Must be greater than 0"),
        (|p| p.height = "tall".into(), "Not a valid length"),
        (|p| p.laser_power = "-5".into(), "Must not be negative"),
        (|p| p.feed_rate = "0".into(), "Must be greater than 0"),
    ];
    for (break_it, message) in cases {
        let mut h = open_tool(CamTool::SpoilboardGrid);
        break_it(pane(&h).spoilboard_grid_mut());
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
