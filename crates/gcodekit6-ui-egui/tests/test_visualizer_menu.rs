//! The visualizer's right-click menu (GTK's): the work-plane point under the pointer, Copy
//! cursor coordinates in GTK's format, the fits and the Toggle switches.

use egui::{Modifiers, PointerButton};
use egui_kittest::kittest::Queryable;
use gcodekit6_core::units::MeasurementSystem;
use gcodekit6_ui_egui::panes::visualizer::menu::{coordinates_text, cursor_on_plane};
use gcodekit6_ui_egui::panes::visualizer::{Camera, StdView, VisualizerPane};
use gcodekit6_ui_egui::workspace::Pane;
use glam::{Vec2, Vec3};

use crate::test_designer_canvas::click_with;
use crate::test_visualizer::{ndc, pane_harness};

/// The screen position of `world` in a view of `rect` seen by `camera`.
fn screen(camera: &Camera, rect: egui::Rect, world: Vec3) -> egui::Pos2 {
    let p = ndc(camera.view_proj(rect.width() / rect.height()), world);
    egui::pos2(
        rect.left() + (p.x + 1.0) / 2.0 * rect.width(),
        rect.top() + (1.0 - p.y) / 2.0 * rect.height(),
    )
}

#[test]
fn test_cursor_on_plane_inverts_the_projection() {
    let rect = egui::Rect::from_min_size(egui::pos2(250.0, 0.0), egui::vec2(480.0, 360.0));
    for view in [StdView::Top, StdView::IsoSe, StdView::IsoNw] {
        let mut camera = Camera::default();
        camera.set_aspect(rect.width() / rect.height());
        camera.view(view);
        for world in [Vec3::new(12.5, -7.25, 0.0), Vec3::new(30.0, 40.0, 0.0)] {
            let at = cursor_on_plane(&camera, rect, screen(&camera, rect, world));
            let at = at.unwrap_or(Vec2::splat(f32::NAN));
            assert!(
                at.distance(world.truncate()) < 0.05,
                "{view:?}: {at:?} for {world:?}"
            );
        }
    }
    // Looking along the plane, there is no point to report.
    let mut side = Camera::default();
    side.view(StdView::Front);
    let centre = rect.center();
    assert!(cursor_on_plane(&side, rect, centre).is_none_or(|p| p.is_finite()));
}

#[test]
fn test_coordinates_text_is_gtk_format() {
    let at = Vec2::new(12.3456, -7.0);
    assert_eq!(
        coordinates_text(at, MeasurementSystem::Metric),
        "X 12.346  Y -7.000"
    );
    assert_eq!(
        coordinates_text(Vec2::new(25.4, 50.8), MeasurementSystem::Imperial),
        "X 1.000  Y 2.000"
    );
}

/// Right-clicks the view of the harness's pane at the work-plane point `world`.
fn right_click(harness: &mut egui_kittest::Harness<'_, VisualizerPane>, world: Vec3) {
    let rect = harness.state().view_rect();
    let pos = screen(harness.state().camera(), rect, world);
    click_with(harness, pos, PointerButton::Secondary, Modifiers::NONE);
}

#[test]
fn test_copy_cursor_coordinates_copies_the_clicked_point() {
    let mut pane = VisualizerPane::new();
    pane.apply(gcodekit6_ui_egui::panes::visualizer::NavAction::View(
        StdView::Top,
    ));
    let mut harness = pane_harness(pane);
    harness.run();
    right_click(&mut harness, Vec3::new(10.0, 5.0, 0.0));
    harness.get_by_label("Copy cursor coordinates").click();
    harness.step();
    let copied: Vec<String> = harness
        .output()
        .platform_output
        .commands
        .iter()
        .filter_map(|c| match c {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(copied.len(), 1, "{copied:?}");
    let parts: Vec<f32> = copied[0]
        .split_whitespace()
        .filter_map(|w| w.parse().ok())
        .collect();
    assert_eq!(parts.len(), 2, "{copied:?}");
    assert!(
        (parts[0] - 10.0).abs() < 0.5 && (parts[1] - 5.0).abs() < 0.5,
        "{copied:?}"
    );
}

#[test]
fn test_menu_toggles_and_fits() {
    let mut pane = VisualizerPane::new();
    let axis = |max: f64| gcodekit6_devicedb::AxisLimits {
        min: 0.0,
        max,
        enabled: true,
    };
    let profile = gcodekit6_devicedb::DeviceProfile {
        x_axis: axis(300.0),
        y_axis: axis(200.0),
        z_axis: axis(0.0),
        ..gcodekit6_devicedb::DeviceProfile::default()
    };
    pane.set_device(Some(&profile));
    let mut harness = pane_harness(pane);
    harness.run();
    for (label, read) in [
        (
            "Toggle Grid",
            (|d| d.grid) as fn(gcodekit6_ui_egui::panes::visualizer::Display) -> bool,
        ),
        ("Toggle Machine Bounds", |d| d.bounds),
        ("Toggle Rapid Moves", |d| d.rapids),
        ("Toggle Cutting Moves", |d| d.cuts),
    ] {
        right_click(&mut harness, Vec3::ZERO);
        harness.get_by_label(label).click();
        harness.run();
        assert!(!read(harness.state().display()), "{label} turned it off");
    }
    harness.state_mut().camera_mut().zoom(50.0);
    let buttons: Vec<egui::Rect> = harness
        .get_all_by_label("Fit to Device Working Area")
        .map(|n| n.rect())
        .collect();
    right_click(&mut harness, Vec3::ZERO);
    // The sidebar's and the floating buttons share the name; click the menu's.
    let item = harness
        .get_all_by_label("Fit to Device Working Area")
        .find(|n| !buttons.contains(&n.rect()));
    match item {
        Some(item) => item.click(),
        None => panic!("no menu item"),
    }
    harness.run();
    assert_eq!(
        harness.state().camera().target(),
        Vec3::new(150.0, 100.0, 0.0),
        "the profile's work area"
    );
    assert!(
        harness.query_by_label("Toggle Grid").is_none(),
        "menu closed"
    );
}
