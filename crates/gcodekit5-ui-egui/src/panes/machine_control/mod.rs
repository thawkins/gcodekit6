//! The Machine Control pane: connection, emergency stop, homing, the position readout (DRO),
//! job control with stream progress, jogging, feed/rapid/spindle overrides and a one-line
//! command entry.
//!
//! The pane acts on the shared [`MachineController`] directly; every controller call returns
//! without I/O, so the lock is held for one call at a time and never across a frame. What it
//! shows comes from the app's [`Machine`](crate::machine::Machine) (this frame's events and the
//! derived status) through [`PaneContext::machine`], so the pane never polls the controller.
//!
//! The machine commands of the catalog ([`Command::Connect`], [`Command::FeedHold`],
//! [`Command::JogXPos`], ...) are run here, whether they come from the pane's buttons, a key
//! or a menu ([`Pane::command`]).
//!
//! Ported from the GTK `ui/gtk/machine_control` view (UI parts) and `ui/gtk/osd_format.rs`.
//!
//! Dependencies: `egui`, `gcodekit5-app` (controller, translations),
//! `gcodekit5-settings` (connection settings).

mod connection;
mod direct_sender;
mod dro;
mod jog;
mod overrides;
mod stream;

use std::sync::{Arc, Mutex};

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::{Axis, MachineController, MachineEvent, RealtimeCommand};
use gcodekit5_settings::ConnectionSettings;

use crate::commands::Command;
use crate::machine::{MachineStatus, lock};
use crate::workspace::{Pane, PaneContext, PaneId};

/// The msgids the pane shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    [
        connection::MSGIDS,
        dro::MSGIDS,
        jog::MSGIDS,
        overrides::MSGIDS,
        stream::MSGIDS,
        direct_sender::MSGIDS,
        MSGIDS,
    ]
    .concat()
    .into_iter()
    .chain(
        Command::ALL
            .into_iter()
            .filter(|c| c.is_jog())
            .map(Command::label_msgid),
    )
    .collect()
}

const MSGIDS: &[&str] = &[
    n_!("E-STOP"),
    n_!("Emergency stop (soft reset Ctrl-X)"),
    n_!("Home"),
    n_!("Home machine ($H)"),
    n_!("Unlock"),
    n_!("Unlock from ALARM ($X)"),
];

/// The Machine Control pane.
pub struct MachineControlPane {
    controller: Arc<Mutex<MachineController>>,
    connection: connection::ConnectionForm,
    jog: jog::JogPanel,
    direct: direct_sender::DirectSender,
    /// A refusal of the pane's own (not the controller's), shown until the next action.
    notice: Option<String>,
}

impl std::fmt::Debug for MachineControlPane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MachineControlPane")
            .field("connection", &self.connection)
            .field("jog", &self.jog)
            .field("notice", &self.notice)
            .finish_non_exhaustive()
    }
}

impl MachineControlPane {
    /// A pane acting on `controller`, with default connection and jog settings.
    ///
    /// # Arguments
    /// * `controller` - The app's shared machine controller.
    pub fn new(controller: Arc<Mutex<MachineController>>) -> Self {
        Self {
            controller,
            connection: connection::ConnectionForm::new(ConnectionSettings::default()),
            jog: jog::JogPanel::default(),
            direct: direct_sender::DirectSender::default(),
            notice: None,
        }
    }

    /// The same pane connecting with `settings`.
    ///
    /// # Arguments
    /// * `settings` - Connection type, port or host, baud rate.
    pub fn with_connection(mut self, settings: ConnectionSettings) -> Self {
        self.connection = connection::ConnectionForm::new(settings);
        self
    }

    /// The same pane jogging by `step_mm` at `feed_mm_min`, and jogging continuously at most
    /// `travel_mm` (X, Y, Z travel) in one hold.
    ///
    /// # Arguments
    /// * `step_mm` - Jog step in mm.
    /// * `feed_mm_min` - Jog feed in mm/min.
    /// * `travel_mm` - Machine travel of X, Y and Z in mm.
    pub fn with_jog(mut self, step_mm: f32, feed_mm_min: f32, travel_mm: [f32; 3]) -> Self {
        self.jog = jog::JogPanel::new(step_mm, feed_mm_min, travel_mm);
        self
    }

