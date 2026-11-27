//! The visualizer's right-click menu: Fit to Content, Reset View
//! and Fit to Device Working Area; Copy cursor coordinates; and the Toggle Grid,
//! Toggle Machine Bounds, Toggle Rapid Moves and Toggle Cutting Moves switches of the
//! sidebar's toggles.
//!
//! The cursor coordinates are where the pointer was when the menu opened, on the work plane
//! (Z = 0) under it ([`cursor_on_plane`]); they are copied as
//! `X 12.345  Y 6.789` in the user's measurement system with three decimals.
//!
//! Dependencies: `egui`, `glam`, `gcodekit6-app` (translations), `gcodekit6-core` (units).

use gcodekit6_app::i18n::Translator;
use gcodekit6_core::units::{MeasurementSystem, format_length};
use glam::{Vec2, Vec4};

use super::camera::Camera;
use super::sidebar::Display;

/// The msgids the menu shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Fit to Content"),
        n_!("Reset View"),
        n_!("Fit to Device Working Area"),
        n_!("Copy cursor coordinates"),
        n_!("Toggle Grid"),
        n_!("Toggle Machine Bounds"),
        n_!("Toggle Rapid Moves"),
        n_!("Toggle Cutting Moves"),
    ]
}

/// A view command chosen in the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// Fit the toolpath into the view.
    Fit,
    /// Reset the camera and fit the toolpath.
    ResetView,
    /// Fit the active device's work area into the view.
    FitDevice,
}

/// The point on the work plane (Z = 0) under `pos` in a view of `rect` seen by `camera`,
/// or `None` when the view looks along the plane.
///
/// # Arguments
/// * `camera` - The view's camera.
/// * `rect` - The view's rectangle in points.
/// * `pos` - The screen position in points.
pub fn cursor_on_plane(camera: &Camera, rect: egui::Rect, pos: egui::Pos2) -> Option<Vec2> {
    let aspect = rect.width() / rect.height().max(1.0);
    let inverse = camera.view_proj(aspect).inverse();
    let x = (pos.x - rect.left()) / rect.width().max(1.0) * 2.0 - 1.0;
    let y = 1.0 - (pos.y - rect.top()) / rect.height().max(1.0) * 2.0;
    let unproject = |depth: f32| {
        let p = inverse * Vec4::new(x, y, depth, 1.0);
        p.truncate() / p.w
    };
    let (near, far) = (unproject(0.0), unproject(1.0));
    let dz = far.z - near.z;
    if dz.abs() < f32::EPSILON || !near.is_finite() || !far.is_finite() {
        return None;
    }
    let t = -near.z / dz;
    let hit = near + (far - near) * t;
    hit.is_finite().then(|| hit.truncate())
}

/// The clipboard text for the work-plane point `at`: `X <x>  Y <y>`.
///
/// # Arguments
/// * `at` - The point in mm.
/// * `units` - The measurement system to copy in.
pub fn coordinates_text(at: Vec2, units: MeasurementSystem) -> String {
    format!(
        "X {}  Y {}",
        format_length(f64::from(at.x), units),
        format_length(f64::from(at.y), units)
    )
}

/// Draws the menu's items into `ui` (the open context menu), switching `display`'s toggles
/// and copying `cursor` in place.
///
/// # Arguments
/// * `ui` - The context menu's `Ui`.
/// * `display` - The sidebar's toggles.
/// * `cursor` - The work-plane point under the pointer when the menu opened.
/// * `units` - The measurement system to copy in.
/// * `tr` - Translator for the current UI language.
///
/// # Returns
/// The view command chosen, if any.
pub fn show(
    ui: &mut egui::Ui,
    display: &mut Display,
    cursor: Option<Vec2>,
    units: MeasurementSystem,
    tr: &Translator,
) -> Option<MenuAction> {
    let mut action = None;
    let views = [
        ("Fit to Content", MenuAction::Fit),
        ("Reset View", MenuAction::ResetView),
        ("Fit to Device Working Area", MenuAction::FitDevice),
    ];
    for (msgid, act) in views {
        if ui.button(tr.tr(msgid)).clicked() {
            action = Some(act);
            ui.close();
        }
    }
    ui.separator();
    let copy = ui.add_enabled(
        cursor.is_some(),
        egui::Button::new(tr.tr("Copy cursor coordinates")),
    );
    if copy.clicked()
        && let Some(at) = cursor
    {
        ui.ctx().copy_text(coordinates_text(at, units));
        ui.close();
    }
    ui.separator();
    let toggles: [(&str, &mut bool); 4] = [
        ("Toggle Grid", &mut display.grid),
        ("Toggle Machine Bounds", &mut display.bounds),
        ("Toggle Rapid Moves", &mut display.rapids),
        ("Toggle Cutting Moves", &mut display.cuts),
    ];
    for (msgid, value) in toggles {
        if ui.button(tr.tr(msgid)).clicked() {
            *value = !*value;
            ui.close();
        }
    }
    action
}
