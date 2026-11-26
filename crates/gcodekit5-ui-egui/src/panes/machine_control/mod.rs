//! The Machine Control pane: connection, emergency stop, homing, the position readout (DRO),
//! work zero (per-axis Zero, the G54-G59 selector, Zero All Axes, Go to Work Zero), job
//! control with stream progress, jogging, feed/rapid/spindle overrides and a one-line command
//! entry.
//!
//! Every work zero control is disabled while disconnected and refused, with the reason shown,
//! while a job runs ([`Machine::job_running`], which also covers a probe stream's halt). Go to
//! Work Zero moves the machine, so it also needs the jog rule's fresh report, in `Idle`.
//! Zero All Axes is confirmed in a dialog the app shows ([`Pane::take_confirm`]).
//!
//! The pane acts on the shared [`MachineController`] directly; every controller call returns
//! without I/O, so the lock is held for one call at a time and never across a frame. What it
//! shows comes from the app's [`Machine`] (this frame's events and the
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
mod wcs;

use std::sync::{Arc, Mutex};

use gcodekit5_app::device_status::{jog_direction, set_jog_inverted};
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::{
    Axis, MachineController, MachineError, MachineEvent, RealtimeCommand,
};
use gcodekit5_app::work_zero::{
    Wcs, go_to_work_zero_command, reports_grbl_parser_state, zero_axes_command,
};
use gcodekit5_core::units::{FeedRateUnits, MeasurementSystem};
use gcodekit5_settings::ConnectionSettings;
use gcodekit5_settings::config::Config;

pub(crate) use dro::format_length;

use crate::commands::Command;
use crate::dialogs::{ConfirmChoice, ConfirmRequest};
use crate::machine::{Machine, MachineStatus, lock};
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
        wcs::MSGIDS,
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
    n_!("Emergency stop (soft reset, {shortcut})"),
    n_!("Home"),
    n_!("Home machine ($H)"),
    n_!("Unlock"),
    n_!("Unlock from ALARM ($X)"),
    COMMAND_DURING_JOB,
    WCS_CHANGE_DURING_JOB,
    GO_TO_ZERO_NOT_IDLE,
    ZERO_NOT_IDLE,
    WCS_CHANGED,
];

/// A work zero command refused while a job runs.
const COMMAND_DURING_JOB: &str = n_!("Command ignored: a job is running.");
/// A work coordinate system selection refused while a job runs.
const WCS_CHANGE_DURING_JOB: &str = n_!("Work coordinate change ignored: a job is running.");
/// Go to Work Zero refused without a fresh `Idle` report.
const GO_TO_ZERO_NOT_IDLE: &str = n_!("Go to Work Zero refused: the machine is not idle.");
/// Zero or Zero All Axes refused without a fresh `Idle` report.
const ZERO_NOT_IDLE: &str = n_!("Zero refused: the machine is not idle.");
/// Zero All Axes refused at Yes: the active system is no longer the one the question named.
const WCS_CHANGED: &str = n_!("Zero All refused: the work coordinate system changed.");

/// The Machine Control pane.
pub struct MachineControlPane {
    controller: Arc<Mutex<MachineController>>,
    connection: connection::ConnectionForm,
    jog: jog::JogPanel,
    direct: direct_sender::DirectSender,
    wcs: wcs::WcsPanel,
    /// A confirmation the app is to ask for (Zero All Axes).
    confirm: Option<ConfirmRequest>,
    /// The measurement system lengths are shown in.
    units: MeasurementSystem,
    /// The units feeds are shown in.
    feed_units: FeedRateUnits,
    /// A refusal of the pane's own (not the controller's), shown until the next action.
    notice: Option<String>,
}