    /// Runs `f` on the controller, holding its lock only for that call.
    fn with_controller<R>(&self, f: impl FnOnce(&mut MachineController) -> R) -> R {
        f(&mut lock(&self.controller))
    }

    /// Starts connecting with the form's settings; a refusal is shown in the pane.
    fn connect(&mut self, tr: &Translator) {
        let settings = self.connection.settings().clone();
        match self.with_controller(|c| c.connect(&settings)) {
            Ok(()) => {
                self.notice = None;
                self.connection.set_connecting(true);
            }
            Err(error) => {
                tracing::warn!(%error, "connect refused");
                self.notice = Some(format!("{}: {error}", tr.tr("Error connecting")));
            }
        }
    }

    /// Starts streaming the loaded program.
    fn start_stream(&mut self, ctx: &PaneContext, tr: &Translator) {
        let Some(program) = ctx.machine().and_then(|m| m.program()).cloned() else {
            self.notice = Some(tr.tr("No program loaded").into_owned());
            return;
        };
        match self.with_controller(|c| c.start_stream(program)) {
            Ok(()) => self.notice = None,
            Err(error) => {
                tracing::warn!(%error, "stream refused");
                self.notice = Some(error.to_string());
            }
        }
    }

    /// Jogs `axis` one step in the direction of `sign`.
    fn jog_step(&mut self, axis: Axis, sign: f32) {
        let (step, feed) = (self.jog.step_mm(), self.jog.feed_mm_min());
        self.with_controller(|c| c.jog(axis, sign * step, feed));
    }

    /// Runs a machine command; returns false for the commands that are not the pane's.
    fn run(&mut self, command: Command, ctx: &PaneContext) -> bool {
        let tr = ctx.tr;
        match command {
            Command::JogXPos => self.jog_step(Axis::X, 1.0),
            Command::JogXNeg => self.jog_step(Axis::X, -1.0),
            Command::JogYPos => self.jog_step(Axis::Y, 1.0),
            Command::JogYNeg => self.jog_step(Axis::Y, -1.0),
            Command::JogZPos => self.jog_step(Axis::Z, 1.0),
            Command::JogZNeg => self.jog_step(Axis::Z, -1.0),
            Command::Connect => self.connect(tr),
            Command::Disconnect => {
                self.with_controller(MachineController::disconnect);
                self.connection.set_connecting(false);
            }
            Command::StreamStart => self.start_stream(ctx, tr),
            // Feed hold also stops feeding program lines; cycle start feeds them again.
            Command::StreamPause | Command::FeedHold => {
                self.with_controller(MachineController::pause);
            }
            Command::CycleStart => self.with_controller(MachineController::resume),
            // Soft reset through `stop`: 0x18, and the queued lines are dropped with it.
            Command::StreamStop | Command::SoftReset => {
                self.jog.forget_hold();
                self.with_controller(MachineController::stop);
            }
            Command::Home => self.with_controller(|c| c.send_line("$H")),
            Command::Unlock => self.with_controller(|c| c.send_line("$X")),
            Command::FileNew
            | Command::FileNew3d
            | Command::FileOpen
            | Command::FileSave
            | Command::FileSaveAs
            | Command::FileImport
            | Command::FileImportImage
            | Command::FileExportGcode
            | Command::FileExportSvg
            | Command::FileRun
            | Command::Quit
            | Command::Undo
            | Command::Redo
            | Command::Cut
            | Command::Copy
            | Command::Paste
            | Command::ZoomFit
            | Command::ZoomIn
            | Command::ZoomOut
            | Command::ResetView
            | Command::ViewTop
            | Command::ViewFront
            | Command::ViewIso
            | Command::OpenSettings
            | Command::OpenDeviceManager
            | Command::OpenToolsManager
            | Command::OpenMaterialsManager
            | Command::OpenHelp
            | Command::About => return false,
        }
        true
    }

