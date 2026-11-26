//! The digital readout (DRO): machine state, work and machine positions of X, Y and Z (and
//! of A, B and C in degrees when the machine reports them, as GTK shows them), and the
//! current feed rate and spindle speed, from the latest status report, with a Zero button
//! beside each work axis (`G10 L20 P<n> X0` in the active work coordinate system; a rotary
//! axis takes its calibration value instead of 0).
//!
//! Positions are millimetres internally, shown in the user's measurement system (two
//! decimals for mm, three for inches); ported from the GTK
//! `osd_format::format_length_with_unit` and the GTK status report's DRO. A report carries
//! either `MPos` or `WPos`; the other is derived with the last work coordinate offset
//! (`WCO`) when one is known.
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations).

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::Axis;
use gcodekit5_core::units::{FeedRateUnits, MeasurementSystem, get_unit_label};

use super::jog::feed_to_display;

use crate::icons::Icon;
use crate::machine::MachineStatus;

/// The msgids this section shows.
pub(super) const MSGIDS: &[&str] = &[
    n_!("Machine State"),
    n_!("Work Coordinates (WPos)"),
    n_!("Machine Coordinates (MPos)"),
    n_!("Feed:"),
    n_!("Spindle:"),
    n_!("Zero"),
    n_!("Set work axis to zero"),
    n_!("Buffer:"),
    n_!("Machine Limits"),
];

/// The degree sign rotary values are shown with, as GTK shows them.
const DEGREES: &str = "\u{b0}";

/// How the work position block offers its Zero buttons.
pub(super) struct ZeroButtons<'a> {
    /// Translator for the button texts.
    pub tr: &'a Translator,
    /// Whether the buttons are enabled.
    pub enabled: bool,
    /// Why they are disabled, shown on hover; `None` when enabled or obvious (offline).
    pub disabled_reason: Option<&'a str>,
}

/// Formats a length held in millimetres in `units`: mm with two decimals (`1.23`), inches
/// with three (`0.049`).
///
/// Three decimals for inches are a deliberate exception to the project's two-decimal rule,
/// which is about mm values: 0.01 in is 0.254 mm, too coarse to see the 0.001 in jog steps,
/// and the GTK front end showed inches with three decimals too.
///
/// # Arguments
/// * `value_mm` - Length in millimetres.
/// * `units` - The measurement system to show.
pub(crate) fn format_length(value_mm: f32, units: MeasurementSystem) -> String {
    match units {
        MeasurementSystem::Metric => format!("{value_mm:.2}"),
        MeasurementSystem::Imperial => format!("{:.3}", value_mm / 25.4),
    }
}

/// The size of the work position readout's text (GTK's large DRO labels).
const DRO_TEXT_SIZE: f32 = 22.0;

/// The work position block's title with the length unit: "Work Coordinates (WPos) (mm)".
pub(super) fn work_title(tr: &Translator, units: MeasurementSystem) -> String {
    format!(
        "{} ({})",
        tr.tr("Work Coordinates (WPos)"),
        get_unit_label(units)
    )
}

/// Draws the work position rows (`WX 0.00 [Zero]`), each with its Zero button; returns the
/// axis whose Zero was clicked.
pub(super) fn show_work(
    ui: &mut egui::Ui,
    status: &MachineStatus,
    units: MeasurementSystem,
    zero: &ZeroButtons,
) -> Option<Axis> {
    let position = status.work_position();
    let mut clicked = None;
    egui::Grid::new("dro-wpos")
        .num_columns(3)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            for (axis, i) in [(Axis::X, 0), (Axis::Y, 1), (Axis::Z, 2)] {
                let letter = axis.letter();
                ui.label(
                    egui::RichText::new(format!("W{letter}"))
                        .size(DRO_TEXT_SIZE)
                        .strong(),
                );
                let text = position.map_or_else(|| "-".to_string(), |p| format_length(p[i], units));
                ui.add_sized(
                    [110.0, 32.0],
                    egui::Label::new(egui::RichText::new(text).monospace().size(DRO_TEXT_SIZE)),
                );
                if zero_button(ui, zero, letter) {
                    clicked = Some(axis);
                }
                ui.end_row();
            }
            // The rotary axes the machine reports, in degrees (GTK's A/B/C rows).
            let rotary = status.work_rotary();
            let reported = status.rotary_reported();
            for (i, axis) in [Axis::A, Axis::B, Axis::C].into_iter().enumerate() {
                if !reported[i] {
                    continue;
                }
                let letter = axis.letter();
                ui.label(
                    egui::RichText::new(format!("W{letter}"))
                        .size(DRO_TEXT_SIZE)
                        .strong(),
                );
                let text =
                    rotary[i].map_or_else(|| "-".to_string(), |v| format!("{v:.2}{DEGREES}"));
                ui.add_sized(
                    [110.0, 32.0],
                    egui::Label::new(egui::RichText::new(text).monospace().size(DRO_TEXT_SIZE)),
                );
                if zero_button(ui, zero, letter) {
                    clicked = Some(axis);
                }
                ui.end_row();
            }
        });
    clicked
}

