//! The status bar at the bottom of the window, laid out as GTK's: the eStop button, the
//! connection, machine state and position, the active device profile
//! (`<name> | <firmware> | <type>`), and on the right the job progress and the last error.
//!
//! [`summary`] turns the app's [`MachineStatus`] into the strings the bar shows, so they can
//! be tested without a window; [`show`] draws them. The position is the machine position in
//! the user's measurement system (two decimals for mm, three for inches, through
//! `format_length` as the DRO does).
//!
//! Ported from the GTK `ui/gtk/status_bar.rs`; the remaining time is estimated as GTK's
//! `update_progress` does, from the average time per acknowledged line. The feed comes from
//! the status report (as the Machine Control DRO's) with two decimals and its unit, where
//! GTK showed the commanded feed with three decimals and no unit. Not ported: the timed
//! notices; the connected port is not known to the status.
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations), `gcodekit5-core` (units),
//! `gcodekit5-devicedb` (the active profile).

use std::time::Duration;

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::{FeedRateUnits, MeasurementSystem, get_unit_label};
use gcodekit5_devicedb::DeviceProfile;

use crate::machine::{MachineStatus, Progress, Xyz};
use crate::panes::machine_control::format_length;

/// The msgids the bar shows, for the translation check.
pub const MSGIDS: &[&str] = &[
    n_!("Connected"),
    n_!("Disconnected"),
    n_!("eStop"),
    n_!("No device selected"),
    n_!("Emergency stop (soft reset, {shortcut})"),
    n_!("Elapsed:"),
    n_!("Remaining:"),
];

/// What the status bar shows, as text.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    /// Whether the machine is connected.
    pub connected: bool,
    /// "Connected" or "Disconnected", translated.
    pub connection: String,
    /// The machine state of the latest report (`Idle`, `Run`, `Hold:0`, ...).
    pub state: Option<String>,
    /// The machine position, see [`position_text`].
    pub position: Option<String>,
    /// The job's progress as a fraction in `0.0..=1.0` and its label (`25.0%`), from the
    /// acknowledged lines; `None` when no job ran.
    pub progress: Option<(f32, String)>,
    /// While a job streams: `Elapsed: HH:MM:SS` and `Remaining: HH:MM:SS`, translated.
    pub times: Option<(String, String)>,
    /// The last error the controller or transport reported.
    pub error: Option<String>,
}

/// The position as `X: 1.23  Y: 2.00  Z: -3.10 (mm)` in `units`.
///
/// # Arguments
/// * `xyz` - X, Y and Z in millimetres.
/// * `units` - The measurement system to show.
pub fn position_text(xyz: Xyz, units: MeasurementSystem) -> String {
    format!(
        "X: {}  Y: {}  Z: {} ({})",
        format_length(xyz[0], units),
        format_length(xyz[1], units),
        format_length(xyz[2], units),
        get_unit_label(units)
    )
}

