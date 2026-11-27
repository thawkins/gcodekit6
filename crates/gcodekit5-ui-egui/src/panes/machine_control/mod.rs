//! The Machine Control pane: connection, emergency stop, homing, the position readout (DRO,
//! with A/B/C in degrees when reported), machine limits and the buffer readout, work zero
//! (per-axis Zero, the G54-G59 selector, Zero All Axes, Go to Work Zero), quick Z and edge
//! probes, job control with stream progress, jogging (pad and keyboard), feed/rapid/spindle
//! overrides with Set %, and a one-line command entry.
//!
//! Laid out as GTK's page: a sidebar (status strip, connection, machine state with homing,
//! the G54-G59 selector, job control, the command entry and notices) and a centre column
//! (overrides, the work readout with Zero All Axes and Go to Work Zero beside it, the
//! machine position, then the jog row and pad with the E-STOP picture button). The page puts
//! the editor and console to the right ([`crate::workspace`]).
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
mod quick_probe;
mod stream;
mod wcs;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::grbl_stream::{GrblResponseLine, classify_response};
use gcodekit5_app::machine::{
    Axis, MachineController, MachineError, MachineEvent, RealtimeCommand, realtime_label,
};
use gcodekit5_app::override_target::{OverrideTarget, TargetStep};
use gcodekit5_app::work_zero::{
    Wcs, WorkZeroSupport, go_to_work_zero_command, work_zero_mode, work_zero_support,
    zero_axes_command, zero_rotary_command,
};
use gcodekit5_communication::firmware::ControllerProtocol;
use gcodekit5_core::units::{FeedRateUnits, MeasurementSystem};
use gcodekit5_settings::ConnectionSettings;
use gcodekit5_settings::config::Config;

pub(crate) use dro::format_length;
pub(crate) use jog::feed_to_display;

use crate::commands::Command;
use crate::dialogs::{ConfirmChoice, ConfirmRequest};
use crate::icons::Icon;
use crate::machine::{Machine, MachineStatus, lock, protocol_mismatch_text};
use crate::panes::cam_tools::probe_routines::run::{self as probe_run, Outcome, ProbeRun};
use crate::workspace::{Pane, PaneContext, PaneId};

/// The msgids the pane shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    [
        connection::MSGIDS,
        dro::MSGIDS,
        jog::MSGIDS,
        overrides::MSGIDS,
        quick_probe::MSGIDS,
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
    ESTOP_REPETIER,
    OVERRIDES_NOT_ON_REPETIER,
    n_!("Home"),
    n_!("Home machine ($H)"),
    n_!("Unlock"),
    n_!("Unlock from ALARM ($X)"),
    n_!("Connect to enable controls."),
    ALARM_UNLOCK,
    UNLOCK_NOT_IN_ALARM,
    COMMAND_DURING_JOB,
    WCS_CHANGE_DURING_JOB,
    GO_TO_ZERO_NOT_IDLE,
    ZERO_NOT_IDLE,
    WCS_CHANGED,
    OVERRIDE_NOT_CONFIRMED,
    CALIBRATION_NOT_A_NUMBER,
];

/// The emergency stop's tooltip on a Repetier connection, which has no soft reset.
const ESTOP_REPETIER: &str = n_!("Emergency stop (M112, {shortcut})");

/// Why the overrides are disabled on a Repetier connection.
const OVERRIDES_NOT_ON_REPETIER: &str =
    n_!("Overrides need GRBL's real-time commands; Repetier firmware has none.");

/// Unlock from the menu or its key refused outside ALARM.
const UNLOCK_NOT_IN_ALARM: &str = n_!("Unlock ignored: the machine is not in ALARM.");

/// GTK's hint while the machine is in ALARM.
const ALARM_UNLOCK: &str = n_!("ALARM: Unlock required.");

/// Whether the latest status report says ALARM (GTK enables Unlock only then).
fn in_alarm(status: &MachineStatus) -> bool {
    status
        .report
        .as_ref()
        .and_then(|r| r.machine_state.as_deref())
        .is_some_and(|state| state.starts_with("Alarm"))
}

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
/// A rotary Zero refused: Preferences hold a calibration that is not a number.
const CALIBRATION_NOT_A_NUMBER: &str =
    n_!("Rotary calibration is not a number: check Preferences > Machine.");
