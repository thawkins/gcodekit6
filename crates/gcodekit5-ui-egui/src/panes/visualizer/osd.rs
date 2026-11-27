//! The visualizer's on-screen display: the program's bounds and the tool position in the
//! bottom-left corner, the empty-state message when no program is loaded, and a note when
//! the toolpath is too large for the GPU.
//!
//! It shows the inspector's "Bounds X/Y" rows (the
//! cutting moves' bounds, else every move's) and its empty state. The "Mouse Controls" help
//! button is with the nav cube ([`super::nav_cube`]).
//! Lengths are in the user's measurement system, two decimals for mm and three for inches,
//! as the DRO shows them.
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations), `gcodekit5-core` (units).

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::{MeasurementSystem, get_unit_label};

use super::camera::Aabb;
use crate::machine::Xyz;
use crate::panes::machine_control::format_length;

/// The msgids the OSD shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Bounds {axis}: {min} to {max} {unit}"),
        n_!("Position"),
        n_!("No G-code loaded"),
        n_!("Open a file to preview toolpaths."),
        n_!("Toolpath too large to display on this GPU."),
        n_!("Could not draw this toolpath."),
    ]
}

/// The bounds rows: `Bounds X: 0.00 to 10.00 mm`, and the same for Y and Z.
///
/// # Arguments
/// * `bounds` - The box to describe, in mm.
/// * `units` - The measurement system to show.
/// * `tr` - Translator for the current UI language.
pub fn bounds_lines(bounds: &Aabb, units: MeasurementSystem, tr: &Translator) -> Vec<String> {
    let template = tr.tr("Bounds {axis}: {min} to {max} {unit}");
    let unit = get_unit_label(units);
    ["X", "Y", "Z"]
        .into_iter()
        .zip(bounds.min.to_array().into_iter().zip(bounds.max.to_array()))
        .map(|(axis, (min, max))| {
            template
                .replace("{axis}", axis)
                .replace("{min}", &format_length(min, units))
                .replace("{max}", &format_length(max, units))
                .replace("{unit}", unit)
        })
        .collect()
}

/// The tool position row: `Position X: 1.00  Y: 2.00  Z: 3.00 mm`.
///
/// # Arguments
/// * `xyz` - The work position in mm.
/// * `units` - The measurement system to show.
/// * `tr` - Translator for the current UI language.
pub fn position_line(xyz: Xyz, units: MeasurementSystem, tr: &Translator) -> String {
    format!(
        "{} X: {}  Y: {}  Z: {} {}",
        tr.tr("Position"),
        format_length(xyz[0], units),
        format_length(xyz[1], units),
        format_length(xyz[2], units),
        get_unit_label(units)
    )
}

/// A translucent panel of `rows` with its corner `anchor` placed at `pos`.
fn panel(ui: &mut egui::Ui, pos: egui::Pos2, anchor: egui::Align2, rows: &[String]) {
    let painter = ui.painter();
    let font = egui::FontId::monospace(12.0);
    let color = ui.visuals().text_color();
    let galleys: Vec<_> = rows
        .iter()
        .map(|row| painter.layout_no_wrap(row.clone(), font.clone(), color))
        .collect();
    let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let height: f32 = galleys.iter().map(|g| g.size().y).sum();
    let pad = egui::vec2(8.0, 6.0);
    let rect = anchor.anchor_size(pos, egui::vec2(width, height) + 2.0 * pad);
    let mut fill = ui.visuals().window_fill;
    fill = fill.gamma_multiply(0.85);
    painter.rect_filled(rect, 4.0, fill);
    let mut y = rect.top() + pad.y;
    for galley in galleys {
        let h = galley.size().y;
        painter.galley(egui::pos2(rect.left() + pad.x, y), galley, color);
        y += h;
    }
}

/// What the OSD describes.
#[derive(Debug, Clone, Copy, Default)]
pub struct Overlay<'a> {
    /// Whether a program is loaded; without one the empty-state message shows.
    pub loaded: bool,
    /// Whether the toolpath is too large for the GPU and is not drawn.
    pub too_large: bool,
    /// Whether the build of the program's toolpath failed; nothing is drawn for it.
    pub failed: bool,
    /// The program's bounds, once built, if it moves at all.
    pub bounds: Option<&'a Aabb>,
    /// The tool's work position, when the machine reports one.
    pub tool: Option<Xyz>,
    /// The lowest the bottom-left panel may reach (the top of the run preview bar).
    pub bottom: f32,
    /// A warning (msgids of its title and text) to show in the middle of the view.
    pub notice: Option<(&'a str, &'a str)>,
}

/// Draws the OSD over the view `rect`.
///
/// # Arguments
/// * `ui` - The visualizer's `Ui`.
/// * `rect` - The view's rectangle.
/// * `overlay` - What to describe.
/// * `units` - The measurement system to show.
/// * `tr` - Translator for the current UI language.
pub fn show(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    overlay: Overlay<'_>,
    units: MeasurementSystem,
    tr: &Translator,
) {
    let Overlay {
        loaded,
        too_large,
        failed,
        bounds,
        tool,
        bottom,
        notice,
    } = overlay;
    let margin = 10.0;
    let mut rows = bounds.map_or_else(Vec::new, |b| bounds_lines(b, units, tr));
    if let Some(xyz) = tool {
        rows.push(position_line(xyz, units, tr));
    }
    if !rows.is_empty() {
        let corner = egui::pos2(rect.left() + margin, bottom.min(rect.bottom()) - margin);
        panel(ui, corner, egui::Align2::LEFT_BOTTOM, &rows);
    }
    if too_large {
        let note = [tr
            .tr("Toolpath too large to display on this GPU.")
            .into_owned()];
        panel(ui, rect.center(), egui::Align2::CENTER_CENTER, &note);
    }
    if let Some((title, text)) = notice {
        let note = [tr.tr(title).into_owned(), tr.tr(text).into_owned()];
        panel(ui, rect.center(), egui::Align2::CENTER_CENTER, &note);
    }
    if failed {
        let note = [tr.tr("Could not draw this toolpath.").into_owned()];
        panel(ui, rect.center(), egui::Align2::CENTER_CENTER, &note);
    }
    if !loaded {
        let empty = [
            tr.tr("No G-code loaded").into_owned(),
            tr.tr("Open a file to preview toolpaths.").into_owned(),
        ];
        panel(ui, rect.center(), egui::Align2::CENTER_CENTER, &empty);
    }
}
