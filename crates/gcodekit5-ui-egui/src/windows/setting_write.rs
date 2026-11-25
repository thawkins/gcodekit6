//! Sending settings commands from the Firmware Settings and Device Info windows: when they
//! may be sent ([`blocked`]), and a `$n=v` write followed until the controller answers it
//! ([`PendingWrite`]).
//!
//! Every line goes through the shared `MachineController` (`send_line` queues it for the
//! controller's worker thread, which owns the transport), so the UI never waits on I/O. The
//! controller reports no event for a manual line's `ok`, so a GRBL write is confirmed by
//! reading the settings back: `$n=v` is followed by `$$`. The controller answers lines in
//! order, and a write starts only once every earlier line is answered ([`blocked`]), so
//! after `$n=v` is written the first reply seen is its own: an `error:N` refuses it, and
//! the first `$n=` line of the `$$` dump means its `ok` came first.
//!
//! Dependencies: `gcodekit5-app` (controller, settings guard, translations).

use std::time::{Duration, Instant};

use gcodekit5_app::firmware_settings::{SettingsBlocked, settings_blocked};
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::MachineEvent;

use crate::machine::{Machine, lock, worker_message};

/// How long a write waits for the controller's answer.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

/// Why a settings command must not be sent now, or `None` when it may be: connected, no
/// job streaming ([`Machine::job_running`]), every earlier command line answered
/// (`MachineController::lines_answered`, so a write's reply is its own) and the machine
/// Idle or in Alarm.
///
/// # Arguments
/// * `machine` - The app's machine connection.
pub fn blocked(machine: &Machine) -> Option<SettingsBlocked> {
    let status = machine.status();
    let state = status
        .report
        .as_ref()
        .and_then(|report| report.machine_state.as_deref());
    let answered = lock(machine.controller()).lines_answered();
    settings_blocked(status.connected, machine.job_running(), answered, state)
}

/// The translated reason a settings command was refused.
///
/// # Arguments
/// * `blocked` - Why it was refused.
/// * `tr` - Translator for the current UI language.
pub fn blocked_text(blocked: &SettingsBlocked, tr: &Translator) -> String {
    match blocked {
        SettingsBlocked::NotConnected => tr.tr(n_!("not connected")).into_owned(),
        SettingsBlocked::JobRunning => tr.tr(n_!("a job is running")).into_owned(),
        SettingsBlocked::Unanswered => tr
            .tr(n_!("waiting for the controller to answer earlier commands"))
            .into_owned(),
        SettingsBlocked::NotIdle(state) => {
            format!("{} ({state})", tr.tr(n_!("the machine must be idle")))
        }
    }
}

/// Sends `lines` in order, stopping at the first the controller refuses.
///
/// # Returns
/// The translated reason the first refused line was not sent.
pub fn send_lines(machine: &Machine, lines: &[&str], tr: &Translator) -> Result<(), String> {
    let mut controller = lock(machine.controller());
    for line in lines {
        controller
            .send_line(line)
            .map_err(|e| worker_message(&e.to_string(), tr))?;
    }
    Ok(())
}

/// How the controller answered a write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteOutcome {
    /// The controller took it.
    Accepted,
    /// The controller or its worker refused it; holds the reply (`error:3`).
    Rejected(String),
    /// No answer within [`WRITE_TIMEOUT`], or the connection closed.
    NoReply,
}

/// A setting write waiting for the controller's answer.
#[derive(Debug, Clone)]
pub struct PendingWrite {
    line: String,
    confirm: bool,
    written: bool,
    started: Instant,
}

impl PendingWrite {
    /// Sends `line`, then `$$` when `confirm` is set (a GRBL write).
    ///
    /// # Arguments
    /// * `machine` - The app's machine connection.
    /// * `line` - The write, `$110=5000`.
    /// * `confirm` - Whether to read the settings back to learn the answer; without it the
    ///   write counts as accepted once written (Repetier's `M206`, whose `ok` is not tracked).
    /// * `tr` - Translator for the refusal reason.
    ///
    /// # Errors
    /// The translated reason the controller refused to send the line.
    pub fn send(
        machine: &Machine,
        line: &str,
        confirm: bool,
        tr: &Translator,
    ) -> Result<Self, String> {
        let lines: &[&str] = if confirm { &[line, "$$"] } else { &[line] };
        send_lines(machine, lines, tr)?;
        Ok(Self {
            line: line.to_string(),
            confirm,
            written: false,
            started: Instant::now(),
        })
    }

    /// The line written.
    pub fn line(&self) -> &str {
        &self.line
    }

    /// The answer `event` gives, if it decides the write.
    ///
    /// # Arguments
    /// * `event` - The next machine event, in order.
    pub fn observe(&mut self, event: &MachineEvent) -> Option<WriteOutcome> {
        match event {
            MachineEvent::Sent(line) if !self.written && *line == self.line => {
                self.written = true;
                (!self.confirm).then_some(WriteOutcome::Accepted)
            }
            // The worker refused the line before writing it (too long for the buffer).
            MachineEvent::Error(message) if !self.written && message.ends_with(&self.line) => {
                Some(WriteOutcome::Rejected(message.clone()))
            }
            MachineEvent::Error(message) if self.written && message.starts_with("error:") => {
                Some(WriteOutcome::Rejected(message.clone()))
            }
            MachineEvent::Received(line) if self.written && line.trim_start().starts_with('$') => {
                Some(WriteOutcome::Accepted)
            }
            MachineEvent::Disconnected => Some(WriteOutcome::NoReply),
            MachineEvent::Connected
            | MachineEvent::StatusReport(_)
            | MachineEvent::Received(_)
            | MachineEvent::StreamProgress { .. }
            | MachineEvent::StreamHalted { .. }
            | MachineEvent::StreamFinished { .. }
            | MachineEvent::StreamStopped { .. }
            | MachineEvent::Sent(_)
            | MachineEvent::Error(_) => None,
        }
    }

    /// Whether the answer is overdue.
    pub fn timed_out(&self) -> bool {
        self.started.elapsed() > WRITE_TIMEOUT
    }

    /// How long until the answer is overdue.
    pub fn time_left(&self) -> Duration {
        WRITE_TIMEOUT.saturating_sub(self.started.elapsed())
    }
}

/// The English msgids this module shows, for the translation test.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("not connected"),
        n_!("a job is running"),
        n_!("waiting for the controller to answer earlier commands"),
        n_!("the machine must be idle"),
    ]
}
