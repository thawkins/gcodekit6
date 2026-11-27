//! The worker's Repetier protocol: numbered, checksummed lines, one in flight, `Resend:`
//! replay, pen mode and the `M112` emergency stop.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use gcodekit5_communication::firmware::grbl::status_parser::StatusParser;
use gcodekit5_communication::firmware::protocol_registry::detect_protocol;
use gcodekit5_communication::firmware::repetier::RepetierProcessor;
use gcodekit5_communication::firmware::repetier::processor::RepetierEvent;
use gcodekit5_communication::firmware::{ControllerProtocol, protocol_profile};

use super::MAX_REPETIER_RESENDS;
use super::types::{Command, MachineEvent, REPETIER_ESTOP_LABEL, RealtimeCommand, worker_text};
use super::worker::{RepetierLink, WorkerState, repetier_translates};

impl WorkerState {
    /// The same worker on a connection opened for `configured`: Repetier's flow control on a
    /// Repetier connection, and the identity check for a known protocol. `halted` is the
    /// controller's flag for a Repetier `M112`.
    pub(super) fn with_protocol(
        mut self,
        configured: ControllerProtocol,
        halted: Arc<AtomicBool>,
        pen_mode: Arc<AtomicBool>,
    ) -> Self {
        self.configured = configured;
        self.halted = halted;
        self.repetier =
            (configured == ControllerProtocol::Repetier).then(|| RepetierLink::new(pen_mode));
        self
    }

    /// Drops the jogs still waiting in the queue (after a jog cancel or a reset): they must
    /// not start afterwards, and are answered by never running.
    pub(super) fn drop_queued_jogs(&mut self) {
        let (jogs, kept): (Vec<_>, Vec<_>) = self
            .manual
            .drain(..)
            .partition(|(line, _)| line.starts_with("$J="));
        self.manual = kept.into();
        for (_, seq) in jogs {
            self.drop_line(seq);
        }
        self.jog_outstanding = false;
    }

    /// Checks a received line against the protocol the connection was opened for, until the
    /// controller names its firmware (GTK's `protocol_mismatch`). Another known firmware
    /// rejects the connection: the line and [`MachineEvent::ProtocolMismatch`] are reported
    /// and the worker closes the connection.
    ///
    /// # Returns
    /// Whether the connection was rejected.
    pub(super) fn check_identity(&mut self, line: &str) -> bool {
        if self.identity_checked || !protocol_profile(self.configured.as_str()).announces_identity {
            return false;
        }
        let Some(detected) = detect_protocol(line) else {
            return false;
        };
        self.identity_checked = true;
        if detected == ControllerProtocol::Unknown || detected == self.configured {
            return false;
        }
        tracing::warn!(
            configured = %self.configured,
            %detected,
            "the controller's firmware differs from the active profile; disconnecting"
        );
        self.sink
            .emit(MachineEvent::Received(line.trim().to_string()));
        self.sink.emit(MachineEvent::ProtocolMismatch {
            configured: self.configured,
            detected,
        });
        self.rejected = true;
        true
    }

    /// Handles a stop, pause, resume or real-time command on a Repetier connection, where no
    /// GRBL real-time byte is ever written (see [`MachineController::realtime`](super::MachineController::realtime)).
    pub(super) fn handle_repetier_command(&mut self, cmd: Command) -> Result<(), String> {
        let realtime = |byte: u8, command: RealtimeCommand| byte == command.byte();
        match cmd {
            Command::Stop => self.repetier_emergency_stop()?,
            Command::Realtime(byte) if realtime(byte, RealtimeCommand::SoftReset) => {
                self.repetier_emergency_stop()?;
            }
            Command::Pause => self.paused = true,
            Command::Realtime(byte) if realtime(byte, RealtimeCommand::FeedHold) => {
                self.paused = true;
            }
            Command::Resume => self.paused = false,
            Command::Realtime(byte) if realtime(byte, RealtimeCommand::CycleStart) => {
                self.paused = false;
            }
            Command::Realtime(byte) if realtime(byte, RealtimeCommand::JogCancel) => {
                // Repetier cannot cancel a jog it runs; one still queued never starts.
                self.drop_queued_jogs();
            }
            Command::Realtime(byte) if realtime(byte, RealtimeCommand::StatusQuery) => {
                tracing::trace!("status query: Repetier has none; nothing written");
            }
            Command::Realtime(byte) => self.sink.emit(MachineEvent::Error(format!(
                "{}: 0x{byte:02X}",
                worker_text::NOT_ON_REPETIER
            ))),
            // Routed to `handle_command`'s shared arms by the caller.
            Command::Line(..)
            | Command::StateLine(..)
            | Command::Stream(..)
            | Command::Shutdown => {}
        }
        Ok(())
    }

