//! The Designer canvas's device work area: the active device profile's X/Y limits (GTK's
//! `compute_device_bbox`), outlined on the canvas, fitted by Fit to Device, and the limits
//! exported G-code is checked against; without a profile, GTK's 250 x 250 mm area and the
//! Preferences limits.

use gcodekit5_devicedb::{AxisLimits, DeviceProfile};
use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::panes::designer::DesignerPane;
use gcodekit5_ui_egui::panes::designer::canvas::overlay::ViewAction;
use gcodekit5_ui_egui::workspace::Pane;
use glam::Vec2;

use crate::test_designer_canvas::harness;

/// A 400 x 300 mm machine whose X axis starts at -100 mm.
fn profile() -> DeviceProfile {
    let axis = |min: f64, max: f64| AxisLimits {
        min,
        max,
        enabled: true,
    };
    DeviceProfile {
        x_axis: axis(-100.0, 300.0),
        y_axis: axis(0.0, 300.0),
        z_axis: axis(-50.0, 0.0),
        ..DeviceProfile::default()
    }
}

#[test]
fn test_work_area_follows_the_active_profile() {
    let mut pane = DesignerPane::new();
    assert_eq!(pane.work_area(), (0.0, 0.0, 250.0, 250.0), "GTK's default");
    pane.set_device(Some(&profile()));
    assert_eq!(pane.work_area(), (-100.0, 0.0, 300.0, 300.0));
    // A profile with an axis's limits the wrong way round still gives a box.
    let mut swapped = profile();
    swapped.y_axis.min = 300.0;
    swapped.y_axis.max = 0.0;
    pane.set_device(Some(&swapped));
    assert_eq!(pane.work_area(), (-100.0, 0.0, 300.0, 300.0));
    pane.set_device(None);
    assert_eq!(pane.work_area(), (0.0, 0.0, 250.0, 250.0));
}

/// The pane's export limits as (x min, y min, x max, y max).
fn limits(pane: &DesignerPane) -> Option<(f64, f64, f64, f64)> {
    pane.machine_limits()
        .map(|l| (l.x_min, l.y_min, l.x_max, l.y_max))
}

#[test]
fn test_export_limits_are_the_profiles_else_the_preferences() {
    let mut pane = DesignerPane::new();
    let mut config = Config::default();
    config.machine.x_limit = 200.0;
    config.machine.y_limit = 150.0;
    pane.configure(&config);
    let preferences = Some((0.0, 0.0, 200.0, 150.0));
    assert_eq!(limits(&pane), preferences);
    pane.set_device(Some(&profile()));
    let device = Some((-100.0, 0.0, 300.0, 300.0));
    assert_eq!(limits(&pane), device);
    // Settings applied later do not replace the profile's limits.
    pane.configure(&config);
    assert_eq!(limits(&pane), device);
    pane.set_device(None);
    assert_eq!(limits(&pane), preferences);
}

#[test]
fn test_fit_to_device_fits_the_profiles_area() {
    let mut pane = DesignerPane::new();
    pane.set_device(Some(&profile()));
    let mut h = harness(pane);
    h.state_mut().apply_view(ViewAction::FitViewport);
    h.run();
    h.state_mut().apply_view(ViewAction::FitWorkArea);
    h.run();
    let view = h.state().view();
    let rect = view.rect();
    let (a, b) = (
        view.world_to_screen(Vec2::new(-100.0, 0.0)),
        view.world_to_screen(Vec2::new(300.0, 300.0)),
    );
    // The whole area is in the canvas, centred, and fills it along one axis (with padding).
    assert!(
        rect.contains(a) && rect.contains(b),
        "{a:?} {b:?} in {rect:?}"
    );
    let centre = egui::Rect::from_two_pos(a, b).center();
    assert!((centre - rect.center()).length() < 1.0, "{centre:?}");
    let fill = ((b.x - a.x) / rect.width()).max((a.y - b.y) / rect.height());
    assert!(fill > 0.8, "{fill}");
}
