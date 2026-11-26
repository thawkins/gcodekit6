//! The digital readout (DRO): machine state, work and machine positions of X, Y and Z, and
//! the current feed rate and spindle speed, from the latest status report, with a Zero
//! button beside each work axis (`G10 L20 P<n> X0` in the active work coordinate system).
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

use crate::machine::{MachineStatus, Xyz};

/// The msgids this section shows.
pub(super) const MSGIDS: &[&str] = &[
    n_!("Machine State"),
    n_!("Work Coordinates (WPos)"),
    n_!("Machine Coordinates (MPos)"),
    n_!("Feed:"),
    n_!("Spindle:"),
    n_!("Zero"),
    n_!("Set work axis to zero"),
];

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

/// Draws one position block, with a Zero button per axis when `zero` is given; returns the
/// axis whose Zero was clicked.
fn show_position(
    ui: &mut egui::Ui,
    title: &str,
    id: &str,
    position: Option<Xyz>,
    units: MeasurementSystem,
    zero: Option<&ZeroButtons>,
) -> Option<Axis> {
    ui.strong(format!("{title} ({})", get_unit_label(units)));
    let mut clicked = None;
    egui::Grid::new(id)
        .num_columns(if zero.is_some() { 3 } else { 2 })
        .spacing([12.0, 2.0])
        .show(ui, |ui| {
            for (axis, i) in [(Axis::X, 0), (Axis::Y, 1), (Axis::Z, 2)] {
                let letter = axis.letter();
                ui.label(letter.to_string());
                let text = position.map_or_else(|| "-".to_string(), |p| format_length(p[i], units));
                ui.monospace(egui::RichText::new(text).size(18.0));
                if let Some(zero) = zero
                    && zero_button(ui, zero, letter)
                {
                    clicked = Some(axis);
                }
                ui.end_row();
            }
        });
    clicked
}

/// Draws one axis's Zero button, named "Zero X" for assistive technology; returns whether
/// it was clicked.
fn zero_button(ui: &mut egui::Ui, zero: &ZeroButtons, letter: char) -> bool {
    let tr = zero.tr;
    let name = format!("{} {letter}", tr.tr("Zero"));
    let response = ui.add_enabled(zero.enabled, egui::Button::new(tr.tr("Zero")));
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, zero.enabled, name.clone()));
    let tip = format!("{} {letter}", tr.tr("Set work axis to zero"));
    response
        .on_hover_text(&tip)
        .on_disabled_hover_text(zero.disabled_reason.unwrap_or(&tip))
        .clicked()
}

/// Draws the readout; returns the work axis whose Zero was clicked.
pub(super) fn show(
    ui: &mut egui::Ui,
    tr: &Translator,
    status: &MachineStatus,
    units: MeasurementSystem,
    feed_units: FeedRateUnits,
    zero: &ZeroButtons,
) -> Option<Axis> {
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
    let zeroed = show_position(ui, &wpos_title, "dro-wpos", wpos, units, Some(zero));
    let mpos_title = tr.tr("Machine Coordinates (MPos)");
    show_position(ui, &mpos_title, "dro-mpos", mpos, units, None);
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
    zeroed
}
