//! The digital readout (DRO): machine state, work and machine positions of X, Y and Z, and
//! the current feed rate and spindle speed, from the latest status report.
//!
//! Positions are millimetres, shown with two decimals (ported from the GTK
//! `osd_format::format_length_with_unit`). A report carries either `MPos` or `WPos`; the
//! other is derived with the last work coordinate offset (`WCO`) when one is known.
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations).

use gcodekit5_app::i18n::Translator;

use crate::machine::MachineStatus;

/// The msgids this section shows.
pub(super) const MSGIDS: &[&str] = &[
    n_!("Machine State"),
    n_!("Work Coordinates (WPos)"),
    n_!("Machine Coordinates (MPos)"),
    n_!("Feed:"),
    n_!("Spindle:"),
];

/// Formats a length in millimetres with two decimals, e.g. `1.23`.
///
/// # Arguments
/// * `value_mm` - Length in millimetres.
pub(super) fn format_mm(value_mm: f32) -> String {
    format!("{value_mm:.2}")
}

/// X, Y and Z in millimetres.
type Xyz = [f32; 3];

/// The work and machine positions of the latest report, each when known.
fn positions(status: &MachineStatus) -> (Option<Xyz>, Option<Xyz>) {
    let Some(report) = &status.report else {
        return (None, None);
    };
    let wco = status.wco.map(|o| [o.x as f32, o.y as f32, o.z as f32]);
    let mpos = report.mpos.map(|p| [p.x as f32, p.y as f32, p.z as f32]);
    let wpos = report.wpos.map(|p| [p.x as f32, p.y as f32, p.z as f32]);
    let combine = |a: Xyz, b: Xyz, sign: f32| [0, 1, 2].map(|i| a[i] + sign * b[i]);
    let wpos = wpos.or_else(|| Some(combine(mpos?, wco?, -1.0)));
    let mpos = mpos.or_else(|| Some(combine(wpos?, wco?, 1.0)));
    (wpos, mpos)
}

/// Draws one position block.
fn show_position(ui: &mut egui::Ui, title: &str, id: &str, position: Option<Xyz>) {
    ui.strong(title);
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([12.0, 2.0])
        .show(ui, |ui| {
            for (axis, i) in [("X", 0), ("Y", 1), ("Z", 2)] {
                ui.label(axis);
                let text = position.map_or_else(|| "-".to_string(), |p| format_mm(p[i]));
                ui.monospace(egui::RichText::new(text).size(18.0));
                ui.end_row();
            }
        });
}

/// Draws the readout.
pub(super) fn show(ui: &mut egui::Ui, tr: &Translator, status: &MachineStatus) {
    let report = status.report.as_ref();
    ui.horizontal(|ui| {
        ui.strong(tr.tr("Machine State"));
        let state = report
            .and_then(|r| r.machine_state.clone())
            .unwrap_or_else(|| "-".to_string());
        ui.label(state);
    });
    let (wpos, mpos) = positions(status);
    show_position(ui, &tr.tr("Work Coordinates (WPos)"), "dro-wpos", wpos);
    show_position(ui, &tr.tr("Machine Coordinates (MPos)"), "dro-mpos", mpos);
    ui.horizontal_wrapped(|ui| {
        let feed = report
            .and_then(|r| r.feed_rate)
            .map_or_else(|| "-".to_string(), |f| format!("{f:.0}"));
        let spindle = report
            .and_then(|r| r.spindle_speed)
            .map_or_else(|| "-".to_string(), |s| s.to_string());
        ui.label(format!("{} {feed}", tr.tr("Feed:")));
        ui.label(format!("{} {spindle}", tr.tr("Spindle:")));
    });
}
