//! A probe routine run on the machine, as GTK's Probe button did it, without blocking the UI.
//!
//! - **Guards** ([`not_ready`]): GTK's (connected, no run in progress, no job, Idle), and
//!   two more: the probe input must not already be triggered, and the status report must
//!   be fresh (read after every command line sent), as jogging requires.
//! - **Start** ([`ProbeRun::start`]): the routine's program, without comments, is streamed
//!   through the shared `MachineController` (it returns at once; the worker thread does the
//!   I/O).
//! - **Progress** ([`ProbeRun::handle`]): the run reads the machine events in order: each
//!   `[PRB:...]` report in mm (following `$13` through the `$$` lines, as the DRO does), the
//!   first `ALARM:` / `error:`, and the stream's end. An `error:` reply makes the run reset
//!   the controller at once (beyond GTK, which streamed on): the routine's remaining lines
//!   are relative moves that must not run after a refused probe line. An alarm, the user's
//!   Stop, feed hold then reset, or E-STOP end the stream in the controller, so no further
//!   line of the routine is sent.
//! - **After a failure** the controller was left in incremental mode (G91): once it is
//!   Idle again (the user cleared the alarm) and nothing streams, `G90` is sent, as GTK did.
//!   A disconnect ends the run without it.
//!
//! Dependencies: `gcodekit5-app` (controller, events, program preparation),
//! `gcodekit5-camtools` (result computation, WCS command), `gcodekit5-core` (probe types).

use std::sync::{Arc, Mutex};

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::grbl_stream::{MAX_LINE_LENGTH, prepare_program};
use gcodekit5_app::machine::{MachineController, MachineEvent};
use gcodekit5_camtools::probe_routines::{ProbeOptions, ProbeRoutineEngine};
use gcodekit5_camtools::wcs_service::{WcsUpdateConfig, WcsUpdateService};
use gcodekit5_core::{Position, ProbeAxis, ProbeReport, ProbeRoutine};

use super::ProbeParams;
use crate::machine::{Machine, lock, parse_report_inches, probe_report};

const NOT_CONNECTED: &str = n_!("The machine is not connected.");
const RUN_IN_PROGRESS: &str = n_!("A probe run is still in progress.");
const JOB_RUNNING: &str = n_!("A job is running.");
const MUST_BE_IDLE: &str = n_!("The machine must be Idle");
const PROBE_TRIGGERED: &str = n_!("The probe input is already triggered.");
const REPORT_STALE: &str = n_!("Waiting for the machine's next status report.");
const NOT_STARTED: &str = n_!("The probe program was not started.");
const PROBING: &str = n_!("Probing");
const PROBE_FAILED: &str = n_!("Probe failed");
const CONTROLLER_REPORTED: &str = n_!("Controller reported");
const STOPPED: &str = n_!("The probe program was stopped.");
const RESTORE_HINT: &str =
    n_!("Clear any alarm ($X); absolute mode (G90) is restored once the machine is Idle.");
const ABORTED: &str = n_!("Probe aborted: the machine was disconnected.");
const RUN_FIRST: &str = n_!("Run a probe first.");
const SURFACE_AT: &str = n_!("Surface at machine");
const EDGE_AT: &str = n_!("Edge at machine");
const CORNER_AT: &str = n_!("Corner at machine");
const CENTRE_AT: &str = n_!("Centre at machine");
const MEASURED_DIAMETER: &str = n_!("Measured diameter:");
const TOOL_LENGTH_OFFSET: &str = n_!("Tool length offset:");
const REPORTS_RECEIVED: &str = n_!("probe report(s) received.");

/// The msgids a run shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        NOT_CONNECTED,
        RUN_IN_PROGRESS,
        JOB_RUNNING,
        MUST_BE_IDLE,
        PROBE_TRIGGERED,
        REPORT_STALE,
        NOT_STARTED,
        PROBING,
        PROBE_FAILED,
        CONTROLLER_REPORTED,
        STOPPED,
        RESTORE_HINT,
        ABORTED,
        RUN_FIRST,
        SURFACE_AT,
        EDGE_AT,
        CORNER_AT,
        CENTRE_AT,
        MEASURED_DIAMETER,
        TOOL_LENGTH_OFFSET,
        REPORTS_RECEIVED,
    ]
}