/// Draws the machine position block: its title and one `MX: 0.00  MY: 0.00  MZ: 0.00` line.
pub(super) fn show_machine(
    ui: &mut egui::Ui,
    tr: &Translator,
    status: &MachineStatus,
    units: MeasurementSystem,
) {
    let title = tr.tr("Machine Coordinates (MPos)");
    crate::theme::section_heading(ui, &format!("{title} ({})", get_unit_label(units)));
    let position = status.machine_position();
    ui.horizontal(|ui| {
        for (letter, i) in [('X', 0), ('Y', 1), ('Z', 2)] {
            let text = position.map_or_else(|| "-".to_string(), |p| format_length(p[i], units));
            // The value is a label of its own, so it reads (and is found) as the number.
            ui.label(format!("M{letter}:"));
            ui.label(text);
            ui.add_space(8.0);
        }
        let rotary = status.machine_rotary();
        let reported = status.rotary_reported();
        for (i, letter) in ['A', 'B', 'C'].into_iter().enumerate() {
            if reported[i] {
                let text =
                    rotary[i].map_or_else(|| "-".to_string(), |v| format!("{v:.2}{DEGREES}"));
                ui.label(format!("M{letter}:"));
                ui.label(text);
                ui.add_space(8.0);
            }
        }
    });
}

/// Draws one axis's Zero button, named "Zero X" for assistive technology; returns whether
/// it was clicked.
fn zero_button(ui: &mut egui::Ui, zero: &ZeroButtons, letter: char) -> bool {
    let tr = zero.tr;
    let name = format!("{} {letter}", tr.tr("Zero"));
    let button = egui::Button::image_and_text(Icon::Zero.themed(ui), tr.tr("Zero"));
    let response = ui.add_enabled(zero.enabled, button);
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, zero.enabled, name.clone()));
    let tip = format!("{} {letter}", tr.tr("Set work axis to zero"));
    response
        .on_hover_text(&tip)
        .on_disabled_hover_text(zero.disabled_reason.unwrap_or(&tip))
        .clicked()
}

/// Draws the Machine State section's readout: the state in large text (the connection
/// state while offline, as GTK shows "Disconnected"), then the feed rate and spindle speed.
pub(super) fn show_state(
    ui: &mut egui::Ui,
    tr: &Translator,
    status: &MachineStatus,
    connecting: bool,
    feed_units: FeedRateUnits,
) {
    let report = status.report.as_ref();
    let state = match report.and_then(|r| r.machine_state.clone()) {
        Some(state) if status.connected => state,
        Some(_) | None => super::connection::state_text(tr, status, connecting).into_owned(),
    };
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(state).size(20.0).strong());
    });
    ui.horizontal_wrapped(|ui| {
        let feed = report.and_then(|r| r.feed_rate).map_or_else(
            || "-".to_string(),
            |f| format!("{:.2} {feed_units}", feed_to_display(f as f32, feed_units)),
        );
        let spindle = report
            .and_then(|r| r.spindle_speed)
            .map_or_else(|| "-".to_string(), |s| s.to_string());
        ui.weak(format!("{} {feed}", tr.tr("Feed:")));
        ui.weak(format!("{} {spindle}", tr.tr("Spindle:")));
        let buffer = status.buffer_text().unwrap_or_else(|| "-".to_string());
        ui.weak(format!("{} {buffer}", tr.tr("Buffer:")));
    });
}

/// The configured travel the Machine Limits section falls back to: X, Y, Z in mm and A, B,
/// C in degrees (Preferences > Machine).
pub(super) type ConfiguredLimits = [f32; 6];

/// The Machine Limits rows as `(axis, text)`: X, Y and Z from the controller's max travel
/// (`$130`-`$132`) once read, else the configured limit, in `units`; then each rotary axis
/// the machine reports, from its configured limit in degrees. As GTK's
/// `refresh_limit_labels`.
///
/// # Arguments
/// * `status` - The machine status.
/// * `units` - The measurement system to show lengths in.
/// * `configured` - The configured limits.
pub(super) fn limit_rows(
    status: &MachineStatus,
    units: MeasurementSystem,
    configured: &ConfiguredLimits,
) -> Vec<(char, String)> {
    let unit = get_unit_label(units);
    let mut rows: Vec<(char, String)> = ['X', 'Y', 'Z']
        .into_iter()
        .enumerate()
        .map(|(i, axis)| {
            let max = status.grbl_travel[i].unwrap_or(configured[i]);
            (axis, format!("0 - {} {unit}", format_length(max, units)))
        })
        .collect();
    let reported = status.rotary_reported();
    for (i, axis) in ['A', 'B', 'C'].into_iter().enumerate() {
        if reported[i] {
            rows.push((axis, format!("0 - {:.1}{DEGREES}", configured[3 + i])));
        }
    }
    rows
}

/// Draws the Machine Limits section ([`limit_rows`]).
pub(super) fn show_limits(
    ui: &mut egui::Ui,
    tr: &Translator,
    status: &MachineStatus,
    units: MeasurementSystem,
    configured: &ConfiguredLimits,
) {
    crate::theme::section_heading(ui, &tr.tr("Machine Limits"));
    egui::Grid::new("machine-control-limits")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            for (axis, text) in limit_rows(status, units, configured) {
                ui.weak(format!("{axis}:"));
                ui.label(text);
                ui.end_row();
            }
        });
}
