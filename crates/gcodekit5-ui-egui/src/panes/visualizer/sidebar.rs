//! The visualizer's left sidebar, laid out as the GTK one: a toolbar (Fit to Content,
//! Reset View and Hide) above a collapsible Inspector with the program's bounds.
//!
//! Ported from the GTK `ui/gtk/visualizer/mod.rs` sidebar. GTK's Fit to Viewport button
//! (`view-restore`) is Reset View here, which `Cmd+0` also runs. Left out, because the
//! egui visualizer lacks the features: Fit to Device Working Area, the 2D/3D switch, the
//! Toolpath and Guides toggles (rapids, cuts, laser/spindle, grid, grid spacing, machine
//! bounds), the Simulation section and the inspector's S-value statistics.
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations), `gcodekit5-core` (units).

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::{MeasurementSystem, get_unit_label};

use super::camera::Aabb;
use crate::icons::{Icon, icon_only_button};
use crate::panes::machine_control::format_length;
use crate::workspace::named_icon_button;

/// The sidebar's default width in points.
pub const WIDTH: f32 = 250.0;
/// The sidebar's least width in points (GTK's width request).
pub const MIN_WIDTH: f32 = 200.0;

/// The msgids the sidebar shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Fit to Content"),
        n_!("Reset View"),
        n_!("Hide"),
        n_!("Hide Sidebar"),
        n_!("Show Sidebar"),
        n_!("Inspector"),
        n_!("Bounds"),
        n_!("to"),
    ]
}

/// What a sidebar button asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarAction {
    /// Fit the toolpath into the view.
    Fit,
    /// Reset the camera and fit the toolpath.
    ResetView,
    /// Hide the sidebar.
    Hide,
}

/// The inspector's bounds value: `0.00 to 40.00 mm`.
fn range(min: f32, max: f32, units: MeasurementSystem, tr: &Translator) -> String {
    format!(
        "{} {} {} {}",
        format_length(min, units),
        tr.tr("to"),
        format_length(max, units),
        get_unit_label(units)
    )
}

/// Draws the sidebar's contents.
///
/// # Arguments
/// * `ui` - The sidebar panel's `Ui`.
/// * `bounds` - The program's bounds, once built.
/// * `units` - The measurement system to show.
/// * `tr` - Translator for the current UI language.
///
/// # Returns
/// The action of the button clicked this frame, if any.
pub fn show(
    ui: &mut egui::Ui,
    bounds: Option<&Aabb>,
    units: MeasurementSystem,
    tr: &Translator,
) -> Option<SidebarAction> {
    let mut action = None;
    ui.add_space(f32::from(crate::theme::PAGE_MARGIN) / 2.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(8.0, 8.0);
        if icon_only_button(ui, Icon::Fit, &tr.tr("Fit to Content")).clicked() {
            action = Some(SidebarAction::Fit);
        }
        if icon_only_button(ui, Icon::Restore, &tr.tr("Reset View")).clicked() {
            action = Some(SidebarAction::ResetView);
        }
        if named_icon_button(ui, Icon::Hide, &tr.tr("Hide"), &tr.tr("Hide Sidebar")) {
            action = Some(SidebarAction::Hide);
        }
    });
    ui.add_space(4.0);
    egui::CollapsingHeader::new(tr.tr("Inspector"))
        .id_salt("visualizer-inspector")
        .default_open(false)
        .show(ui, |ui| {
            let label = tr.tr("Bounds");
            for (axis, name) in ["X", "Y"].into_iter().enumerate() {
                let value = bounds.map_or_else(
                    || "-".to_owned(),
                    |b| range(b.min[axis], b.max[axis], units, tr),
                );
                ui.horizontal(|ui| {
                    ui.small(format!("{label} {name}"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.monospace(value)
                    });
                });
            }
        });
    action
}

/// The floating Show Sidebar button at the top left of the view `rect` while the sidebar
/// is hidden (GTK's `view-reveal` button).
///
/// # Returns
/// Whether it was clicked.
pub fn show_button(ui: &mut egui::Ui, rect: egui::Rect, tr: &Translator) -> bool {
    let margin = f32::from(crate::theme::PAGE_MARGIN);
    let strip = egui::Rect::from_min_max(
        rect.min + egui::vec2(margin, margin),
        egui::pos2(rect.center().x, rect.top() + margin + 36.0),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(strip), |ui| {
        let name = tr.tr("Show Sidebar");
        named_icon_button(ui, Icon::Show, &name, &name)
    })
    .inner
}
