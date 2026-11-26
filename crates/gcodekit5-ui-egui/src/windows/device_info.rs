//! The Device Info panel, the left column of the Device Config page as in GTK: what the
//! controller reports about itself (name, firmware, version, build date, compile options),
//! its capabilities (homing `$22`, laser mode `$32`), and the device type selector that
//! writes `$32`.
//!
//! - The identity follows the controller's lines whether the page is shown or not: the
//!   startup banner, the `$I` reply (`[VER:...]`, `[OPT:...]`), Repetier's `M115` reply and
//!   `$22`/`$32` from a `$$` dump. A disconnect forgets it.
//! - **Refresh device info** sends `$I` then `$$`; **Copy device info** copies the name,
//!   firmware and version (GTK's three lines).
//! - Choosing CNC or Laser writes `$32=0` or `$32=1` through [`PendingWrite`]; Other leaves
//!   `$32` alone. The selector is enabled only while connected, and a write is refused with
//!   a notice while a job runs or the machine is not Idle. The selector follows `$32` as
//!   the controller reports it ([`DeviceType::follow_laser_mode`]).
//!
//! Dependencies: `egui`, `gcodekit5-app` (identity model, controller, translations).

use gcodekit5_app::device_identity::{DeviceIdentity, DeviceType};
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::MachineEvent;

use super::setting_write::{self, PendingWrite, WriteOutcome};
use crate::icons::{Icon, icon_only_button};
use crate::machine::{Machine, worker_message};

/// A device type write waiting for the controller's answer.
#[derive(Debug, Clone)]
struct TypeWrite {
    pending: PendingWrite,
    /// The selection to go back to when the write fails.
    previous: DeviceType,
}

/// The Device Info panel.
#[derive(Debug, Default)]
pub struct DeviceInfoPanel {
    identity: DeviceIdentity,
    device_type: DeviceType,
    /// Whether the selection already followed `$32` on this connection.
    type_initialised: bool,
    /// The latest notice and whether it reports a failure.
    notice: Option<(String, bool)>,
    write: Option<TypeWrite>,
}

impl DeviceInfoPanel {
    /// What the controller reported about itself.
    pub fn identity(&self) -> &DeviceIdentity {
        &self.identity
    }

    /// The selected device type.
    pub fn device_type(&self) -> DeviceType {
        self.device_type
    }