/// A Set % target ended without the controller confirming a step.
const OVERRIDE_NOT_CONFIRMED: &str = n_!("The controller did not confirm the override change.");

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
    /// The configured travel the Machine Limits section falls back to.
    limits: dro::ConfiguredLimits,
    /// The rotary calibration of A, B and C in degrees (Preferences > Machine): the value a
    /// rotary Zero gives the current position.
    rotary_calibration: [f64; 3],
    /// A refusal of the pane's own (not the controller's), shown until the next action.
    notice: Option<String>,
    /// A refusal found outside drawing (no translator there), shown on the next frame.
    pending_refusal: Option<&'static str>,
    /// The Set % fields.
    targets: overrides::Targets,
    /// The override being moved to a Set % target, one confirmed step at a time.
    override_target: Option<OverrideTarget>,
    /// The quick probe on the machine, until it succeeds, is restored or aborts.
    probe: Option<ProbeRun>,
    /// The last quick probe's progress or result.
    probe_text: Option<String>,
    /// The emergency stop picture (`images/eStop2.png`): `None` until first drawn, then the
    /// texture, or `None` inside when it could not be decoded (the button shows its text).
    estop_image: Option<Option<egui::TextureHandle>>,
    /// The active device profile's protocol, which a connect speaks
    /// ([`MachineController::connect_with`]).
    protocol: ControllerProtocol,
    /// How work zero works on the active device profile's controller: the section's mode
    /// while offline.
    profile_support: WorkZeroSupport,
    /// How work zero works on the profile the last connect was started with: the mode of a
    /// connection whose protocol does not say ([`ControllerProtocol::Unknown`]).
    connect_support: WorkZeroSupport,
    /// Whether the last frame saw the machine connected; a profile change applies to the
    /// work zero section only while offline (see [`work_zero_mode`]).
    online: bool,
}

