//! The pane's side of the Probe Tools page: the page's input, the run on the machine (fed
//! the app's machine events whether or not the pane is drawn, see
//! [`CamToolsPane::probe_events`]), the last successful result for Apply to WCS, and the
//! text shown about them.
//!
//! Dependencies: `gcodekit5-app` (events, translations), `gcodekit5-core` (positions).

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::MachineEvent;
use gcodekit5_core::Position;

use super::probe_routines::run::{self, Outcome, ProbeRun, ProbeSuccess};
use super::probe_routines::{ProbeParams, ProbeView};
use super::{CamAction, CamToolsPane};
use crate::machine::Machine;

/// Whether `event` makes a probe result stale: an alarm, a controller reset (its banner or
/// the reset byte) or homing (`$H`), after which the machine coordinates may differ.
fn invalidates_result(event: &MachineEvent) -> bool {
    match event {
        MachineEvent::Error(message) => message.to_ascii_lowercase().starts_with("alarm:"),
        MachineEvent::Received(line) => {
            let lower = line.trim().to_ascii_lowercase();
            lower.starts_with("grbl ") || lower.starts_with("grblhal ")
        }
        MachineEvent::Sent(line) => {
            let line = line.trim().to_ascii_uppercase();
            line.starts_with("$H") || line.starts_with("0X18")
        }
        MachineEvent::Disconnected
        | MachineEvent::StreamHalted { .. }
        | MachineEvent::ProtocolMismatch { .. } => true,
        MachineEvent::Connected
        | MachineEvent::StatusReport(_)
        | MachineEvent::StreamProgress { .. }
        | MachineEvent::StreamFinished { .. }
        | MachineEvent::StreamStopped { .. } => false,
    }
}

/// The Probe Tools page's state.
#[derive(Debug, Default)]
pub(super) struct ProbeState {
    /// The page's input.
    pub(super) params: ProbeParams,
    /// The run on the machine, from Probe until it succeeds, is restored or aborts.
    run: Option<ProbeRun>,
    /// The last successful run, for Apply to WCS.
    last: Option<ProbeSuccess>,
    /// The text about the last run.
    result: Option<String>,
    /// The machine position (mm) written into a generated program's header.
    pub(super) position: Position,
}

impl CamToolsPane {
    /// The Probe Tools page's input.
    pub fn probe_mut(&mut self) -> &mut ProbeParams {
        &mut self.probe.params
    }

    /// Whether a probe run is in progress (streaming, or failed and waiting to restore G90).
    pub fn probe_running(&self) -> bool {
        self.probe.run.is_some()
    }

    /// Whether a probe routine is streaming.
    pub fn probe_running_stream(&self) -> bool {
        self.probe.run.as_ref().is_some_and(ProbeRun::streaming)
    }

    /// The text about the last run, if any.
    pub fn probe_result_text(&self) -> Option<&str> {
        self.probe.result.as_deref()
    }

    /// The last successful run's result (machine coordinates, mm).
    pub fn probe_last_result(&self) -> Option<Position> {
        self.probe.last.as_ref().map(|s| s.position)
    }

    /// How long until the running probe's backstop is due (zero once it is), if one is armed
    /// (see `ProbeRun::backstop_in`).
    pub fn probe_backstop_in(&self) -> Option<std::time::Duration> {
        self.probe.run.as_ref().and_then(ProbeRun::backstop_in)
    }

    /// Takes the machine's events into the run (the app hands every event to the pane,
    /// drawn or not).
    ///
    /// # Arguments
    /// * `events` - Machine events, oldest first.
    /// * `tr` - Translator for the current UI language.
    pub fn probe_events(&mut self, events: &[MachineEvent], tr: &Translator) {
        if self.probe.last.is_some() && events.iter().any(invalidates_result) {
            // The machine's coordinates may have changed: the result no longer applies.
            self.probe.last = None;
        }
        let Some(run) = self.probe.run.as_mut() else {
            return;
        };
        for outcome in run.handle(events, tr) {
            match outcome {
                Outcome::Succeeded(success) => {
                    self.probe.result = Some(run::describe(&success, tr));
                    self.probe.last = Some(*success);
                    self.probe.run = None;
                }
                Outcome::Failed(text) => self.probe.result = Some(text),
                Outcome::Restored => self.probe.run = None,
                Outcome::Aborted => {
                    self.probe.result = Some(run::aborted_text(tr));
                    self.probe.run = None;
                }
            }
        }
    }

    /// What the page shows of the machine and the runs.
    pub(super) fn probe_view(&self, machine: Option<&Machine>, tr: &Translator) -> ProbeView {
        let status = machine.map(Machine::status);
        let report = status.and_then(|s| s.report.as_ref());
        ProbeView {
            connected: status.is_some_and(|s| s.connected),
            state: report.and_then(|r| r.machine_state.clone()),
            probe_pin: report.and_then(|r| r.probe_pin),
            not_ready: run::not_ready(machine, self.probe_running(), tr),
            streaming: self.probe_running_stream(),
            can_apply: self.probe.last.is_some(),
            result: self.probe.result.clone(),
        }
    }

    /// Keeps the position a generated program records.
    pub(super) fn track_probe_position(&mut self, machine: Option<&Machine>) {
        self.probe.position = machine
            .and_then(|m| m.status().machine_position())
            .map(|[x, y, z]| Position::new(x, y, z))
            .unwrap_or_default();
    }

    /// Runs the page's machine actions: Probe, Apply to WCS and Stop Probe.
    ///
    /// # Returns
    /// Whether `action` was one of them.
    pub(super) fn probe_action(
        &mut self,
        action: CamAction,
        machine: Option<&Machine>,
        tr: &Translator,
    ) -> bool {
        match action {
            CamAction::Probe => {
                if self.probe_running() {
                    return true;
                }
                match ProbeRun::start(machine, &self.probe.params, tr) {
                    Ok((run, text)) => {
                        self.probe.run = Some(run);
                        self.probe.last = None;
                        self.probe.result = Some(text);
                        self.notice = None;
                    }
                    Err(reason) => self.notice = Some(reason),
                }
            }
            CamAction::ApplyWcs => {
                let applied =
                    run::apply(machine, self.probe.last.as_ref(), self.probe.params.wcs, tr);
                if let Err(reason) = applied {
                    self.notice = Some(reason);
                }
            }
            CamAction::StopProbe => {
                if let Some(run) = self.probe.run.as_mut() {
                    run.stop();
                }
            }
            CamAction::None
            | CamAction::Generate
            | CamAction::SendToEditor
            | CamAction::Preview
            | CamAction::LoadPreset
            | CamAction::SavePreset
            | CamAction::SendToDesigner
            | CamAction::ChooseFolder
            | CamAction::ChooseImage => return false,
        }
        true
    }
}
