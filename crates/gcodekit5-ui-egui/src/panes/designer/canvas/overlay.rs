//! The canvas's floating controls, as GTK's on-screen display: the view toolbar at the top
//! centre (zoom out, fit to content, fit to viewport, fit to the work area, zoom in) and the
//! status line at the bottom left (zoom, view centre, cursor, snap, units).
//!
//! Not ported: GTK's scrollbar toggle and keyboard-shortcut popover (the canvas has no
//! scrollbars, and no shortcut list exists here).
//!
//! Dependencies: `egui`, `gcodekit5-core` (units), `gcodekit5-app` (translations).

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::{self, MeasurementSystem};

use crate::icons::{self, Icon};

/// The msgids the overlay shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Zoom Out"),
        n_!("Fit to Content"),
        n_!("Fit to Viewport"),
        n_!("Fit to Device Working Area"),
        n_!("Zoom In"),
        n_!("Zoom"),
        n_!("Center"),
        n_!("Cursor"),
        n_!("Snap"),
    ]
}

/// A view change asked for on the toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewAction {
    /// Zoom out one step.
    ZoomOut,
    /// Fit every shape into the view.
    FitContent,
    /// 100% with the origin at the bottom-left corner.
    FitViewport,
    /// Fit the work area into the view.
    FitWorkArea,
    /// Zoom in one step.
    ZoomIn,
}

/// The toolbar's buttons: action, icon and tooltip msgid, in GTK's order.
const BUTTONS: [(ViewAction, Icon, &str); 5] = [
    (ViewAction::ZoomOut, Icon::ZoomOut, n_!("Zoom Out")),
    (ViewAction::FitContent, Icon::Fit, n_!("Fit to Content")),
    (
        ViewAction::FitViewport,
        Icon::Restore,
        n_!("Fit to Viewport"),
    ),
    (
        ViewAction::FitWorkArea,
        Icon::Display,
        n_!("Fit to Device Working Area"),
    ),
    (ViewAction::ZoomIn, Icon::ZoomIn, n_!("Zoom In")),
];

/// Gap between the overlays and the canvas edge in points.
const MARGIN: f32 = 10.0;

/// Height the toolbar takes at the top of the canvas, margins included.
pub const TOOLBAR_HEIGHT: f32 = 48.0;

/// The floating frame both overlays are drawn in.
fn osd_frame(ui: &egui::Ui) -> egui::Frame {
    let visuals = ui.visuals();
    egui::Frame::new()
        .fill(visuals.window_fill.gamma_multiply(0.92))
        .stroke(visuals.window_stroke)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(6, 4))
}

/// The toolbar at the top centre of the canvas `rect`.
///
/// # Arguments
/// * `ui` - The canvas's `Ui`; the toolbar floats above it in its own area.
/// * `rect` - The canvas.
/// * `tr` - Translator for the current UI language.
///
/// # Returns
/// The action clicked this frame, if any.
pub fn toolbar(ui: &mut egui::Ui, rect: egui::Rect, tr: &Translator) -> Option<ViewAction> {
    let mut clicked = None;
    // An area pivoted on its top centre is centred from its own size.
    egui::Area::new(egui::Id::unique("gcodekit5-designer-canvas-toolbar"))
        .order(egui::Order::Middle)
        .fixed_pos(egui::pos2(rect.center().x, rect.top() + MARGIN))
        .pivot(egui::Align2::CENTER_TOP)
        .constrain_to(rect)
        .show(ui.ctx(), |ui| {
            osd_frame(ui).show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (action, icon, msgid) in BUTTONS {
                        if icons::icon_only_button(ui, icon, &tr.tr(msgid)).clicked() {
                            clicked = Some(action);
                        }
                    }
                });
            });
        });
    clicked
}

/// What the status line shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Status {
    /// The zoom factor (1.0 is 100%).
    pub zoom: f64,
    /// The world point at the canvas centre, mm.
    pub center: (f64, f64),
    /// The last world point under the pointer, mm.
    pub cursor: (f64, f64),
    /// Whether snapping is on.
    pub snap: bool,
    /// The display units.
    pub units: MeasurementSystem,
}

/// `mm` in the display units with two decimals.
fn length(mm: f64, system: MeasurementSystem) -> String {
    let shown = match system {
        MeasurementSystem::Metric => mm,
        MeasurementSystem::Imperial => mm / 25.4,
    };
    format!("{shown:.2}")
}

/// The status line's text, as GTK's `format_zoom_center_cursor` plus the snap flag and the
/// unit label.
///
/// # Arguments
/// * `status` - The values shown.
/// * `tr` - Translator for the current UI language.
pub fn status_text(status: &Status, tr: &Translator) -> String {
    let u = status.units;
    let mut text = format!(
        "{}: {:.0}%  {}: X {} Y {}  {}: X {} Y {}",
        tr.tr("Zoom"),
        status.zoom * 100.0,
        tr.tr("Center"),
        length(status.center.0, u),
        length(status.center.1, u),
        tr.tr("Cursor"),
        length(status.cursor.0, u),
        length(status.cursor.1, u),
    );
    if status.snap {
        text.push_str(&format!("  {}", tr.tr("Snap")));
    }
    text.push_str(&format!("  {}", units::get_unit_label(u)));
    text
}

/// The status line at the bottom left of the canvas `rect`; it takes no input.
///
/// # Arguments
/// * `ui` - The canvas's `Ui`.
/// * `rect` - The canvas.
/// * `status` - The values shown.
/// * `tr` - Translator for the current UI language.
pub fn status_line(ui: &mut egui::Ui, rect: egui::Rect, status: &Status, tr: &Translator) {
    let height = 28.0;
    let strip = egui::Rect::from_min_max(
        egui::pos2(rect.left() + MARGIN, rect.bottom() - height - MARGIN),
        egui::pos2(rect.right() - MARGIN, rect.bottom() - MARGIN),
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(strip)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            osd_frame(ui).show(ui, |ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(status_text(status, tr)).small())
                        .selectable(false)
                        .truncate(),
                );
            });
        },
    );
}