/// Share of the pane's width the sidebar (connection, state, work coordinates, job) takes.
const SIDEBAR_SHARE: f32 = 0.29;
/// Narrowest the sidebar gets while the centre column has room, so its two-button rows keep
/// their labels.
const SIDEBAR_MIN: f32 = 250.0;
/// Narrowest the sidebar gets in a window too narrow for [`SIDEBAR_MIN`] beside the centre
/// column.
const SIDEBAR_FLOOR: f32 = 140.0;
/// Width the override rows, and the readout with its zero actions, need side by side; a
/// narrower column stacks them.
const WIDE_ROWS: f32 = 450.0;
/// Width of the centre column with every row side by side and the emergency stop beside the
/// pad, with the panel's margins and scroll bar.
const CONTROLS_COMFORT: f32 = WIDE_ROWS + 50.0;
/// Width the page gives this pane before the editor column may grow over it: the sidebar
/// and the centre column with the emergency stop beside the pad.
pub(crate) const COMFORT_WIDTH: f32 = SIDEBAR_MIN + CONTROLS_COMFORT;
/// Width of the centre column's content, centred in the column as GTK centres it.
const CONTROLS_WIDTH: f32 = 560.0;
/// Size of the emergency stop button (GTK: 112 x 80 plus its picture).
const ESTOP_SIZE: egui::Vec2 = egui::vec2(112.0, 96.0);
/// Edge of the emergency stop picture.
const ESTOP_IMAGE: f32 = 60.0;

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
            limits: [200.0, 200.0, 100.0, 360.0, 360.0, 360.0],
            rotary_calibration: [0.0; 3],
            notice: None,
            pending_refusal: None,
            targets: overrides::Targets::default(),
            override_target: None,
            probe: None,
            probe_text: None,
            estop_image: None,
            protocol: ControllerProtocol::Unknown,
            profile_support: WorkZeroSupport::default(),
            connect_support: WorkZeroSupport::default(),
            online: false,
        }
    }

    /// Takes the settings that change how the pane jogs and shows values: the measurement
    /// system and feed units, the X/Y/Z travel and the per-axis jog inversion.
    ///
    /// # Arguments
    /// * `config` - The current settings.
    pub fn configure(&mut self, config: &Config) {
        let machine = &config.machine;
        self.jog
            .set_inverted(["X", "Y", "Z", "A", "B", "C"].map(|axis| {
                machine
                    .axis_direction_invert
                    .get(axis)
                    .copied()
                    .unwrap_or(false)
            }));
        self.units = config.ui.measurement_system;
        self.feed_units = config.ui.feed_rate_units;
        let travel = [machine.x_limit, machine.y_limit, machine.z_limit].map(|v| v as f32);
        self.limits = [
            machine.x_limit,
            machine.y_limit,
            machine.z_limit,
            machine.a_limit,
            machine.b_limit,
            machine.c_limit,
        ]
        .map(|v| v as f32);
        self.rotary_calibration = ['A', 'B', 'C'].map(|axis| {
            machine
                .rotary_calibration
                .get(&axis.to_string())
                .copied()
                .unwrap_or(0.0)
        });
        self.jog.configure(self.units, self.feed_units, travel);
        self.jog.configure_rotary(
            machine.jog_increment_rotary as f32,
            [machine.a_limit, machine.b_limit, machine.c_limit].map(|v| v as f32),
        );
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

    /// Sets the rotary `axis` of the active work coordinate system to its calibration value
    /// (`G10 L20 P<n> A<calibration>`), under the same guards as a linear Zero
    /// ([`Self::work_zero_target`]).
    fn zero_rotary(&mut self, machine: Option<&Machine>, tr: &Translator, axis: Axis) {
        let index = match axis {
            Axis::A => 0,
            Axis::B => 1,
            Axis::C => 2,
            Axis::X | Axis::Y | Axis::Z => return self.zero_axes(machine, tr, &[axis], None),
        };
        let calibration = self.rotary_calibration[index];
        match self.work_zero_target(machine, ZERO_NOT_IDLE) {
            Ok(wcs) => match zero_rotary_command(wcs, axis, calibration) {
                Some(line) => self.send_line(&line),
                None => {
                    tracing::warn!(?axis, calibration, "rotary calibration not finite");
                    self.refuse(tr, CALIBRATION_NOT_A_NUMBER);
                }
            },
            Err(msgid) => self.refuse(tr, msgid),
        }
    }

    /// Asks the app to confirm Zero All Axes in the active system, unless it would be
    /// refused now.
    fn ask_zero_all(&mut self, machine: Option<&Machine>, tr: &Translator) {
        match self.work_zero_target(machine, ZERO_NOT_IDLE) {
            Ok(wcs) => {
                self.notice = None;
                let named = self.wcs.names_system().then_some(wcs);
                self.confirm = Some(ConfirmRequest::ZeroAllAxes(named));
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
        let inverted = self.jog.inverted();
        self.jog = jog::JogPanel::new(step_mm, feed_mm_min, travel_mm);
        self.jog.set_inverted(inverted);
        self
    }

    /// Runs `f` on the controller, holding its lock only for that call.
    fn with_controller<R>(&self, f: impl FnOnce(&mut MachineController) -> R) -> R {
        f(&mut lock(&self.controller))
    }

    /// Starts connecting with the form's settings; a refusal is shown in the pane.
    fn connect(&mut self, tr: &Translator) {
        let settings = self.connection.settings().clone();
        let protocol = self.protocol;
        match self.with_controller(|c| c.connect_with(&settings, protocol)) {
            Ok(()) => {
                self.notice = None;
                self.connect_support = self.profile_support;
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
        let distance = self.jog.directed(axis, sign * step);
        self.with_controller(|c| c.jog(axis, distance, feed));
    }

    /// Runs a jog command: one step, or, when a key ran it in continuous mode, a hold that
    /// lasts while the key is down (as a held jog button), under the same guards.
    fn jog_command(&mut self, ctx: &PaneContext, command: Command, axis: Axis, sign: f32) {
        let Some(key) = ctx.jog_key().filter(|_| self.jog.continuous()) else {
            self.jog_step(ctx, axis, sign);
            return;
        };
        let machine = ctx.machine();
        let status = machine.map(Machine::status);
        let allowed = status.is_some_and(|s| s.connected && !s.streaming)
            && machine.is_some_and(Machine::jog_allowed);
        if !allowed {
            self.notice = Some(
                ctx.tr
                    .tr("Jog refused: the machine is not idle.")
                    .into_owned(),
            );
            return;
        }
        let Some(status) = status else {
            return;
        };
        match self.jog.key_hold(command, key, status) {
            jog::KeyHold::Started(action) => {
                self.notice = None;
                self.jog_action(action, ctx.tr);
            }
            jog::KeyHold::Busy => {}
            jog::KeyHold::Refused(msgid) => self.notice = Some(ctx.tr.tr(msgid).into_owned()),
        }
    }

    /// Sends what a jog action asks for.
    fn jog_action(&mut self, action: jog::Action, tr: &Translator) {
        match action {
            jog::Action::Move {
                axis,
                distance_mm,
                feed_mm_min,
            } => self.with_controller(|c| c.jog(axis, distance_mm, feed_mm_min)),
            jog::Action::Cancel => {
                self.with_controller(|c| c.realtime(RealtimeCommand::JogCancel));
            }
            jog::Action::Refused(msgid) => self.notice = Some(tr.tr(msgid).into_owned()),
        }
    }

    /// Ends a keyboard hold unless this frame confirms it: its key is still down, no key
    /// event arrived (any press or release but the held key's own repeat: a release reported
    /// as another key, Shift turning keypad 8 into ArrowUp, a NumLock change), the modifiers
    /// are the ones held at the press, the machine still allows jogging, no widget took the
    /// keyboard (a text field focused, a dropdown open) and the pointer was not pressed
    /// (focus moving elsewhere). A page switch and window focus loss end it through
    /// [`Pane::cancel_hold`], and [`Pane::frame`] ends one no frame confirmed for
    /// [`jog::KEY_HOLD_DEAD_MAN`]. While it lasts a frame is asked for every
    /// [`jog::KEY_HOLD_FRAME`].
    fn follow_key_hold(&mut self, ui: &egui::Ui, machine: Option<&Machine>) {
        let Some(held) = self.jog.held_key() else {
            return;
        };
        let (key_down, pressed, modifiers, key_event) = ui.input(|i| {
            let key_event = i.raw.events.iter().any(|e| {
                matches!(e, egui::Event::Key { key, repeat, .. } if !(*repeat && *key == held.key))
            });
            (
                i.key_down(held.key),
                i.pointer.any_pressed(),
                i.modifiers,
                key_event,
            )
        });
        let widget_has_keys =
            ui.ctx().memory(|m| m.focused().is_some()) || egui::Popup::is_any_open(ui.ctx());
        let allowed = machine
            .is_some_and(|m| m.status().connected && !m.status().streaming && m.jog_allowed());
        let may_continue =
            allowed && !widget_has_keys && !pressed && !key_event && modifiers == held.modifiers;
        if self.jog.follow_key_hold(key_down, may_continue) {
            tracing::debug!(?held, key_down, may_continue, "keyboard jog ended");
            self.with_controller(|c| c.realtime(RealtimeCommand::JogCancel));
        } else {
            ui.ctx().request_repaint_after(jog::KEY_HOLD_FRAME);
        }
    }

    /// Runs a machine command; returns false for the commands that are not the pane's.
    fn run(&mut self, command: Command, ctx: &PaneContext) -> bool {
        let tr = ctx.tr;
        match command {
            Command::JogXPos => self.jog_command(ctx, command, Axis::X, 1.0),
            Command::JogXNeg => self.jog_command(ctx, command, Axis::X, -1.0),
            Command::JogYPos => self.jog_command(ctx, command, Axis::Y, 1.0),
            Command::JogYNeg => self.jog_command(ctx, command, Axis::Y, -1.0),
            Command::JogZPos => self.jog_command(ctx, command, Axis::Z, 1.0),
            Command::JogZNeg => self.jog_command(ctx, command, Axis::Z, -1.0),
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
            // GTK enabled Unlock only in ALARM; the menu item and its key follow the button.
            Command::Unlock => {
                if ctx.machine().is_some_and(|m| in_alarm(m.status())) {
                    self.send_line("$X");
                } else {
                    self.refuse(tr, UNLOCK_NOT_IN_ALARM);
                }
            }
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
            | Command::About
            | Command::AlignLeft
            | Command::AlignRight
            | Command::AlignTop
            | Command::AlignBottom
            | Command::AlignCenterX
            | Command::AlignCenterY
            | Command::Group
            | Command::Ungroup
            | Command::Duplicate
            | Command::FitDevice => return false,
        }
        true
    }

    /// Starts a quick probe through the Probe Routines run; a refusal is shown in the pane.
    fn start_quick_probe(
        &mut self,
        machine: Option<&Machine>,
        tr: &Translator,
        quick: quick_probe::QuickProbe,
    ) {
        if self.probe.is_some() {
            return;
        }
        match ProbeRun::start(machine, &quick.params(), tr) {
            Ok((run, text)) => {
                self.probe = Some(run);
                self.probe_text = Some(text);
                self.notice = None;
            }
            Err(reason) => {
                tracing::info!(%reason, ?quick, "quick probe refused");
                self.notice = Some(reason);
            }
        }
    }

    /// Takes machine events into the quick probe run, if one is on.
    fn follow_probe(&mut self, events: &[MachineEvent], tr: &Translator) {
        let Some(run) = self.probe.as_mut() else {
            return;
        };
        for outcome in run.handle(events, tr) {
            match outcome {
                Outcome::Succeeded(success) => {
                    self.probe_text = Some(probe_run::describe(&success, tr));
                    self.probe = None;
                }
                Outcome::Failed(text) => self.probe_text = Some(text),
                Outcome::Restored => self.probe = None,
                Outcome::Aborted => {
                    self.probe_text = Some(probe_run::aborted_text(tr));
                    self.probe = None;
                }
            }
        }
    }

    /// Takes the Set % target one step further: sends the next override byte once the
    /// controller reported the last one; ends it at the target, on a disconnect, or (with a
    /// notice) when a step is not confirmed.
    fn drive_override_target(&mut self, machine: &Machine) {
        let Some(target) = self.override_target.as_mut() else {
            return;
        };
        let status = machine.status();
        if !status.connected {
            self.override_target = None;
            return;
        }
        match target.step(status.overrides.as_ref(), Instant::now()) {
            TargetStep::Send(command) => self.with_controller(|c| c.realtime(command)),
            TargetStep::Wait => {}
            TargetStep::Done(_) => self.override_target = None,
            TargetStep::Failed => {
                let (kind, goal) = (target.kind(), target.goal());
                tracing::warn!(?kind, goal, "override target not confirmed");
                self.override_target = None;
                self.pending_refusal = Some(OVERRIDE_NOT_CONFIRMED);
            }
        }
    }

    /// Clears the connecting state once the controller reported how the connect went (a
    /// failed connect reports its error, then `Disconnected`).
    fn follow_events(&mut self, events: &[MachineEvent], tr: &Translator) {
        for event in events {
            if let MachineEvent::ProtocolMismatch {
                configured,
                detected,
            } = event
            {
                self.notice = Some(protocol_mismatch_text(*configured, *detected, tr));
            }
        }
        if events
            .iter()
            .any(|e| matches!(e, MachineEvent::Connected | MachineEvent::Disconnected))
        {
            self.connection.set_connecting(false);
        }
        if events.contains(&MachineEvent::Disconnected) {
            self.jog.forget_hold();
        }
        // A reset (E-STOP, Stop, or the controller's own) puts the overrides back to 100 %:
        // a Set % target must not then drive them toward its goal again.
        let reset = realtime_label(RealtimeCommand::SoftReset.byte());
        let was_reset = events.iter().any(|e| match e {
            MachineEvent::Sent(line) => Some(line.as_str()) == reset,
            MachineEvent::Received(line) => {
                matches!(classify_response(line), GrblResponseLine::Reset(_))
            }
            MachineEvent::Connected
            | MachineEvent::Disconnected
            | MachineEvent::StatusReport(_)
            | MachineEvent::StreamProgress { .. }
            | MachineEvent::StreamHalted { .. }
            | MachineEvent::ProtocolMismatch { .. }
            | MachineEvent::StreamFinished { .. }
            | MachineEvent::StreamStopped { .. }
            | MachineEvent::Error(_) => false,
        });
        if was_reset && self.override_target.take().is_some() {
            tracing::info!("Set % target dropped: the controller was reset");
        }
    }

    /// Draws the homing buttons, side by side and of equal width.
    fn show_homing(&mut self, ui: &mut egui::Ui, tr: &Translator, status: &MachineStatus) {
        let idle = status.connected && !status.streaming && !status.estopped;
        let width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
        ui.horizontal(|ui| {
            let home = egui::Button::image_and_text(Icon::Home.themed(ui), tr.tr("Home"))
                .min_size(egui::vec2(width, 0.0));
            if ui
                .add_enabled(idle, home)
                .on_hover_text(tr.tr("Home machine ($H)"))
                .clicked()
            {
                self.send_line("$H");
            }
            let unlock = egui::Button::image_and_text(Icon::Unlock.themed(ui), tr.tr("Unlock"))
                .min_size(egui::vec2(width, 0.0));
            // GTK: Unlock is only useful in ALARM.
            if ui
                .add_enabled(idle && in_alarm(status), unlock)
                .on_hover_text(tr.tr("Unlock from ALARM ($X)"))
                .clicked()
            {
                self.send_line("$X");
            }
        });
    }

    /// The emergency stop picture, decoded on first use.
    fn estop_texture(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        self.estop_image
            .get_or_insert_with(|| load_estop(ctx))
            .clone()
    }

    /// The left column: connection, machine state, work coordinates, job control, then the
    /// command entry and any notice. Returns the command to request (Send starts the
    /// stream through the app).
    fn show_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        tr: &Translator,
        machine: Option<&Machine>,
        status: &MachineStatus,
    ) -> Option<Command> {
        let has_program = machine.and_then(|m| m.program()).is_some();
        let mut request = None;
        self.connection.show_strip(ui, tr, status);
        ui.add_space(SECTION_GAP);
        match self.connection.show(ui, tr, status) {
            Some(connection::Action::Connect) => self.connect(tr),
            Some(connection::Action::Disconnect) => {
                self.with_controller(MachineController::disconnect);
                self.connection.set_connecting(false);
            }
            None => {}
        }
        ui.add_space(SECTION_GAP);
        crate::theme::section_heading(ui, &tr.tr("Machine State"));
        let connecting = self.connection.connecting();
        dro::show_state(ui, tr, status, connecting, self.feed_units);
        self.show_homing(ui, tr, status);
        ui.add_space(SECTION_GAP);
        match self.wcs.show_selector(ui, tr, status) {
            Some(wcs::Action::Select(wcs)) => self.select_wcs(machine, tr, wcs),
            Some(wcs::Action::ZeroAll | wcs::Action::GoToZero { .. }) | None => {}
        }
        ui.add_space(SECTION_GAP);
        dro::show_limits(ui, tr, status, self.units, &self.limits);
        ui.add_space(SECTION_GAP);
        match stream::show(ui, tr, status, has_program) {
            Some(stream::Action::Start) => request = Some(Command::StreamStart),
            Some(stream::Action::Pause) => self.with_controller(MachineController::pause),
            Some(stream::Action::Resume) => self.with_controller(MachineController::resume),
            Some(stream::Action::Stop) => self.with_controller(MachineController::stop),
            None => {}
        }
        ui.add_space(SECTION_GAP);
        if let Some(line) = self.direct.show(ui, tr, status) {
            self.send_line(&line);
        }
        if !status.connected {
            ui.weak(tr.tr("Connect to enable controls."));
        } else if status.estopped {
            ui.colored_label(ui.visuals().warn_fg_color, tr.tr(wcs::ESTOPPED_RESET));
        } else if in_alarm(status) {
            ui.colored_label(ui.visuals().warn_fg_color, tr.tr(ALARM_UNLOCK));
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
        request
    }

    /// The centre column: overrides, the work position with the zero actions beside it, the
    /// machine position, then jogging with the emergency stop beside the pad. A narrow
    /// column stacks the rows and puts the emergency stop at the top.
    fn show_controls(
        &mut self,
        ui: &mut egui::Ui,
        tr: &Translator,
        machine: Option<&Machine>,
        status: &MachineStatus,
    ) {
        // The visible width: rows wider than the column would run under the editor column.
        let room = ui
            .available_width()
            .min(ui.clip_rect().right() - ui.cursor().left());
        let stacked = room < WIDE_ROWS;
        // A column too narrow for E-STOP beside the jog pad shows it at the top instead, so
        // it is never pushed past the column's edge or below the fold.
        let estop_on_top = stacked || room < jog::PAD_ROW_WIDTH + ESTOP_SIZE.x;
        let texture = self.estop_texture(ui.ctx());
        // A Repetier connection has no real-time commands: no overrides, no jog cancel (so no
        // continuous jog), no probing, and the emergency stop is `M112`.
        let repetier = status.connected && self.with_controller(|c| c.is_repetier());
        let estop_label = tr.tr("E-STOP");
        let estop_msgid = if repetier {
            ESTOP_REPETIER
        } else {
            "Emergency stop (soft reset, {shortcut})"
        };
        let estop_hint = tr.tr(estop_msgid).replace("{shortcut}", SOFT_RESET_CHORD);
        let mut estop = false;
        if estop_on_top {
            estop = estop_button(ui, &estop_label, texture.as_ref(), status.connected)
                .on_hover_text(&estop_hint)
                .clicked();
            ui.add_space(SECTION_GAP);
        }
        self.targets.busy = self.override_target.is_some();
        let overrides = ui
            .add_enabled_ui(!repetier, |ui| {
                overrides::show(ui, tr, status, stacked, &mut self.targets)
            })
            .inner;
        if repetier {
            ui.weak(tr.tr(OVERRIDES_NOT_ON_REPETIER));
        }
        match overrides {
            Some(overrides::Action::Realtime(command)) => {
                self.with_controller(|c| c.realtime(command));
            }
            Some(overrides::Action::SetTarget(kind, percent)) => {
                self.notice = None;
                self.override_target = Some(OverrideTarget::new(kind, percent, Instant::now()));
            }
            None => {}
        }
        ui.add_space(SECTION_GAP);
        crate::theme::section_heading(ui, &dro::work_title(tr, self.units));
        let blocked = self.wcs.blocked(status);
        let reason = blocked.map(|msgid| tr.tr(msgid));
        let zero = dro::ZeroButtons {
            tr,
            enabled: status.connected && blocked.is_none(),
            disabled_reason: reason.as_deref().filter(|_| status.connected),
        };
        let units = self.units;
        let wcs = &mut self.wcs;
        // Quick probes: GTK's guards (connected, no job, one probe at a time); the run
        // checks the rest (Idle, fresh report, probe input) when it starts.
        let probe_enabled = status.connected
            && !repetier
            && self.probe.is_none()
            && !machine.is_some_and(Machine::job_running);
        let probe_refusal = repetier.then(|| tr.tr(probe_run::PROBE_NOT_ON_REPETIER));
        let probe_text = probe_refusal.as_deref().or(self.probe_text.as_deref());
        self.jog.begin_frame();
        // Rotary jog buttons in the readout rows: the pad's guards (read after this frame's
        // Home/Unlock buttons) and rules.
        let rotary_enabled =
            status.connected && !status.streaming && machine.is_some_and(|m| m.jog_allowed());
        let mut rotary_actions = Vec::new();
        let jog = &mut self.jog;
        let mut rotary_jog = |ui: &mut egui::Ui, axis: Axis| {
            jog.rotary_buttons(ui, tr, axis, rotary_enabled, &mut rotary_actions);
        };
        let (zeroed, wcs_action, quick) = if stacked {
            let zeroed = dro::show_work(ui, status, units, &zero, &mut rotary_jog);
            ui.add_space(6.0);
            let action = wcs.show_zero_actions(ui, tr, status);
            let quick = quick_probe::show(ui, tr, probe_enabled, probe_text);
            (zeroed, action, quick)
        } else {
            ui.horizontal(|ui| {
                let zeroed = ui
                    .vertical(|ui| dro::show_work(ui, status, units, &zero, &mut rotary_jog))
                    .inner;
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width().min(260.0));
                    // Compact, so the column (wrapped in a narrow window) is no taller
                    // than the readout beside it.
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.add_space(4.0);
                    let action = wcs.show_zero_actions(ui, tr, status);
                    let quick = quick_probe::show(ui, tr, probe_enabled, probe_text);
                    (zeroed, action, quick)
                })
                .inner
            })
            .inner
        };
        for action in rotary_actions {
            if !matches!(action, jog::Action::Refused(_)) {
                self.notice = None;
            }
            self.jog_action(action, tr);
        }
        if let Some(quick) = quick {
            self.start_quick_probe(machine, tr, quick);
        }
        if let Some(axis) = zeroed {
            self.zero_rotary(machine, tr, axis);
        }
        match wcs_action {
            Some(wcs::Action::ZeroAll) => self.ask_zero_all(machine, tr),
            Some(wcs::Action::GoToZero { include_z }) => {
                self.go_to_work_zero(machine, tr, include_z);
            }
            Some(wcs::Action::Select(_)) | None => {}
        }
        ui.add_space(SECTION_GAP);
        dro::show_machine(ui, tr, status, self.units);
        ui.add_space(SECTION_GAP);
        // Read now, after this frame's Home/Unlock buttons: a line they sent blocks it.
        let jog_allowed = machine.is_some_and(|m| m.jog_allowed());
        let actions = self.jog.show(ui, tr, status, jog_allowed, |ui| {
            if !estop_on_top {
                ui.vertical(|ui| {
                    ui.add_space(((3.0 * 56.0 + 10.0) - ESTOP_SIZE.y).max(0.0) / 2.0);
                    estop |= estop_button(ui, &estop_label, texture.as_ref(), status.connected)
                        .on_hover_text(&estop_hint)
                        .clicked();
                });
            }
        });
        if self.jog.end_frame() {
            self.with_controller(|c| c.realtime(RealtimeCommand::JogCancel));
        }
        if estop {
            self.emergency_stop();
        }
        for action in actions {
            if !matches!(action, jog::Action::Refused(_)) {
                self.notice = None;
            }
            self.jog_action(action, tr);
        }
    }
}

/// Vertical gap between the sections of a column.
const SECTION_GAP: f32 = 12.0;

/// Decodes the embedded emergency stop picture into a texture; `None` (logged) when it is
/// missing or does not decode.
fn load_estop(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let Some(bytes) = crate::resources::image("eStop2.png") else {
        tracing::warn!("eStop2.png not embedded");
        return None;
    };
    match image::load_from_memory(bytes) {
        Ok(decoded) => {
            let rgba = decoded.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let pixels = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            Some(ctx.load_texture(
                "machine-control-estop",
                pixels,
                egui::TextureOptions::LINEAR,
            ))
        }
        Err(error) => {
            tracing::warn!(%error, "eStop2.png could not be decoded");
            None
        }
    }
}

/// The emergency stop button: the GTK picture above "E-STOP", on a raised card. Its
/// accessible name is `label`.
fn estop_button(
    ui: &mut egui::Ui,
    label: &str,
    texture: Option<&egui::TextureHandle>,
    enabled: bool,
) -> egui::Response {
    ui.add_enabled(enabled, |ui: &mut egui::Ui| {
        let (rect, response) = ui.allocate_exact_size(ESTOP_SIZE, egui::Sense::click());
        let enabled = ui.is_enabled();
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&response);
            ui.painter().rect(
                rect,
                visuals.corner_radius,
                visuals.weak_bg_fill,
                visuals.bg_stroke,
                egui::StrokeKind::Inside,
            );
            let font = egui::FontId::proportional(18.0);
            let text_y = rect.bottom() - 18.0;
            if let Some(texture) = texture {
                let image = egui::Rect::from_center_size(
                    egui::pos2(rect.center().x, rect.top() + 6.0 + ESTOP_IMAGE / 2.0),
                    egui::vec2(ESTOP_IMAGE, ESTOP_IMAGE),
                );
                egui::Image::new((texture.id(), image.size())).paint_at(ui, image);
            }
            let colour = visuals.text_color();
            let at = egui::pos2(rect.center().x, text_y);
            ui.painter()
                .text(at, egui::Align2::CENTER_CENTER, label, font.clone(), colour);
            // Bold like GTK's title-4, painted twice as the theme's headings are.
            ui.painter().text(
                at + egui::vec2(0.6, 0.0),
                egui::Align2::CENTER_CENTER,
                label,
                font,
                colour,
            );
        }
        response
    })
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
        if let Some(msgid) = self.pending_refusal.take() {
            self.notice = Some(tr.tr(msgid).into_owned());
        }
        self.follow_key_hold(ui, ctx.machine());
        let default_status = MachineStatus::default();
        let machine = ctx.machine();
        let status = machine.map_or(&default_status, |m| m.status());
        // The sidebar never takes the centre column's room: E-STOP and the jog pad stay
        // visible however wide it is dragged, and a narrow window narrows it first.
        let room = ui.available_width() - CONTROLS_COMFORT;
        let min = SIDEBAR_MIN.min(room).max(SIDEBAR_FLOOR);
        let sidebar = (ui.available_width() * SIDEBAR_SHARE).max(SIDEBAR_MIN);
        let request = egui::Panel::left("gcodekit5-machine-sidebar")
            .resizable(true)
            .default_size(sidebar)
            .size_range(min..=room.max(min))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("machine-control-sidebar")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.show_sidebar(ui, tr, machine, status))
                    .inner
            })
            .inner;
        // A drag off a held jog button must not scroll the column under the pointer.
        let scroll_source = if self.jog.is_holding() {
            egui::scroll_area::ScrollSource {
                drag: egui::scroll_area::DragScroll::Never,
                ..egui::scroll_area::ScrollSource::default()
            }
        } else {
            egui::scroll_area::ScrollSource::default()
        };
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("machine-control-controls")
                .auto_shrink([false, false])
                .scroll_source(scroll_source)
                .show(ui, |ui| {
                    let width = CONTROLS_WIDTH.min(ui.available_width());
                    let margin = (ui.available_width() - width) / 2.0;
                    ui.horizontal(|ui| {
                        ui.add_space(margin);
                        ui.vertical(|ui| {
                            ui.set_width(width);
                            self.show_controls(ui, tr, machine, status);
                        });
                    });
                });
        });
        if let Some(command) = request {
            ctx.request(command);
        }
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

    fn machine_events(&mut self, events: &[MachineEvent], tr: &Translator) {
        // Here, not in `ui`: the pane is hidden on every page but Machine Control, and the
        // connection state, the work zero query and a quick probe must follow the machine
        // meanwhile.
        self.follow_events(events, tr);
        self.wcs.follow(events);
        self.follow_probe(events, tr);
    }

    fn wake_in(&self) -> Option<std::time::Duration> {
        self.probe.as_ref().and_then(ProbeRun::backstop_in)
    }

    fn frame(&mut self, machine: &Machine) {
        // Every frame, drawn or not: a keyboard jog may run while the pane is hidden.
        let connected = machine.status().connected;
        let protocol = self.with_controller(|c| c.protocol());
        self.online = connected;
        let support = if connected {
            work_zero_mode(protocol, self.connect_support)
        } else {
            self.profile_support
        };
        self.wcs.set_support(support, connected);
        let repetier = connected && protocol == ControllerProtocol::Repetier;
        if self.jog.set_continuous_allowed(!repetier) {
            self.with_controller(|c| c.realtime(RealtimeCommand::JogCancel));
        }
        if self.jog.expire_key_hold() {
            tracing::warn!("keyboard jog not confirmed in time; cancelled");
            self.with_controller(|c| c.realtime(RealtimeCommand::JogCancel));
        }
        // Asks `$G` when the active system may have changed; zeroing waits for it.
        self.wcs.watch_stream(machine);
        self.wcs.drive(machine);
        self.drive_override_target(machine);
    }

    fn set_controller(&mut self, controller: Option<&gcodekit5_devicedb::ControllerType>) {
        self.profile_support = work_zero_support(controller);
        if !self.online {
            self.wcs.set_support(self.profile_support, false);
        }
        self.protocol = controller.map_or(ControllerProtocol::Unknown, |c| {
            ControllerProtocol::from_name(c.as_str())
        });
    }

    fn set_device(&mut self, profile: Option<&gcodekit5_devicedb::DeviceProfile>) {
        // Read by the controller when a Repetier program streams, as GTK read the active
        // profile's pen mode at job start.
        let pen_mode = profile.is_some_and(|p| p.pen_mode);
        self.with_controller(|c| c.set_pen_mode(pen_mode));
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
            self.zero_axes(ctx.machine(), ctx.tr, &axes, asked);
        }
        true
    }
}
