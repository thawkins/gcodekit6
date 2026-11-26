//! The visualizer's machine bounds and Fit to Device Working Area: the active device
//! profile's X, Y and Z limits, outlined in the view (GTK's Show Machine Bounds) and fitted
//! by the sidebar and floating buttons.

use egui_kittest::kittest::Queryable;
use gcodekit5_devicedb::{AxisLimits, DeviceProfile};
use gcodekit5_ui_egui::panes::visualizer::{
    self, Aabb, Camera, Display, StdView, VisualizerPane, default_work_area, work_area_of,
};
use gcodekit5_ui_egui::workspace::Pane;
use glam::Vec3;

use crate::test_visualizer::{gpu_harness, load, pane_harness, pixel_near, sample};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A 400 x 300 mm machine with 50 mm of Z below zero.
fn profile() -> DeviceProfile {
    let axis = |min: f64, max: f64| AxisLimits {
        min,
        max,
        enabled: true,
    };
    DeviceProfile {
        x_axis: axis(0.0, 400.0),
        y_axis: axis(0.0, 300.0),
        z_axis: axis(-50.0, 0.0),
        ..DeviceProfile::default()
    }
}

#[test]
fn test_work_area_comes_from_the_profile() {
    assert_eq!(
        work_area_of(&profile()),
        Aabb::new(Vec3::new(0.0, 0.0, -50.0), Vec3::new(400.0, 300.0, 0.0))
    );
    let mut swapped = profile();
    swapped.z_axis = AxisLimits {
        min: 0.0,
        max: -50.0,
        enabled: true,
    };
    assert_eq!(work_area_of(&swapped), work_area_of(&profile()));
    let mut pane = VisualizerPane::new();
    pane.set_device(Some(&profile()));
    assert_eq!(pane.work_area(), Some(work_area_of(&profile())));
    pane.set_device(None);
    assert_eq!(pane.work_area(), None);
}

/// The camera after fitting `area` from the default view.
fn fitted(area: Aabb, aspect: f32) -> Camera {
    let mut camera = Camera::default();
    camera.set_aspect(aspect);
    camera.fit(area);
    camera
}

#[test]
fn test_fit_to_device_buttons_fit_the_work_area() {
    for index in [0, 1] {
        let mut pane = VisualizerPane::new();
        pane.set_device(Some(&profile()));
        let mut harness = pane_harness(pane);
        harness.run();
        // The sidebar's button first, then the floating one under the nav cube.
        let buttons: Vec<_> = harness
            .get_all_by_label("Fit to Device Working Area")
            .collect();
        assert_eq!(buttons.len(), 2, "sidebar and floating button");
        buttons[index].click();
        harness.run();
        let rect = harness.state().view_rect();
        let expected = fitted(work_area_of(&profile()), rect.width() / rect.height());
        assert_eq!(harness.state().camera().target(), expected.target());
        assert_eq!(harness.state().camera().distance(), expected.distance());
    }
}

#[test]
fn test_fit_to_device_without_a_profile_uses_the_default_area() {
    let mut pane = VisualizerPane::new();
    pane.fit_device();
    assert_eq!(
        pane.camera().target(),
        Vec3::new(125.0, 125.0, 0.0),
        "GTK's 250 x 250 mm default"
    );
    assert_eq!(default_work_area().max, Vec3::new(250.0, 250.0, 0.0));
}

/// Machine bounds blue.
fn is_bounds(p: [u8; 4]) -> bool {
    p == [0, 128, 255, 255]
}

/// Whether the top edge of the work area (Y = 300) is drawn, the view fitted to the
/// device from the top.
fn bounds_drawn(display: Display, device: bool) -> Result<bool, Box<dyn std::error::Error>> {
    let mut pane = VisualizerPane::new();
    pane.set_display(display);
    pane.set_device(device.then(profile).as_ref());
    let mut harness = gpu_harness(pane);
    load(&mut harness, sample())?;
    harness
        .state_mut()
        .apply(visualizer::NavAction::View(StdView::Top));
    harness.state_mut().fit_device();
    harness.run();
    let rendered = harness.render()?;
    let image = |x: u32, y: u32| rendered.get_pixel_checked(x, y).map(|p| p.0);
    Ok(pixel_near(
        &image,
        harness.state(),
        Vec3::new(200.0, 300.0, 0.0),
        is_bounds,
    ))
}

#[test]
fn test_machine_bounds_drawn_when_shown() -> TestResult {
    assert!(bounds_drawn(Display::default(), true)?, "drawn");
    let hidden = Display {
        bounds: false,
        ..Display::default()
    };
    assert!(!bounds_drawn(hidden, true)?, "hidden by the toggle");
    assert!(
        !bounds_drawn(Display::default(), false)?,
        "no profile, no bounds"
    );
    Ok(())
}