    /// The Repetier emergency stop: `M112` written at once, then every line queued, in
    /// flight or still in the channel is dropped (never to run, as after a GRBL reset) and
    /// the stream ends. `M112` is written unnumbered, so the firmware acts on it whatever line
    /// number it expects next (a numbered line out of sequence would only be asked again).
    pub(super) fn repetier_emergency_stop(&mut self) -> Result<(), String> {
        self.write(b"M112\n")?;
        self.halted.store(true, Ordering::SeqCst);
        self.sink
            .emit(MachineEvent::Sent(REPETIER_ESTOP_LABEL.to_string()));
        self.reset_at = Some(Instant::now());
        if let Some(link) = self.repetier.as_mut() {
            link.acks_due = 0;
            link.internal_in_flight = false;
            link.internal.clear();
            // Nothing sent before the stop may be replayed after it.
            link.processor = RepetierProcessor::new();
        }
        self.reset_buffers();
        self.end_stream(false);
        Ok(())
    }

    /// Numbers, checksums and writes one command for Repetier.
    ///
    /// # Returns
    /// How many numbered lines were written (one acknowledgement is due for each), or `None`
    /// when the command has no Repetier equivalent and nothing was written: a GRBL `$`
    /// command (other than `$H` and `$J=`) or real-time character is never written.
    fn write_repetier(&mut self, line: &str) -> Result<Option<usize>, String> {
        let upper = line.trim().to_ascii_uppercase();
        let Some(link) = self.repetier.as_mut() else {
            return Ok(None);
        };
        if matches!(upper.as_str(), "?" | "!" | "~") || !repetier_translates(&upper) {
            return Ok(None);
        }
        let Some(bytes) = link.processor.process_outgoing(line.trim().as_bytes()) else {
            return Ok(None);
        };
        let count = bytes.iter().filter(|b| **b == b'\n').count();
        if count == 0 {
            return Ok(None);
        }
        // A move: the stand-in status report says `Run` until it is acknowledged.
        if upper.starts_with('G') || upper.starts_with('X') || upper.starts_with("$J=") {
            link.processor.mark_buffer_full();
        }
        self.write(&bytes)?;
        Ok(Some(count))
    }

    /// Notes a command written as `count` numbered lines as the one in flight.
    fn set_in_flight(&mut self, count: usize, internal: bool) {
        if let Some(link) = self.repetier.as_mut() {
            link.acks_due = count;
            link.internal_in_flight = internal;
        }
    }