    /// The latest notice (a refused or answered `$32` write), if any.
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_ref().map(|(text, _)| text.as_str())
    }

    /// Takes the controller's events, in order, whether the page is shown or not.
    ///
    /// # Arguments
    /// * `events` - Events no observer has seen yet.
    /// * `tr` - Translator for the notices.
    pub fn machine_events(&mut self, events: &[MachineEvent], tr: &Translator) {
        for event in events {
            match event {
                MachineEvent::Received(line) => {
                    let laser = self.identity.laser;
                    self.identity.apply_line(line);
                    if self.identity.laser != laser || line.trim().starts_with("$32=") {
                        self.follow_laser_mode();
                    }
                }
                MachineEvent::Disconnected => {
                    self.identity = DeviceIdentity::default();
                    self.type_initialised = false;
                    self.notice = None;
                }
                MachineEvent::Connected
                | MachineEvent::StatusReport(_)
                | MachineEvent::StreamProgress { .. }
                | MachineEvent::StreamHalted { .. }
                | MachineEvent::ProtocolMismatch { .. }
                | MachineEvent::StreamFinished { .. }
                | MachineEvent::StreamStopped { .. }
                | MachineEvent::Sent(_)
                | MachineEvent::Error(_) => {}
            }
            if let Some(write) = self.write.as_mut()
                && let Some(outcome) = write.pending.observe(event)
            {
                self.finish_write(outcome, tr);
            }
        }
        self.tick(tr);
    }

    /// Ends a write whose answer is overdue.
    fn tick(&mut self, tr: &Translator) {
        if self.write.as_ref().is_some_and(|w| w.pending.timed_out()) {
            self.finish_write(WriteOutcome::NoReply, tr);
        }
    }

    /// Moves the selection to match the reported `$32`.
    fn follow_laser_mode(&mut self) {
        let Some(laser) = self.identity.laser else {
            return;
        };
        if let Some(wanted) =
            DeviceType::follow_laser_mode(self.device_type, self.type_initialised, laser)
        {
            self.device_type = wanted;
        }
        self.type_initialised = true;
    }

    fn finish_write(&mut self, outcome: WriteOutcome, tr: &Translator) {
        let Some(write) = self.write.take() else {
            return;
        };
        let reason = match outcome {
            WriteOutcome::Accepted => {
                let sent = tr.tr(n_!("Sent"));
                self.notice = Some((format!("{sent} {}", write.pending.line()), false));
                return;
            }
            WriteOutcome::Rejected(reply) => worker_message(&reply, tr),
            WriteOutcome::NoReply => tr.tr(n_!("no reply from the controller")).into_owned(),
            WriteOutcome::Mismatch(stored) => {
                setting_write::mismatch_text(write.pending.line(), &stored, tr)
            }
        };
        self.device_type = write.previous;
        self.refuse(&reason, tr);
    }

    fn refuse(&mut self, reason: &str, tr: &Translator) {
        let title = tr.tr(n_!("Could not set laser mode:"));
        self.notice = Some((format!("{title} {reason}"), true));
    }

    /// Selects `wanted`, writing `$32` for CNC and Laser.
    fn choose_type(&mut self, wanted: DeviceType, machine: &Machine, tr: &Translator) {
        let previous = self.device_type;
        let Some(value) = wanted.laser_mode_value() else {
            self.device_type = wanted;
            return;
        };
        if let Some(blocked) = setting_write::blocked(machine) {
            let reason = setting_write::blocked_text(&blocked, tr);
            self.refuse(&reason, tr);
            return;
        }
        match PendingWrite::send(machine, &format!("$32={value}"), true, tr) {
            Ok(pending) => {
                self.device_type = wanted;
                self.notice = None;
                self.write = Some(TypeWrite { pending, previous });
            }
            Err(reason) => self.refuse(&reason, tr),
        }
    }

    /// Asks the controller for its build info and settings again.
    fn refresh(&mut self, machine: &Machine, tr: &Translator) {
        let sent = match setting_write::blocked(machine) {
            Some(blocked) => Err(setting_write::blocked_text(&blocked, tr)),
            None => setting_write::send_lines(machine, &["$I", "$$"], tr),
        };
        if let Err(reason) = sent {
            let title = tr.tr(n_!("Refresh device info"));
            self.notice = Some((format!("{title}: {reason}"), true));
        }
    }

    /// GTK's copied text: name, firmware and version.
    fn copy_text(&self, tr: &Translator) -> String {
        let shown = |value: &Option<String>| value.clone().unwrap_or_else(|| "-".to_string());
        format!(
            "{} {}\n{} {}\n{} {}",
            tr.tr(n_!("Device:")),
            shown(&self.identity.name),
            tr.tr(n_!("Firmware:")),
            shown(&self.identity.firmware),
            tr.tr(n_!("Version:")),
            shown(&self.identity.version),
        )
    }

    /// Keeps a device type write going; the app calls this once every frame, whatever page
    /// is shown.
    ///
    /// # Arguments
    /// * `ctx` - The egui context.
    /// * `tr` - Translates the notices.
    pub fn update(&mut self, ctx: &egui::Context, tr: &Translator) {
        self.tick(tr);
        if let Some(write) = &self.write {
            ctx.request_repaint_after(write.pending.time_left());
        }
    }

    /// Draws the panel into the Device Config page's left column.
    ///
    /// # Arguments
    /// * `ui` - The column's `Ui`.
    /// * `machine` - The app's machine connection.
    /// * `tr` - Translates the labels.
    pub fn show(&mut self, ui: &mut egui::Ui, machine: &Machine, tr: &Translator) {
        egui::ScrollArea::vertical()
            .id_salt("gcodekit5-device-info")
            .auto_shrink([false, false])
            .show(ui, |ui| self.contents(ui, machine, tr));
    }

    fn contents(&mut self, ui: &mut egui::Ui, machine: &Machine, tr: &Translator) {
        let connected = machine.status().connected;
        crate::theme::section_heading(ui, &tr.tr(n_!("Device Type")));
        let choices = [
            (
                DeviceType::Cnc,
                n_!("CNC"),
                n_!("Spindle-based machining (laser mode off, $32=0)"),
            ),
            (
                DeviceType::Laser,
                n_!("Laser"),
                n_!("Laser cutter/engraver (laser mode on, $32=1)"),
            ),
            (
                DeviceType::Other,
                n_!("Other"),
                n_!("Other device type ($32 unchanged)"),
            ),
        ];
        let mut chosen = None;
        ui.add_enabled_ui(connected && self.write.is_none(), |ui| {
            for (choice, label, hint) in choices {
                let radio = ui
                    .radio(self.device_type == choice, tr.tr(label))
                    .on_hover_text(tr.tr(hint));
                if radio.clicked() && self.device_type != choice {
                    chosen = Some(choice);
                }
            }
        });
        if let Some(choice) = chosen {
            self.choose_type(choice, machine, tr);
        }
        match &self.notice {
            Some((text, true)) => {
                ui.colored_label(ui.visuals().error_fg_color, text);
            }
            Some((text, false)) => {
                ui.label(text);
            }
            None => {}
        }
        ui.separator();
        // GTK's header: the connection icon, "Device", and the refresh and copy buttons.
        ui.horizontal(|ui| {
            // The icon shows the state, as in GTK. Its tooltip and accessible name say it as
            // "Device: Not connected", not the bare state the page's status line names.
            let (icon, state) = if connected {
                (Icon::Transfer, n_!("Connected"))
            } else {
                (Icon::Disconnected, n_!("Not connected"))
            };
            let name = format!("{} {}", tr.tr(n_!("Device:")), tr.tr(state));
            let (rect, response) =
                ui.allocate_exact_size(egui::Vec2::splat(18.0), egui::Sense::hover());
            icon.themed(ui).paint_at(ui, rect);
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, &name));
            response.on_hover_text(&name);
            crate::theme::section_heading(ui, &tr.tr(n_!("Device")));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(connected, |ui| {
                    let copy = tr.tr(n_!("Copy device info"));
                    if icon_only_button(ui, Icon::Copy, &copy).clicked() {
                        ui.ctx().copy_text(self.copy_text(tr));
                    }
                    let refresh = tr.tr(n_!("Refresh device info"));
                    if icon_only_button(ui, Icon::Refresh, &refresh).clicked() {
                        self.refresh(machine, tr);
                    }
                });
            });
        });
        self.details(ui, connected, tr);
        egui::CollapsingHeader::new(tr.tr(n_!("Capabilities")))
            .id_salt("gcodekit5-device-info-capabilities")
            .show(ui, |ui| self.capabilities(ui, tr));
    }

    /// The name, firmware, version, build and options rows.
    fn details(&self, ui: &mut egui::Ui, connected: bool, tr: &Translator) {
        let id = &self.identity;
        let name = match (&id.name, connected) {
            (Some(name), true) => name.clone(),
            (None, true) => "-".to_string(),
            (_, false) => tr.tr(n_!("No device")).into_owned(),
        };
        let rows = [
            (n_!("Name"), Some(name)),
            (n_!("Firmware"), id.firmware.clone()),
            (n_!("Version"), id.version.clone()),
            (n_!("Build"), id.build.clone()),
            (n_!("Options"), id.options.clone()),
        ];
        egui::Grid::new("gcodekit5-device-info-details")
            .num_columns(2)
            .show(ui, |ui| {
                for (label, value) in rows {
                    ui.weak(tr.tr(label));
                    ui.label(value.unwrap_or_else(|| "-".to_string()));
                    ui.end_row();
                }
            });
    }

    /// GTK's capability rows: variable spindle, homing (`$22`) and laser mode (`$32`).
    fn capabilities(&self, ui: &mut egui::Ui, tr: &Translator) {
        let unknown = n_!("Unknown (settings not loaded)");
        let homing = match self.identity.homing {
            Some(true) => (true, n_!("Enabled ($22=1)")),
            Some(false) => (false, n_!("Disabled ($22=0)")),
            None => (false, unknown),
        };
        let laser = match self.identity.laser {
            Some(true) => (true, n_!("Enabled ($32=1)")),
            Some(false) => (false, n_!("Disabled ($32=0)")),
            None => (false, unknown),
        };
        let rows = [
            (n_!("Variable Spindle"), (true, n_!("PWM spindle control"))),
            (n_!("Homing Cycle"), homing),
            (n_!("Laser Mode"), laser),
        ];
        egui::Grid::new("gcodekit5-device-info-capabilities-grid")
            .num_columns(2)
            .show(ui, |ui| {
                for (name, (enabled, notes)) in rows {
                    let color = if enabled {
                        egui::Color32::from_rgb(0x2e, 0xa0, 0x43)
                    } else {
                        ui.visuals().error_fg_color
                    };
                    ui.colored_label(color, tr.tr(name));
                    ui.weak(tr.tr(notes));
                    ui.end_row();
                }
            });
    }
}