/// A job time as GTK's status bar shows it: `HH:MM:SS`, seconds truncated.
///
/// # Arguments
/// * `time` - The duration to show.
pub fn format_time(time: Duration) -> String {
    let secs = time.as_secs();
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// The job's elapsed and estimated remaining time, formatted by [`format_time`].
///
/// The estimate is GTK's: the average time per acknowledged line so far, times the lines
/// not yet acknowledged; zero until a line has been acknowledged.
///
/// # Arguments
/// * `progress` - The stream's progress.
/// * `elapsed` - The time since the stream started.
pub fn job_times(progress: Progress, elapsed: Duration) -> (String, String) {
    let remaining = match u32::try_from(progress.acked) {
        Ok(acked) if acked > 0 && !elapsed.is_zero() => {
            let left = progress.total.saturating_sub(progress.acked) as f64;
            Duration::from_secs_f64(elapsed.as_secs_f64() / f64::from(acked) * left)
        }
        Ok(_) | Err(_) => Duration::ZERO,
    };
    (format_time(elapsed), format_time(remaining))
}

/// The feed and spindle readout, `F: 1500.00 mm/min  S: 12000 RPM`, from the latest status
/// report; `None` while disconnected or until a report gives both, as GTK.
///
/// # Arguments
/// * `status` - The app's machine status.
/// * `feed_units` - The feed rate units to show.
pub fn feed_spindle_text(status: &MachineStatus, feed_units: FeedRateUnits) -> Option<String> {
    let report = status.report.as_ref().filter(|_| status.connected)?;
    let feed = crate::panes::machine_control::feed_to_display(report.feed_rate? as f32, feed_units);
    Some(format!(
        "F: {feed:.2} {feed_units}  S: {} RPM",
        report.spindle_speed?
    ))
}

/// What the bar shows for `status`.
///
/// # Arguments
/// * `status` - The app's machine status.
/// * `units` - The measurement system lengths are shown in.
/// * `tr` - Translator for the current UI language.
pub fn summary(status: &MachineStatus, units: MeasurementSystem, tr: &Translator) -> Summary {
    let connection = if status.connected {
        tr.tr("Connected")
    } else {
        tr.tr("Disconnected")
    };
    let progress = status.progress.filter(|p| p.total > 0).map(|p| {
        let fraction = (p.acked as f32 / p.total as f32).clamp(0.0, 1.0);
        (fraction, format!("{:.1}%", fraction * 100.0))
    });
    Summary {
        connected: status.connected,
        connection: connection.into_owned(),
        state: status
            .report
            .as_ref()
            .and_then(|r| r.machine_state.clone())
            .filter(|_| status.connected),
        position: status
            .machine_position()
            .filter(|_| status.connected)
            .map(|xyz| position_text(xyz, units)),
        progress,
        times: status
            .progress
            .zip(status.stream_started)
            .filter(|_| status.streaming)
            .map(|(progress, started)| {
                let (elapsed, remaining) = job_times(progress, started.elapsed());
                (
                    format!("{} {elapsed}", tr.tr("Elapsed:")),
                    format!("{} {remaining}", tr.tr("Remaining:")),
                )
            }),
        error: status
            .last_error
            .as_deref()
            .map(|message| crate::machine::worker_message(message, tr)),
    }
}

/// The active device profile as GTK's status bar shows it: `<name> | <firmware> | <type>`,
/// or "No device selected".
///
/// # Arguments
/// * `profile` - The active device profile, if one is set.
/// * `tr` - Translator for the current UI language.
pub fn device_text(profile: Option<&DeviceProfile>, tr: &Translator) -> String {
    match profile {
        Some(p) => format!("{} | {:?} | {:?}", p.name, p.controller_type, p.device_type),
        None => tr.tr("No device selected").into_owned(),
    }
}

/// Smallest size of the bar's eStop, the only emergency stop button on the pages other than
/// Machine Control: wider and taller than a plain button so it is always easy to hit.
pub const ESTOP_MIN_SIZE: egui::Vec2 = egui::vec2(72.0, 28.0);

/// Draws the bar's contents into `ui` (a bottom panel's).
///
/// # Arguments
/// * `ui` - The panel's `Ui`.
/// * `tr` - Translator for the current UI language.
/// * `status` - The app's machine status.
/// * `units` - The measurement system lengths are shown in.
/// * `feed_units` - The feed rate units to show.
/// * `profile` - The active device profile, if one is set.
///
/// # Returns
/// Whether the user clicked eStop; the caller queues [`crate::commands::Command::SoftReset`],
/// which the Machine Control pane runs as its own E-STOP button does.
pub fn show(
    ui: &mut egui::Ui,
    tr: &Translator,
    status: &MachineStatus,
    units: MeasurementSystem,
    feed_units: FeedRateUnits,
    profile: Option<&DeviceProfile>,
) -> bool {
    let summary = summary(status, units, tr);
    let mut estop_clicked = false;
    ui.horizontal(|ui| {
        let estop = egui::Button::new(
            egui::RichText::new(tr.tr("eStop"))
                .strong()
                .color(egui::Color32::WHITE),
        )
        .fill(egui::Color32::from_rgb(0xc0, 0x1c, 0x28))
        .min_size(ESTOP_MIN_SIZE);
        estop_clicked = ui
            .add_enabled(summary.connected, estop)
            .on_hover_text(tr.tr("Emergency stop (soft reset, {shortcut})").replace(
                "{shortcut}",
                crate::panes::machine_control::SOFT_RESET_CHORD,
            ))
            .clicked();

        // Painted: the bundled fonts have no bullet glyph.
        let colour = if summary.connected {
            egui::Color32::from_rgb(0x2e, 0xa0, 0x43)
        } else {
            ui.visuals().weak_text_color()
        };
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(10.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.0, colour);
        ui.label(&summary.connection);
        if let Some(state) = &summary.state {
            ui.separator();
            ui.strong(state);
        }
        if let Some(position) = &summary.position {
            ui.separator();
            ui.monospace(position);
        }
        if let Some(feed_spindle) = feed_spindle_text(status, feed_units) {
            ui.separator();
            ui.monospace(feed_spindle);
        }
        ui.add_space(8.0);
        ui.weak(device_text(profile, tr));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some((fraction, label)) = &summary.progress {
                ui.add(
                    egui::ProgressBar::new(*fraction)
                        .desired_width(120.0)
                        .text(label),
                );
            }
            if let Some((elapsed, remaining)) = &summary.times {
                ui.label(remaining);
                ui.label(elapsed);
                // The elapsed time counts up between status reports too.
                ui.ctx().request_repaint_after(Duration::from_secs(1));
            }
            if let Some(error) = &summary.error {
                ui.add(
                    egui::Label::new(egui::RichText::new(error).color(ui.visuals().error_fg_color))
                        .truncate(),
                );
            }
        });
    });
    estop_clicked
}