/// The lines a run streams for `gcode`: comments and empty lines removed, each line short
/// enough for the controller (the app's stream preparation).
///
/// # Errors
/// The first line that is too long.
pub fn program_lines(gcode: &str) -> Result<Vec<String>, String> {
    Ok(prepare_program(gcode, false, MAX_LINE_LENGTH)?
        .lines()
        .map(str::to_string)
        .collect())
}

/// Why the machine cannot take a probe run or a WCS command now, translated, if it cannot.
///
/// # Arguments
/// * `machine` - The app's machine connection.
/// * `running` - Whether a probe run is in progress.
/// * `tr` - Translator for the current UI language.
pub fn not_ready(machine: Option<&Machine>, running: bool, tr: &Translator) -> Option<String> {
    let Some(machine) = machine.filter(|m| m.status().connected) else {
        return Some(tr.tr(NOT_CONNECTED).into_owned());
    };
    if running {
        return Some(tr.tr(RUN_IN_PROGRESS).into_owned());
    }
    if machine.job_running() {
        return Some(tr.tr(JOB_RUNNING).into_owned());
    }
    let report = machine.status().report.as_ref();
    let state = report.and_then(|r| r.machine_state.as_deref());
    if !state.is_some_and(|s| s.starts_with("Idle")) {
        return Some(format!(
            "{} ({})",
            tr.tr(MUST_BE_IDLE),
            state.unwrap_or("?")
        ));
    }
    if report.and_then(|r| r.probe_pin) == Some(true) {
        return Some(tr.tr(PROBE_TRIGGERED).into_owned());
    }
    if !machine.jog_allowed() {
        return Some(tr.tr(REPORT_STALE).into_owned());
    }
    None
}

/// A successful run: its reports, options and computed result (machine coordinates, mm).
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeSuccess {
    /// The routine and its probe reports.
    pub report: ProbeReport,
    /// The options it ran with.
    pub options: ProbeOptions,
    /// The computed result.
    pub position: Position,
}

/// What a batch of events did to a run.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// The run succeeded; it is over.
    Succeeded(Box<ProbeSuccess>),
    /// The run failed (the text says why); it now waits to restore G90.
    Failed(String),
    /// G90 was restored after a failure; the run is over.
    Restored,
    /// The machine disconnected; the run is over.
    Aborted,
}

/// Where a run is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// The routine streams.
    Streaming,
    /// It failed; G90 waits for an Idle machine.
    Restoring,
}

/// A probe routine running on the machine.
pub struct ProbeRun {
    controller: Arc<Mutex<MachineController>>,
    routine: ProbeRoutine,
    options: ProbeOptions,
    report_inches: bool,
    reports: Vec<gcodekit5_core::ProbeResult>,
    failure: Option<String>,
    reset_sent: bool,
    phase: Phase,
}

impl std::fmt::Debug for ProbeRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProbeRun")
            .field("routine", &self.routine)
            .field("reports", &self.reports.len())
            .field("failure", &self.failure)
            .field("phase", &self.phase)
            .finish_non_exhaustive()
    }
}

impl ProbeRun {
    /// Starts `params`' routine on the machine; returns the run and the text shown while
    /// it probes.
    ///
    /// # Arguments
    /// * `machine` - The app's machine connection.
    /// * `params` - The page's input.
    /// * `tr` - Translator for the current UI language.
    ///
    /// # Errors
    /// Why it was not started: a guard ([`not_ready`]), the input, or the controller.
    pub fn start(
        machine: Option<&Machine>,
        params: &ProbeParams,
        tr: &Translator,
    ) -> Result<(Self, String), String> {
        if let Some(reason) = not_ready(machine, false, tr) {
            return Err(reason);
        }
        let Some(machine) = machine else {
            return Err(tr.tr(NOT_CONNECTED).into_owned());
        };
        let (routine, options) = params
            .parameters()
            .map_err(|_| tr.tr(n_!("Invalid Parameters")).into_owned())?;
        let start = machine
            .status()
            .machine_position()
            .map(|[x, y, z]| Position::new(x, y, z))
            .unwrap_or_default();
        let output = params.generate(start).map_err(|e| e.message(tr))?;
        let lines: Arc<[String]> = program_lines(&output.gcode)?.into();
        lock(machine.controller())
            .start_stream(lines)
            .map_err(|e| format!("{}: {e}", tr.tr(NOT_STARTED)))?;
        tracing::info!(routine = %output.description, "probe run started");
        let run = Self {
            controller: machine.controller().clone(),
            routine,
            options,
            report_inches: machine.status().report_inches,
            reports: Vec::new(),
            failure: None,
            reset_sent: false,
            phase: Phase::Streaming,
        };
        Ok((run, format!("{}: {}", tr.tr(PROBING), output.description)))
    }