/// The English msgids this panel shows, for the translation test.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        n_!("Device Info"),
        n_!("Device"),
        n_!("Device Type"),
        n_!("CNC"),
        n_!("Laser"),
        n_!("Other"),
        n_!("Spindle-based machining (laser mode off, $32=0)"),
        n_!("Laser cutter/engraver (laser mode on, $32=1)"),
        n_!("Other device type ($32 unchanged)"),
        n_!("Connected"),
        n_!("Not connected"),
        n_!("Refresh device info"),
        n_!("Copy device info"),
        n_!("No device"),
        n_!("Name"),
        n_!("Firmware"),
        n_!("Version"),
        n_!("Build"),
        n_!("Options"),
        n_!("Capabilities"),
        n_!("Unknown (settings not loaded)"),
        n_!("Enabled ($22=1)"),
        n_!("Disabled ($22=0)"),
        n_!("Enabled ($32=1)"),
        n_!("Disabled ($32=0)"),
        n_!("Variable Spindle"),
        n_!("PWM spindle control"),
        n_!("Homing Cycle"),
        n_!("Laser Mode"),
        n_!("Sent"),
        n_!("no reply from the controller"),
        n_!("Could not set laser mode:"),
        n_!("Device:"),
        n_!("Firmware:"),
        n_!("Version:"),
    ];
    ids.extend(setting_write::msgids());
    ids
}
