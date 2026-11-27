//! Sending settings commands from the Firmware Settings page and the Device Info panel: when they
//! may be sent ([`blocked`]), and a `$n=v` write followed until the controller answers it
//! ([`PendingWrite`]).
//!
//! Every line goes through the shared `MachineController` (`send_line` queues it for the
//! controller's worker thread, which owns the transport), so the UI never waits on I/O. The
//! controller reports no event for a manual line's `ok`, so a GRBL write is confirmed by
//! reading the settings back: `$n=v` is followed by `$$`. The controller answers lines in
//! order, and a write starts only once every earlier line is answered ([`blocked`]), so
//! after `$n=v` is written the first reply seen is its own: an `error:N` refuses it, and
//! the first `$`-line of the `$$` dump means its `ok` came first. The write is accepted only
//! once the dump's own `$n=` line holds the value written: GRBL 1.1 stores some settings in a
//! byte (`$3=257` is kept as `1`), so a different read-back is a [`WriteOutcome::Mismatch`].
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
    /// The controller took the line but its read-back holds another value (GRBL stores a
    /// byte setting truncated); holds the read-back line (`$3=1`).
    Mismatch(String),
}

/// A setting write waiting for the controller's answer.
#[derive(Debug, Clone)]
pub struct PendingWrite {
    line: String,
    confirm: bool,
    written: bool,
    /// The write's `ok` came (a `$`-line of the read-back arrived); errors after it are
    /// not the write's.
    acknowledged: bool,
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
            acknowledged: false,
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
            MachineEvent::Error(message)
                if self.written && !self.acknowledged && message.starts_with("error:") =>
            {
                Some(WriteOutcome::Rejected(message.clone()))
            }
            MachineEvent::Received(line) if self.written && line.trim_start().starts_with('$') => {
                self.acknowledged = true;
                self.read_back(line.trim())
            }
            MachineEvent::Disconnected => Some(WriteOutcome::NoReply),
            MachineEvent::Connected
            | MachineEvent::Reconnecting { .. }
            | MachineEvent::StatusReport(_)
            | MachineEvent::Received(_)
            | MachineEvent::StreamProgress { .. }
            | MachineEvent::StreamHalted { .. }
            | MachineEvent::ProtocolMismatch { .. }
            | MachineEvent::StreamFinished { .. }
            | MachineEvent::StreamStopped { .. }
            | MachineEvent::Sent(_)
            | MachineEvent::Error(_) => None,
        }
    }

    /// The answer a read-back `$n=v` line gives: none for another setting, else whether it
    /// holds the value written.
    fn read_back(&self, line: &str) -> Option<WriteOutcome> {
        let Some((number, written)) = split_setting(&self.line) else {
            // Not a `$n=v` write: its `ok` is all there is to learn.
            return Some(WriteOutcome::Accepted);
        };
        let (reported, stored) = split_setting(line)?;
        if reported != number {
            return None;
        }
        Some(if same_value(written, stored) {
            WriteOutcome::Accepted
        } else {
            WriteOutcome::Mismatch(format!("${number}={stored}"))
        })
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

/// Splits a `$n=v` line into the setting number and the trimmed value, without an old
/// GRBL's trailing comment (`$3=1 (dir port invert mask)` gives `1`); a text value with
/// spaces (`$70=my shop cnc`) is kept whole.
fn split_setting(line: &str) -> Option<(&str, &str)> {
    let (number, value) = line.strip_prefix('$')?.split_once('=')?;
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let value = value.trim();
    let value = match value.rfind('(') {
        Some(open) if value.ends_with(')') => value[..open].trim_end(),
        Some(_) | None => value,
    };
    Some((number, value))
}

/// Whether a read-back `stored` value is the `written` one. Numbers are compared to the
/// precision the controller prints: within half a unit of the last decimal it shows
/// (`0.0104` is stored as `0.010`), and exactly when it shows no decimals (an integer
/// setting dropped `25.5` to `25`). Anything else (a grblHAL host name) is compared as text.
fn same_value(written: &str, stored: &str) -> bool {
    match (written.parse::<f64>(), stored.parse::<f64>()) {
        (Ok(w), Ok(s)) => {
            let decimals = stored.split_once('.').map_or(0, |(_, f)| f.len());
            let decimals = i32::try_from(decimals).unwrap_or(i32::MAX);
            let tolerance = if decimals == 0 {
                0.0
            } else {
                0.5 * 10f64.powi(-decimals)
            };
            (w - s).abs() <= tolerance + 1e-9
        }
        (Ok(_), Err(_)) | (Err(_), Ok(_)) | (Err(_), Err(_)) => written == stored,
    }
}

/// The translated failure for a write whose read-back holds another value.
///
/// # Arguments
/// * `written` - The line written, `$3=257`.
/// * `stored` - The read-back line, `$3=1`.
/// * `tr` - Translator for the current UI language.
pub fn mismatch_text(written: &str, stored: &str, tr: &Translator) -> String {
    format!(
        "{} {} {written}, {} {stored}",
        tr.tr(n_!("the controller stored a different value:")),
        tr.tr(n_!("written")),
        tr.tr(n_!("stored")),
    )
}

/// The English msgids this module shows, for the translation test.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("not connected"),
        n_!("a job is running"),
        n_!("waiting for the controller to answer earlier commands"),
        n_!("the machine must be idle"),
        n_!("the controller stored a different value:"),
        n_!("written"),
        n_!("stored"),
    ]
}