/// The soft reset chord as the user's platform names it (see [`crate::commands`]).
pub(crate) const SOFT_RESET_CHORD: &str = if cfg!(target_os = "macos") {
    "Cmd+Shift+K"
} else {
    "Ctrl+Shift+K"
};

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
            wcs: wcs::WcsPanel::default(),
            confirm: None,
            units: MeasurementSystem::Metric,
            feed_units: FeedRateUnits::MmPerMin,
            notice: None,
        }
    }

    /// Takes the settings that change how the pane jogs and shows values: the measurement
    /// system and feed units, the X/Y/Z travel and the per-axis jog inversion (shared with
    /// the GTK front end through `device_status::set_jog_inverted`).
    ///
    /// # Arguments
    /// * `config` - The current settings.
    pub fn configure(&mut self, config: &Config) {
        let machine = &config.machine;
        for axis in ['X', 'Y', 'Z', 'A', 'B', 'C'] {
            let inverted = machine
                .axis_direction_invert
                .get(&axis.to_string())
                .copied()
                .unwrap_or(false);
            // Written only on a change, so panes with the default settings never touch the
            // process-wide table.
            if (jog_direction(axis) < 0.0) != inverted {
                set_jog_inverted(axis, inverted);
            }
        }
        self.units = config.ui.measurement_system;
        self.feed_units = config.ui.feed_rate_units;
        let travel = [machine.x_limit, machine.y_limit, machine.z_limit].map(|v| v as f32);
        self.jog.configure(self.units, self.feed_units, travel);
    }

    /// Ends a hold-to-jog in progress with jog cancel (0x85).
    fn cancel_jog_hold(&mut self) {
        if self.jog.take_hold() {
            self.with_controller(|c| c.realtime(RealtimeCommand::JogCancel));
        }
    }

    /// The emergency stop, shared by the pane's button, the soft reset command and the status
    /// bar's button (which queues [`Command::SoftReset`]): forgets a jog hold, then `stop`
    /// (0x18, queued lines dropped, stream ended).
    fn emergency_stop(&mut self) {
        self.jog.forget_hold();
        self.with_controller(MachineController::stop);
    }

    /// Sends a command line; a refusal is shown in the pane.
    fn send_line(&mut self, line: &str) {
        let result: Result<(), MachineError> = self.with_controller(|c| c.send_line(line));
        match result {
            Ok(()) => self.notice = None,
            Err(error) => self.notice = Some(error.to_string()),
        }
    }

    /// Refuses with `msgid` as the notice while a job runs; returns whether it refused.
    fn refuse_during_job(
        &mut self,
        machine: Option<&Machine>,
        tr: &Translator,
        msgid: &str,
    ) -> bool {
        let running = machine.is_some_and(Machine::job_running);
        if running {
            self.refuse(tr, msgid);
        }
        running
    }

    /// The active work coordinate system a work zero command acts on, or the msgid of why
    /// it is refused: a job runs, the system is not known (or outside G54-G59, or the
    /// controller is not GRBL-family), or the machine is not `Idle` with a fresh report.
    /// GRBL refuses G-code in Alarm (`error:9`), parks `G10` behind a Hold and takes the
    /// offset from a moving machine's target, so only `Idle` is accepted.
    fn work_zero_target(
        &self,
        machine: Option<&Machine>,
        not_idle: &'static str,
    ) -> Result<Wcs, &'static str> {
        let Some(machine) = machine else {
            return Err(wcs::WAITING_FOR_WCS);
        };
        if machine.job_running() {
            return Err(COMMAND_DURING_JOB);
        }
        let status = machine.status();
        if let Some(reason) = self.wcs.blocked(status) {
            return Err(reason);
        }
        let idle = machine.jog_allowed()
            && status
                .report
                .as_ref()
                .and_then(|r| r.machine_state.as_deref())
                .is_some_and(|state| state.starts_with("Idle"));
        if !idle {
            return Err(not_idle);
        }
        self.wcs.active(status).ok_or(wcs::WAITING_FOR_WCS)
    }

    /// Shows the refusal `msgid` in the pane.
    fn refuse(&mut self, tr: &Translator, msgid: &str) {
        tracing::info!(reason = msgid, "work zero command refused");
        self.notice = Some(tr.tr(msgid).into_owned());
    }

    /// Makes the current position 0 on `axes` in the active work coordinate system
    /// (`G10 L20 P<n> X0 ...`), unless [`Self::work_zero_target`] refuses, or the system is
    /// no longer `expected` (the one a confirmation named).
    fn zero_axes(
        &mut self,
        machine: Option<&Machine>,
        tr: &Translator,
        axes: &[Axis],
        expected: Option<Wcs>,
    ) {
        match self.work_zero_target(machine, ZERO_NOT_IDLE) {
            Ok(wcs) if expected.is_some_and(|e| e != wcs) => self.refuse(tr, WCS_CHANGED),
            Ok(wcs) => self.send_line(&zero_axes_command(wcs, axes)),
            Err(msgid) => self.refuse(tr, msgid),
        }
    }

    /// Asks the app to confirm Zero All Axes in the active system, unless it would be
    /// refused now.
    fn ask_zero_all(&mut self, machine: Option<&Machine>, tr: &Translator) {
        match self.work_zero_target(machine, ZERO_NOT_IDLE) {
            Ok(wcs) => {
                self.notice = None;
                self.confirm = Some(ConfirmRequest::ZeroAllAxes(wcs));
            }
            Err(msgid) => self.refuse(tr, msgid),
        }
    }

    /// Rapids to work zero, only to a known active system, with a fresh `Idle` report (the
    /// jog rule) and no job.
    fn go_to_work_zero(&mut self, machine: Option<&Machine>, tr: &Translator, include_z: bool) {
        match self.work_zero_target(machine, GO_TO_ZERO_NOT_IDLE) {
            Ok(_) => self.send_line(go_to_work_zero_command(include_z)),
            Err(msgid) => self.refuse(tr, msgid),
        }
    }

    /// Selects a work coordinate system (`G55`), unless a job runs; the status follows the
    /// line once it is sent, and `$G` confirms it.
    fn select_wcs(&mut self, machine: Option<&Machine>, tr: &Translator, wcs: Wcs) {
        if self.refuse_during_job(machine, tr, WCS_CHANGE_DURING_JOB) {
            return;
        }
        self.send_line(wcs.code());
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
                let error = crate::machine::worker_message(&error.to_string(), tr);
                self.notice = Some(format!("{}: {error}", tr.tr("Error connecting")));
            }
        }
    }

    /// Starts streaming the loaded program, ending a hold-to-jog first.
    fn start_stream(&mut self, ctx: &PaneContext, tr: &Translator) {
        self.cancel_jog_hold();
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

    /// Jogs `axis` one step in the direction of `sign`, only while the machine is idle (or
    /// jogging).
    fn jog_step(&mut self, ctx: &PaneContext, axis: Axis, sign: f32) {
        if !ctx.machine().is_some_and(|m| m.jog_allowed()) {
            self.notice = Some(
                ctx.tr
                    .tr("Jog refused: the machine is not idle.")
                    .into_owned(),
            );
            return;
        }
        let (step, feed) = (self.jog.step_mm(), self.jog.feed_mm_min());
        let distance = jog::directed(axis, sign * step);
        self.with_controller(|c| c.jog(axis, distance, feed));
    }

    /// Runs a machine command; returns false for the commands that are not the pane's.
    fn run(&mut self, command: Command, ctx: &PaneContext) -> bool {
        let tr = ctx.tr;
        match command {
            Command::JogXPos => self.jog_step(ctx, Axis::X, 1.0),
            Command::JogXNeg => self.jog_step(ctx, Axis::X, -1.0),
            Command::JogYPos => self.jog_step(ctx, Axis::Y, 1.0),
            Command::JogYNeg => self.jog_step(ctx, Axis::Y, -1.0),
            Command::JogZPos => self.jog_step(ctx, Axis::Z, 1.0),
            Command::JogZNeg => self.jog_step(ctx, Axis::Z, -1.0),
            Command::Connect => self.connect(tr),
            Command::Disconnect => {
                // Disconnect writes jog cancel itself when a jog is outstanding.
                self.jog.forget_hold();
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
            Command::StreamStop | Command::SoftReset => self.emergency_stop(),
            Command::Home => self.send_line("$H"),
            Command::Unlock => self.send_line("$X"),
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
            | Command::OpenFirmwareSettings
            | Command::OpenDeviceInfo
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
        if events.contains(&MachineEvent::Disconnected) {
            self.jog.forget_hold();
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
        let response = ui.add_enabled(status.connected, estop).on_hover_text(
            tr.tr("Emergency stop (soft reset, {shortcut})")
                .replace("{shortcut}", SOFT_RESET_CHORD),
        );
        if response.clicked() {
            self.emergency_stop();
        }
        ui.horizontal(|ui| {
            let idle = status.connected && !status.streaming;
            if ui
                .add_enabled(idle, egui::Button::new(tr.tr("Home")))
                .on_hover_text(tr.tr("Home machine ($H)"))
                .clicked()
            {
                self.send_line("$H");
            }
            if ui
                .add_enabled(idle, egui::Button::new(tr.tr("Unlock")))
                .on_hover_text(tr.tr("Unlock from ALARM ($X)"))
                .clicked()
            {
                self.send_line("$X");
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
            // A drag off a held jog button must not scroll the pane under the pointer.
            .scroll_source(if self.jog.is_holding() {
                egui::scroll_area::ScrollSource {
                    drag: egui::scroll_area::DragScroll::Never,
                    ..egui::scroll_area::ScrollSource::default()
                }
            } else {
                egui::scroll_area::ScrollSource::default()
            })
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
                    // A controller refusal is kept in English; translated here.
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        crate::machine::worker_message(notice, tr),
                    );
                }
                if let Some(error) = &status.last_error {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        crate::machine::worker_message(error, tr),
                    );
                }
                ui.separator();
                self.show_safety(ui, tr, status);
                ui.separator();
                // Asks `$G` when the active system may have changed; zeroing waits for it.
                if let Some(machine) = machine {
                    self.wcs.watch_stream(machine);
                    self.wcs.drive(machine);
                }
                let blocked = self.wcs.blocked(status);
                let reason = blocked.map(|msgid| tr.tr(msgid));
                let zero = dro::ZeroButtons {
                    tr,
                    enabled: status.connected && blocked.is_none(),
                    disabled_reason: reason.as_deref().filter(|_| status.connected),
                };
                if let Some(axis) = dro::show(ui, tr, status, self.units, self.feed_units, &zero) {
                    self.zero_axes(machine, tr, &[axis], None);
                }
                ui.separator();
                match stream::show(ui, tr, status, has_program) {
                    Some(stream::Action::Start) => ctx.request(Command::StreamStart),
                    Some(stream::Action::Pause) => lock(&controller).pause(),
                    Some(stream::Action::Resume) => lock(&controller).resume(),
                    Some(stream::Action::Stop) => lock(&controller).stop(),
                    None => {}
                }
                ui.separator();
                // Read now, after this frame's Home/Unlock buttons: a line they sent blocks it.
                let jog_allowed = machine.is_some_and(|m| m.jog_allowed());
                for action in self.jog.show(ui, tr, status, jog_allowed) {
                    let mut controller = lock(&controller);
                    if !matches!(action, jog::Action::Refused(_)) {
                        self.notice = None;
                    }
                    match action {
                        jog::Action::Move {
                            axis,
                            distance_mm,
                            feed_mm_min,
                        } => {
                            controller.jog(axis, distance_mm, feed_mm_min);
                        }
                        jog::Action::Cancel => controller.realtime(RealtimeCommand::JogCancel),
                        jog::Action::Refused(msgid) => {
                            self.notice = Some(tr.tr(msgid).into_owned());
                        }
                    }
                }
                ui.separator();
                if let Some(command) = overrides::show(ui, tr, status) {
                    lock(&controller).realtime(command);
                }
                ui.separator();
                // Below the long-standing sections, so their places in the pane stay put.
                match self.wcs.show(ui, tr, status) {
                    Some(wcs::Action::Select(wcs)) => self.select_wcs(machine, tr, wcs),
                    Some(wcs::Action::ZeroAll) => self.ask_zero_all(machine, tr),
                    Some(wcs::Action::GoToZero { include_z }) => {
                        self.go_to_work_zero(machine, tr, include_z);
                    }
                    None => {}
                }
                ui.separator();
                if let Some(line) = self.direct.show(ui, tr, status) {
                    self.send_line(&line);
                }
            });
    }

    fn command(&mut self, command: Command, ctx: &mut PaneContext) -> bool {
        self.run(command, ctx)
    }

    fn cancel_hold(&mut self) {
        self.cancel_jog_hold();
    }

    fn configure(&mut self, config: &Config) {
        MachineControlPane::configure(self, config);
    }

    fn machine_events(&mut self, events: &[MachineEvent], _tr: &Translator) {
        self.wcs.follow(events);
    }

    fn set_controller(&mut self, controller: Option<&gcodekit5_devicedb::ControllerType>) {
        self.wcs.set_grbl(reports_grbl_parser_state(controller));
    }

    fn take_confirm(&mut self) -> Option<ConfirmRequest> {
        self.confirm.take()
    }

    fn confirmed(
        &mut self,
        request: ConfirmRequest,
        choice: ConfirmChoice,
        ctx: &mut PaneContext,
    ) -> bool {
        let ConfirmRequest::ZeroAllAxes(asked) = request else {
            return false;
        };
        if choice == ConfirmChoice::Yes {
            // Checked again: the job, the state or the active system may have changed.
            let axes = [Axis::X, Axis::Y, Axis::Z];
            self.zero_axes(ctx.machine(), ctx.tr, &axes, Some(asked));
        }
        true
    }
}