    /// Whether the routine is still streaming (not yet failed and waiting for G90).
    pub fn streaming(&self) -> bool {
        self.phase == Phase::Streaming
    }

    /// Stops the routine: soft-resets the controller, which drops its remaining lines.
    pub fn stop(&mut self) {
        if self.phase == Phase::Streaming {
            lock(&self.controller).stop();
            self.reset_sent = true;
        }
    }

    /// Reads `events` (oldest first) into the run.
    ///
    /// # Arguments
    /// * `events` - Machine events since the last call.
    /// * `tr` - Translator for the current UI language.
    ///
    /// # Returns
    /// What happened, in order; after `Succeeded`, `Restored` or `Aborted` the run is over.
    pub fn handle(&mut self, events: &[MachineEvent], tr: &Translator) -> Vec<Outcome> {
        let mut outcomes = Vec::new();
        for event in events {
            if matches!(event, MachineEvent::Disconnected) {
                outcomes.push(Outcome::Aborted);
                return outcomes;
            }
            let outcome = match self.phase {
                Phase::Streaming => self.while_streaming(event, tr),
                Phase::Restoring => self.while_restoring(event),
            };
            if let Some(outcome) = outcome {
                let over = matches!(outcome, Outcome::Succeeded(_) | Outcome::Restored);
                outcomes.push(outcome);
                if over {
                    return outcomes;
                }
            }
        }
        outcomes
    }

    fn while_streaming(&mut self, event: &MachineEvent, tr: &Translator) -> Option<Outcome> {
        match event {
            MachineEvent::Received(line) => {
                if let Some(inches) = parse_report_inches(line) {
                    self.report_inches = inches;
                }
                if let Some(report) = probe_report(line, self.report_inches) {
                    self.reports.push(report);
                }
                None
            }
            MachineEvent::Error(message) => {
                let lower = message.to_ascii_lowercase();
                let failed = lower.starts_with("alarm:") || lower.starts_with("error:");
                if failed && self.failure.is_none() {
                    self.failure = Some(message.clone());
                }
                if lower.starts_with("error:") && !self.reset_sent {
                    tracing::warn!(%message, "probe line refused; stopping the routine");
                    self.stop();
                }
                None
            }
            MachineEvent::StreamHalted { reason } => {
                // The controller halted the stream itself; the stop that follows says so.
                self.failure = Some(reason.clone());
                None
            }
            MachineEvent::StreamFinished { .. } => Some(self.finish(tr)),
            MachineEvent::StreamStopped { .. } => {
                let reason = self
                    .failure
                    .clone()
                    .map(|line| format!("{}: {line}", tr.tr(CONTROLLER_REPORTED)))
                    .unwrap_or_else(|| tr.tr(STOPPED).into_owned());
                Some(self.fail(reason, tr))
            }
            MachineEvent::Connected
            | MachineEvent::Disconnected
            | MachineEvent::StatusReport(_)
            | MachineEvent::StreamProgress { .. }
            | MachineEvent::Sent(_) => None,
        }
    }

    fn while_restoring(&mut self, event: &MachineEvent) -> Option<Outcome> {
        let MachineEvent::StatusReport(report) = event else {
            return None;
        };
        let idle = report
            .machine_state
            .as_deref()
            .is_some_and(|s| s.starts_with("Idle"));
        let mut controller = lock(&self.controller);
        if !idle || controller.is_streaming() {
            return None;
        }
        match controller.send_line("G90") {
            Ok(()) => {
                tracing::warn!("probe run failed; restored G90");
                Some(Outcome::Restored)
            }
            Err(error) => {
                tracing::warn!(%error, "G90 not restored yet");
                None
            }
        }
    }

