//! The digital readout (DRO): machine state, work and machine positions of X, Y and Z, and
//! the current feed rate and spindle speed, from the latest status report.
//!
//! Positions are millimetres internally, shown in the user's measurement system with two
//! decimals (ported from the GTK `osd_format::format_length_with_unit` and the GTK status
//! report's DRO). A report carries either `MPos` or `WPos`; the
//! other is derived with the last work coordinate offset (`WCO`) when one is known.
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations).

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::{FeedRateUnits, MeasurementSystem, get_unit_label};

use super::jog::feed_to_display;

use crate::machine::{MachineStatus, Xyz};

/// The msgids this section shows.
pub(super) const MSGIDS: &[&str] = &[
    n_!("Machine State"),
    n_!("Work Coordinates (WPos)"),
    n_!("Machine Coordinates (MPos)"),
    n_!("Feed:"),
    n_!("Spindle:"),
];

/// Formats a length held in millimetres in `units`, with two decimals, e.g. `1.23`.
///
/// # Arguments
/// * `value_mm` - Length in millimetres.
/// * `units` - The measurement system to show.
pub(super) fn format_length(value_mm: f32, units: MeasurementSystem) -> String {
    let value = match units {
        MeasurementSystem::Metric => value_mm,
        MeasurementSystem::Imperial => value_mm / 25.4,
    };
    format!("{value:.2}")
}

/// Draws one position block.
fn show_position(
    ui: &mut egui::Ui,
    title: &str,
    id: &str,
    position: Option<Xyz>,
    units: MeasurementSystem,
) {
    ui.strong(format!("{title} ({})", get_unit_label(units)));
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([12.0, 2.0])
        .show(ui, |ui| {
            for (axis, i) in [("X", 0), ("Y", 1), ("Z", 2)] {
                ui.label(axis);
                let text = position.map_or_else(|| "-".to_string(), |p| format_length(p[i], units));
                ui.monospace(egui::RichText::new(text).size(18.0));
                ui.end_row();
            }
        });
}

/// Draws the readout.
pub(super) fn show(
    ui: &mut egui::Ui,
    tr: &Translator,
    status: &MachineStatus,
    units: MeasurementSystem,
    feed_units: FeedRateUnits,
) {
    let report = status.report.as_ref();
    ui.horizontal(|ui| {
        ui.strong(tr.tr("Machine State"));
        let state = report
            .and_then(|r| r.machine_state.clone())
            .unwrap_or_else(|| "-".to_string());
        ui.label(state);
    });
    let (wpos, mpos) = (status.work_position(), status.machine_position());
    let wpos_title = tr.tr("Work Coordinates (WPos)");
    show_position(ui, &wpos_title, "dro-wpos", wpos, units);
    let mpos_title = tr.tr("Machine Coordinates (MPos)");
    show_position(ui, &mpos_title, "dro-mpos", mpos, units);
    ui.horizontal_wrapped(|ui| {
        let feed = report.and_then(|r| r.feed_rate).map_or_else(
            || "-".to_string(),
            |f| format!("{:.2} {feed_units}", feed_to_display(f as f32, feed_units)),
        );
        let spindle = report
            .and_then(|r| r.spindle_speed)
            .map_or_else(|| "-".to_string(), |s| s.to_string());
        ui.label(format!("{} {feed}", tr.tr("Feed:")));
        ui.label(format!("{} {spindle}", tr.tr("Spindle:")));
    });
}
