//! The status bar at the bottom of the window: connection, machine state, position, job
//! progress and the last error.
//!
//! [`summary`] turns the app's [`MachineStatus`] into the strings the bar shows, so they can
//! be tested without a window; [`show`] draws them. The position is the machine position in
//! the user's measurement system (two decimals for mm, three for inches, through
//! `format_length` as the DRO does).
//!
//! Ported from the GTK `ui/gtk/status_bar.rs`. Not ported: the E-stop button (Machine
//! Control has it), the device name, feed/spindle readout, elapsed/remaining time and the
//! timed notices; the connected port is not known to the status.
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations), `gcodekit5-core` (units).

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::{MeasurementSystem, get_unit_label};

use crate::machine::{MachineStatus, Xyz};
use crate::panes::machine_control::format_length;

/// The msgids the bar shows, for the translation check.
pub const MSGIDS: &[&str] = &[
    n_!("Connected"),
    n_!("Disconnected"),
    n_!("E-STOP"),
    n_!("Emergency stop (soft reset, {shortcut})"),
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
        error: status
            .last_error
            .as_deref()
            .map(|message| crate::machine::worker_message(message, tr)),
    }
}

/// Draws the bar's contents into `ui` (a bottom panel's).
///
/// # Arguments
/// * `ui` - The panel's `Ui`.
/// * `tr` - Translator for the current UI language.
/// * `status` - The app's machine status.
/// * `units` - The measurement system lengths are shown in.
///
/// # Returns
/// Whether the user clicked E-STOP; the caller queues [`crate::commands::Command::SoftReset`],
/// which the Machine Control pane runs as its own E-STOP button does.
pub fn show(
    ui: &mut egui::Ui,
    tr: &Translator,
    status: &MachineStatus,
    units: MeasurementSystem,
) -> bool {
    let summary = summary(status, units, tr);
    let mut estop_clicked = false;
    ui.horizontal(|ui| {
        let estop = egui::Button::new(
            egui::RichText::new(tr.tr("E-STOP"))
                .strong()
                .color(egui::Color32::WHITE),
        )
        .fill(egui::Color32::from_rgb(0xc0, 0x1c, 0x28));
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
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some((fraction, label)) = &summary.progress {
                ui.add(
                    egui::ProgressBar::new(*fraction)
                        .desired_width(120.0)
                        .text(label),
                );
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