    /// The stream finished: the result, or why there is none.
    fn finish(&mut self, tr: &Translator) -> Outcome {
        if let Some(line) = self.failure.clone() {
            return self.fail(format!("{}: {line}", tr.tr(CONTROLLER_REPORTED)), tr);
        }
        let mut report = ProbeReport::new(self.routine.clone());
        for r in &self.reports {
            report.add_trigger(*r);
        }
        match ProbeRoutineEngine::compute_result_with(&report, &self.options) {
            Ok(position) => Outcome::Succeeded(Box::new(ProbeSuccess {
                report,
                options: self.options,
                position,
            })),
            Err(e) => self.fail(e.to_string(), tr),
        }
    }

    fn fail(&mut self, reason: String, tr: &Translator) -> Outcome {
        self.phase = Phase::Restoring;
        Outcome::Failed(format!(
            "{}: {reason}\n{}",
            tr.tr(PROBE_FAILED),
            tr.tr(RESTORE_HINT)
        ))
    }
}

/// The text shown for a successful run (GTK's wording).
///
/// # Arguments
/// * `success` - The run's result.
/// * `tr` - Translator for the current UI language.
pub fn describe(success: &ProbeSuccess, tr: &Translator) -> String {
    let pos = success.position;
    let mut text = match &success.report.routine {
        ProbeRoutine::ZTouch { .. } => format!("{} Z{:.3}", tr.tr(SURFACE_AT), pos.z),
        ProbeRoutine::EdgeFind { axis, .. } => {
            let v = match axis {
                ProbeAxis::X => pos.x,
                ProbeAxis::Y => pos.y,
                ProbeAxis::Z => pos.z,
            };
            format!("{} {axis:?}{v:.3}", tr.tr(EDGE_AT))
        }
        ProbeRoutine::CornerFind { .. } => {
            format!("{} X{:.3} Y{:.3}", tr.tr(CORNER_AT), pos.x, pos.y)
        }
        ProbeRoutine::BoreCenter { .. } | ProbeRoutine::BossCenter { .. } => {
            let mut t = format!("{} X{:.3} Y{:.3}", tr.tr(CENTRE_AT), pos.x, pos.y);
            if let Ok(d) = ProbeRoutineEngine::compute_diameter(&success.report, &success.options) {
                t.push_str(&format!("\n{} {d:.3} mm", tr.tr(MEASURED_DIAMETER)));
            }
            t
        }
        ProbeRoutine::ToolLength { .. } => {
            format!("{} {:.3} mm", tr.tr(TOOL_LENGTH_OFFSET), pos.z)
        }
    };
    text.push_str(&format!(
        "\n{} {}",
        success.report.triggers.len(),
        tr.tr(REPORTS_RECEIVED)
    ));
    text
}

/// The text shown when the machine disconnected during a run.
///
/// # Arguments
/// * `tr` - Translator for the current UI language.
pub fn aborted_text(tr: &Translator) -> String {
    tr.tr(ABORTED).into_owned()
}

/// Sends the WCS (or tool-length) command for `success` into `wcs` (54 to 59): `G10 L2`
/// with the probed machine coordinates, only the axes the routine measured (GTK's Apply to
/// WCS, through `WcsUpdateService`).
///
/// # Arguments
/// * `machine` - The app's machine connection.
/// * `success` - The last successful run.
/// * `wcs` - The target work coordinate system.
/// * `tr` - Translator for the current UI language.
///
/// # Returns
/// The command sent.
///
/// # Errors
/// Why nothing was sent: a guard, no successful run, or the service's or controller's
/// refusal.
pub fn apply(
    machine: Option<&Machine>,
    success: Option<&ProbeSuccess>,
    wcs: u8,
    tr: &Translator,
) -> Result<String, String> {
    let Some(success) = success else {
        return Err(tr.tr(RUN_FIRST).into_owned());
    };
    if let Some(reason) = not_ready(machine, false, tr) {
        return Err(reason);
    }
    let Some(machine) = machine else {
        return Err(tr.tr(NOT_CONNECTED).into_owned());
    };
    let mut service = WcsUpdateService::new(WcsUpdateConfig {
        target_wcs: wcs,
        probe_options: success.options,
        ..WcsUpdateConfig::default()
    });
    let result = service
        .process_probe_report(&success.report)
        .map_err(|e| e.to_string())?;
    lock(machine.controller())
        .send_line(&result.command)
        .map_err(|e| e.to_string())?;
    tracing::info!(command = %result.command, "applied probe result");
    Ok(result.command)
}
