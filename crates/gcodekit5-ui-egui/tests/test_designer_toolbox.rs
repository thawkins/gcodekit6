//! Designer toolbox: the tool buttons drive the canvas tool, the grid and snap switches, the
//! machine mode, the tool and stock settings and their unit conversion.

use egui_kittest::kittest::Queryable;
use gcodekit5_designer::designer_state::MachineMode;
use gcodekit5_designer::model::ShapeType;
use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::panes::designer::DesignerPane;
use gcodekit5_ui_egui::panes::designer::canvas::DesignerTool;
use gcodekit5_ui_egui::panes::designer::toolbox::settings::{Kind, from_display, to_display};
use gcodekit5_ui_egui::workspace::Pane;

use crate::test_designer_canvas::{drag, harness, ids, screen};
use gcodekit5_core::units::MeasurementSystem::{Imperial, Metric};

/// The English label of `tool`'s button.
fn label(tool: DesignerTool) -> &'static str {
    match tool {
        DesignerTool::Select => "Select",
        DesignerTool::Pan => "Pan",
        DesignerTool::Rectangle => "Rectangle",
        DesignerTool::Circle => "Circle",
        DesignerTool::Line => "Line",
        DesignerTool::Ellipse => "Ellipse",
        DesignerTool::Triangle => "Triangle",
        DesignerTool::Polygon => "Polygon",
        DesignerTool::Gear => "Gear",
        DesignerTool::Sprocket => "Sprocket",
        DesignerTool::Polyline => "Polyline",
    }
}

#[test]
fn test_each_tool_button_sets_the_canvas_tool() {
    let mut h = harness(DesignerPane::new());
    for tool in DesignerTool::ALL {
        h.get_by_label(label(tool)).click();
        h.run();
        assert_eq!(h.state().tool(), tool, "{tool:?}");
    }
}

#[test]
fn test_active_tool_label_follows_the_tool() {
    let mut h = harness(DesignerPane::new());
    assert!(h.query_by_label("Active tool: Select").is_some());
    h.get_by_label("Gear").click();
    h.run();
    assert!(h.query_by_label("Active tool: Gear").is_some());
    assert!(h.query_by_label("Active tool: Select").is_none());
}

#[test]
fn test_polyline_hint_shows_only_for_the_polyline_tool() {
    let hint = |h: &egui_kittest::Harness<'_, DesignerPane>| {
        h.query_by_label_contains("Click to add points").is_some()
    };
    let mut h = harness(DesignerPane::new());
    assert!(!hint(&h));
    h.get_by_label("Polyline").click();
    h.run();
    assert!(hint(&h));
    h.get_by_label("Select").click();
    h.run();
    assert!(!hint(&h));
}

#[test]
fn test_picking_a_tool_then_dragging_draws_with_it() {
    let mut h = harness(DesignerPane::new());
    h.get_by_label("Circle").click();
    h.run();
    let (from, to) = (screen(&h, 40.0, 40.0), screen(&h, 100.0, 100.0));
    drag(&mut h, from, to);
    let shapes = ids(h.state());
    assert_eq!(shapes.len(), 1);
    let kind = h
        .state()
        .state()
        .canvas
        .get_shape(shapes[0])
        .map(|o| o.shape.shape_type());
    assert_eq!(kind, Some(ShapeType::Circle));
}

#[test]
fn test_grid_and_snap_checkboxes_toggle_the_design() {
    let mut h = harness(DesignerPane::new());
    assert!(h.state().state().show_grid);
    h.get_by_label("Show Grid").click();
    h.get_by_label("Snap to grid").click();
    h.run();
    assert!(!h.state().state().show_grid);
    assert!(!h.state().state().snap_enabled);
}

#[test]
fn test_machine_mode_radios_switch_mode_and_relabel_settings() {
    let mut h = harness(DesignerPane::new());
    assert_eq!(h.state().state().machine_mode(), MachineMode::Laser2D);
    assert!(h.query_by_label("Laser Speed:").is_some());
    assert!(h.query_by_label("Continuous Z").is_none(), "CNC only");
    h.get_by_label("CNC 3D").click();
    h.run();
    assert_eq!(h.state().state().machine_mode(), MachineMode::Cnc3D);
    assert!(h.query_by_label("Feed Rate:").is_some());
    assert!(h.query_by_label("Continuous Z").is_some());
    h.get_by_label("Laser 2D").click();
    h.run();
    assert_eq!(h.state().state().machine_mode(), MachineMode::Laser2D);
}