    /// Writes the next command once the one in flight is acknowledged: the worker's own
    /// start-up lines first, then manual lines, then program lines (unless paused). A
    /// command with no Repetier equivalent is reported and skipped.
    pub(super) fn feed_repetier(&mut self) -> Result<(), String> {
        while self.repetier.as_ref().is_some_and(|l| l.acks_due == 0) {
            if let Some(line) = self.repetier.as_mut().and_then(|l| l.internal.pop_front()) {
                if let Some(count) = self.write_repetier(line)? {
                    self.set_in_flight(count, true);
                    self.sink.emit(MachineEvent::Sent(line.to_string()));
                }
                continue;
            }
            if let Some((line, seq)) = self.manual.pop_front() {
                match self.write_repetier(&line)? {
                    Some(count) => {
                        self.set_in_flight(count, false);
                        self.in_flight_is_program.push_back(false);
                        self.in_flight_seq.push_back(seq);
                        self.written_seq = self.written_seq.max(seq);
                        self.sink.emit(MachineEvent::Sent(line));
                    }
                    None => {
                        self.sink.emit(MachineEvent::Error(format!(
                            "{}: {line}",
                            worker_text::NOT_ON_REPETIER
                        )));
                        self.drop_line(seq);
                    }
                }
                continue;
            }
            if self.paused {
                return Ok(());
            }
            let Some(line) = self
                .stream
                .as_ref()
                .and_then(|s| s.program.get(s.sent))
                .map(|l| l.trim().to_string())
            else {
                return Ok(());
            };
            let Some(line) = self.pen_mode_line(line) else {
                // A pen command repeating the pen's state: skipped, done.
                if let Some(stream) = self.stream.as_mut() {
                    stream.sent += 1;
                    stream.acked += 1;
                    let event = MachineEvent::StreamProgress {
                        sent: stream.sent,
                        acked: stream.acked,
                        total: stream.program.len(),
                    };
                    self.sink.emit(event);
                }
                self.finish_if_done();
                continue;
            };
            let written = self.write_repetier(&line)?;
            match written {
                Some(count) => {
                    self.set_in_flight(count, false);
                    self.in_flight_is_program.push_back(true);
                }
                None => self.sink.emit(MachineEvent::Error(format!(
                    "{}: {line}",
                    worker_text::NO_REPETIER_EQUIVALENT
                ))),
            }
            if let Some(stream) = self.stream.as_mut() {
                stream.sent += 1;
                if written.is_none() {
                    // Skipped, as GTK's sender did: nothing to acknowledge.
                    stream.acked += 1;
                }
                let event = MachineEvent::StreamProgress {
                    sent: stream.sent,
                    acked: stream.acked,
                    total: stream.program.len(),
                };
                self.sink.emit(event);
            }
            if written.is_none() {
                self.finish_if_done();
            }
        }
        Ok(())
    }

    /// The program line `line` as pen mode writes it (GTK's `apply_pen_mode`): `M3`/`M4`
    /// lower the pen (`G0 Z0`), `M5` lifts it (`G0 Z5`), and `None` for one that repeats the
    /// pen's state. Without pen mode, `line` unchanged.
    fn pen_mode_line(&mut self, line: String) -> Option<String> {
        let Some(link) = self
            .repetier
            .as_mut()
            .filter(|l| l.pen_mode.load(Ordering::Relaxed))
        else {
            return Some(line);
        };
        let upper = line.to_ascii_uppercase();
        match upper.split_whitespace().next().unwrap_or("") {
            "M3" | "M4" if !link.pen_up => None,
            "M3" | "M4" => {
                link.pen_up = false;
                Some("G0 Z0".to_string())
            }
            "M5" if link.pen_up => None,
            "M5" => {
                link.pen_up = true;
                Some("G0 Z5".to_string())
            }
            _ => Some(line),
        }
    }

    /// Handles bytes read on a Repetier connection: acknowledgements and resend requests
    /// first, then the reply lines (with the processor's stand-in status report).
    pub(super) fn receive_repetier(&mut self, data: Vec<u8>) -> Result<(), String> {
        let Some(link) = self.repetier.as_mut() else {
            return Ok(());
        };
        let result = link.processor.process_incoming(data);
        for event in result.events {
            match event {
                RepetierEvent::Ack => self.on_repetier_ack(),
                RepetierEvent::Resend(from) => self.on_repetier_resend(from)?,
            }
        }
        for line in self.splitter.push(&result.display) {
            self.handle_repetier_line(&line);
        }
        Ok(())
    }