    /// Clears the connecting state once the controller reported how the connect went (a
    /// failed connect reports its error, then `Disconnected`).
    fn follow_events(&mut self, events: &[MachineEvent]) {
        if events
            .iter()
            .any(|e| matches!(e, MachineEvent::Connected | MachineEvent::Disconnected))
        {
            self.connection.set_connecting(false);
        }
    }

    /// Draws the emergency stop and the homing buttons.
    fn show_safety(&mut self, ui: &mut egui::Ui, tr: &Translator, status: &MachineStatus) {
        let estop = egui::Button::new(
            egui::RichText::new(tr.tr("E-STOP"))
                .strong()
                .color(egui::Color32::WHITE),
        )
        .fill(egui::Color32::from_rgb(0xc0, 0x1c, 0x28))
        .min_size(egui::vec2(ui.available_width(), 36.0));
        let response = ui
            .add_enabled(status.connected, estop)
            .on_hover_text(tr.tr("Emergency stop (soft reset Ctrl-X)"));
        if response.clicked() {
            self.jog.forget_hold();
            self.with_controller(MachineController::stop);
        }
        ui.horizontal(|ui| {
            let idle = status.connected && !status.streaming;
            if ui
                .add_enabled(idle, egui::Button::new(tr.tr("Home")))
                .on_hover_text(tr.tr("Home machine ($H)"))
                .clicked()
            {
                self.with_controller(|c| c.send_line("$H"));
            }
            if ui
                .add_enabled(idle, egui::Button::new(tr.tr("Unlock")))
                .on_hover_text(tr.tr("Unlock from ALARM ($X)"))
                .clicked()
            {
                self.with_controller(|c| c.send_line("$X"));
            }
        });
    }
}

impl Pane for MachineControlPane {
    fn id(&self) -> PaneId {
        PaneId::MachineControl
    }

    fn title(&self, tr: &Translator) -> String {
        tr.tr(PaneId::MachineControl.title_msgid()).into_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext) {
        let tr = ctx.tr;
        let default_status = MachineStatus::default();
        let machine = ctx.machine();
        let status = machine.map_or(&default_status, |m| m.status());
        if let Some(machine) = machine {
            self.follow_events(machine.events());
        }
        let has_program = machine.and_then(|m| m.program()).is_some();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let controller = Arc::clone(&self.controller);
                match self.connection.show(ui, tr, status) {
                    Some(connection::Action::Connect) => self.connect(tr),
                    Some(connection::Action::Disconnect) => {
                        lock(&controller).disconnect();
                        self.connection.set_connecting(false);
                    }
                    None => {}
                }
                if let Some(notice) = &self.notice {
                    ui.colored_label(ui.visuals().error_fg_color, notice);
                }
                if let Some(error) = &status.last_error {
                    ui.colored_label(ui.visuals().warn_fg_color, error);
                }
                ui.separator();
                self.show_safety(ui, tr, status);
                ui.separator();
                dro::show(ui, tr, status);
                ui.separator();
                match stream::show(ui, tr, status, has_program) {
                    Some(stream::Action::Start) => ctx.request(Command::StreamStart),
                    Some(stream::Action::Pause) => lock(&controller).pause(),
                    Some(stream::Action::Resume) => lock(&controller).resume(),
                    Some(stream::Action::Stop) => lock(&controller).stop(),
                    None => {}
                }
                ui.separator();
                for action in self.jog.show(ui, tr, status) {
                    let mut controller = lock(&controller);
                    match action {
                        jog::Action::Move {
                            axis,
                            distance_mm,
                            feed_mm_min,
                        } => {
                            controller.jog(axis, distance_mm, feed_mm_min);
                        }
                        jog::Action::Cancel => controller.realtime(RealtimeCommand::JogCancel),
                    }
                }
                ui.separator();
                if let Some(command) = overrides::show(ui, tr, status) {
                    lock(&controller).realtime(command);
                }
                ui.separator();
                if let Some(line) = self.direct.show(ui, tr, status) {
                    lock(&controller).send_line(&line);
                }
            });
    }

    fn command(&mut self, command: Command, ctx: &mut PaneContext) -> bool {
        self.run(command, ctx)
    }
}