#[test]
fn test_continuous_z_checkbox_writes_the_setting() {
    let mut h = harness(DesignerPane::new());
    h.get_by_label("CNC 3D").click();
    h.run();
    h.get_by_label("Continuous Z").click();
    h.run();
    assert!(h.state().state().tool_settings.continuous_z_between_passes);
}

#[test]
fn test_dragging_a_setting_field_changes_the_setting() {
    let mut h = harness(DesignerPane::new());
    h.get_by_label("CNC 3D").click();
    h.run();
    let before = h.state().state().tool_settings.feed_rate;
    let field = h
        .get_all_by_role(egui::accesskit::Role::SpinButton)
        .next()
        .expect("feed field")
        .rect();
    let from = field.center();
    drag(&mut h, from, from + egui::vec2(40.0, 0.0));
    let after = h.state().state().tool_settings.feed_rate;
    assert!(after > before, "{before} -> {after}");
}

#[test]
fn test_stock_settings_section_opens_and_lists_the_stock() {
    let mut h = harness(DesignerPane::new());
    assert!(h.query_by_label("Stock Width:").is_none(), "collapsed");
    h.get_by_label("Stock Settings").click();
    h.run();
    for name in [
        "Stock Width:",
        "Stock Height:",
        "Stock Thickness:",
        "Safe Z Height:",
    ] {
        assert!(h.query_by_label(name).is_some(), "{name}");
    }
}

#[test]
fn test_units_follow_the_settings() {
    // Feed rate: mm/min or in/min; length: mm or in, except a laser's, unconverted.
    assert_eq!(
        to_display(Kind::FeedRate, MachineMode::Cnc3D, Metric, 2540.0),
        (2540.0, "mm/min")
    );
    assert_eq!(
        to_display(Kind::FeedRate, MachineMode::Cnc3D, Imperial, 2540.0),
        (100.0, "in/min")
    );
    assert_eq!(
        to_display(Kind::Length, MachineMode::Cnc3D, Imperial, 25.4),
        (1.0, "in")
    );
    assert_eq!(
        to_display(Kind::Length, MachineMode::Laser2D, Imperial, 25.4),
        (25.4, "")
    );
    // Spindle: RPM, or a laser's percentage (10 model units per percent).
    assert_eq!(
        to_display(Kind::Rpm, MachineMode::Cnc3D, Metric, 800.0),
        (800.0, "RPM")
    );
    assert_eq!(
        to_display(Kind::Rpm, MachineMode::Laser2D, Metric, 500.0),
        (50.0, "%")
    );
    for kind in [Kind::Rpm, Kind::Length, Kind::FeedRate] {
        for mode in [MachineMode::Laser2D, MachineMode::Cnc3D] {
            for units in [Metric, Imperial] {
                let (shown, _) = to_display(kind, mode, units, 123.0);
                let back = from_display(kind, mode, units, shown);
                assert!((back - 123.0).abs() < 1e-9, "{mode:?} {units:?}: {back}");
            }
        }
    }
}

/// Whether a settings field shows a value with `suffix` (the suffix is part of its value).
fn shows_suffix(h: &egui_kittest::Harness<'_, DesignerPane>, suffix: &str) -> bool {
    h.get_all_by_role(egui::accesskit::Role::SpinButton)
        .any(|n| n.value().is_some_and(|v| v.contains(suffix)))
}

#[test]
fn test_configure_applies_the_measurement_system() {
    let mut metric = harness(DesignerPane::new());
    metric.get_by_label("CNC 3D").click();
    metric.run();
    assert!(shows_suffix(&metric, "mm/min"));
    assert!(!shows_suffix(&metric, "in/min"));

    let mut config = Config::default();
    config.ui.measurement_system = Imperial;
    let mut pane = DesignerPane::new();
    pane.configure(&config);
    let mut imperial = harness(pane);
    imperial.get_by_label("CNC 3D").click();
    imperial.run();
    assert!(shows_suffix(&imperial, "in/min"));
    assert!(!shows_suffix(&imperial, "mm/min"));
}