    /// One acknowledgement (`ok`, `wait`): the command in flight is answered once every
    /// numbered line it was written as is. Nothing in flight (Repetier's idle `wait`): ignored.
    fn on_repetier_ack(&mut self) {
        let Some(link) = self.repetier.as_mut() else {
            return;
        };
        if link.acks_due == 0 {
            return;
        }
        link.acks_due -= 1;
        if link.acks_due > 0 || std::mem::take(&mut link.internal_in_flight) {
            return;
        }
        self.complete_oldest(None);
    }

    /// A `Resend:N`: writes the numbered lines from `N` again, exactly as first sent, and
    /// waits for an acknowledgement of each (GTK's `RepetierSender`). A line no longer known
    /// cannot be sent again: the stream stops.
    fn on_repetier_resend(&mut self, from: u32) -> Result<(), String> {
        if self.halted.load(Ordering::SeqCst) {
            // After `M112` only `M112` is written, until the firmware restarts.
            tracing::info!(from, "Repetier resend ignored: emergency-stopped");
            return Ok(());
        }
        let Some(link) = self.repetier.as_mut() else {
            return Ok(());
        };
        let streak = if link.resend_streak.0 == from {
            link.resend_streak.1 + 1
        } else {
            1
        };
        link.resend_streak = (from, streak);
        let lines = if streak > MAX_REPETIER_RESENDS {
            tracing::warn!(from, streak, "Repetier keeps asking for the same line");
            None
        } else {
            link.processor.resend_lines(from)
        };
        match lines {
            Some(lines) if lines.is_empty() => Ok(()),
            Some(lines) => {
                // Repetier answers a resend request with `ok` as well, after `Resend:`: one
                // acknowledgement more than the lines replayed, so the next line waits for the
                // replayed one's own `ok`. A firmware that sends none recovers with its idle
                // `wait`.
                link.acks_due = lines.len() + 1;
                link.processor.mark_buffer_full();
                tracing::info!(from, count = lines.len(), "Repetier resend");
                self.write(lines.concat().as_bytes())
            }
            None => {
                tracing::warn!(from, "Repetier resend not honoured");
                self.sink.emit(MachineEvent::Error(format!(
                    "{}: N{from}",
                    worker_text::RESEND_FAILED
                )));
                self.end_stream(false);
                Ok(())
            }
        }
    }

    /// Handles one reply line on a Repetier connection. Acknowledgements were counted from
    /// the processor's events; the stand-in `<...>` report is the status report (fresh once
    /// no line waits for its acknowledgement); `start` means the firmware restarted, which
    /// drops every line queued or in flight, as a GRBL reset banner does.
    fn handle_repetier_line(&mut self, line: &str) {
        if self.rejected || self.check_identity(line) {
            return;
        }
        let line = line.trim();
        let lower = line.to_ascii_lowercase();
        if line.is_empty() || lower.starts_with("ok") || lower == "wait" {
            return;
        }
        if line.starts_with('<') && line.ends_with('>') {
            let status = StatusParser::parse_full(line);
            self.sink.emit(MachineEvent::StatusReport(Box::new(status)));
            if self.repetier.as_ref().is_some_and(|l| l.acks_due == 0) && self.manual.is_empty() {
                self.fresh.store(self.written_seq, Ordering::SeqCst);
            }
            return;
        }
        if lower == "start" {
            self.reset_at = Some(Instant::now());
            self.halted.store(false, Ordering::SeqCst);
            self.sink.emit(MachineEvent::Received(line.to_string()));
            self.reset_buffers();
            // The firmware counts lines from the start again: number from scratch and send
            // the start-up lines once more.
            if let Some(link) = self.repetier.as_mut() {
                *link = RepetierLink::new(link.pen_mode.clone());
            }
            self.end_stream(false);
            return;
        }
        if lower.starts_with("error:") {
            self.sink.emit(MachineEvent::Error(line.to_string()));
        } else {
            self.sink.emit(MachineEvent::Received(line.to_string()));
        }
    }
}
