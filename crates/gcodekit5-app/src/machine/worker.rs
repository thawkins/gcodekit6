//! The worker thread's state ([`WorkerState`]) and its GRBL I/O loop: command handling,
//! character-counting streaming, response handling, halts and resets.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, PoisonError};
use std::time::{Duration, Instant};

use gcodekit5_communication::firmware::grbl::status_parser::StatusParser;
use gcodekit5_communication::firmware::repetier::RepetierProcessor;
use gcodekit5_communication::firmware::{ControllerProtocol, protocol_profile};
use gcodekit5_communication::{Communicator, ConnectionParams};
use gcodekit5_settings::{ConnectionSettings, ConnectionType};

use super::grbl_stream::{
    CharCountWindow, GrblResponseLine, ResponseLineSplitter, classify_response,
};
use super::types::{
    Command, MachineEvent, RealtimeCommand, SharedHook, StreamMode, realtime_label, worker_text,
};
use super::{
    HALT_MAX_WAIT, HALT_QUERY_INTERVAL, HALT_TIMEOUT, IDLE_WAIT, RECONNECT_ATTEMPTS,
    RECONNECT_DELAYS, STATUS_INTERVAL,
};

/// Whether a command line has a Repetier equivalent: anything but a GRBL `$` command, except
/// homing (`$H`, sent as `G28`) and jogging (`$J=`, sent as `G1` moves), which
/// [`RepetierProcessor`] translates.
///
/// # Arguments
/// * `line` - One command line.
pub(super) fn repetier_translates(line: &str) -> bool {
    let upper = line.trim().to_ascii_uppercase();
    !upper.starts_with('$') || upper.starts_with("$H") || upper.starts_with("$J=")
}

/// Whether `line` is the emergency stop `M112`, as firmware reads it: whatever the case and
/// spacing, with or without a line number (`N5`), checksum (`*23`) and comments (`; ...`,
/// `(...)`).
pub(super) fn is_m112(line: &str) -> bool {
    let code = line.split(';').next().unwrap_or_default();
    let code = code.split('*').next().unwrap_or_default();
    let mut text = String::with_capacity(code.len());
    let mut depth = 0usize;
    for c in code.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            c if depth == 0 && !c.is_whitespace() => text.push(c.to_ascii_uppercase()),
            _ => {}
        }
    }
    let text = match text.strip_prefix('N') {
        Some(rest) => rest.trim_start_matches(|c: char| c.is_ascii_digit()),
        None => text.as_str(),
    };
    text == "M112"
}

/// Maps the stored connection settings to transport parameters.
pub(super) fn connection_params(cfg: &ConnectionSettings) -> ConnectionParams {
    let mut params = match cfg.connection_type {
        ConnectionType::Serial => ConnectionParams::serial(&cfg.port, cfg.baud_rate),
        ConnectionType::Tcp => ConnectionParams::tcp(&cfg.port, cfg.tcp_port),
        ConnectionType::WebSocket => ConnectionParams::websocket(&cfg.port, cfg.tcp_port),
    };
    params.timeout_ms = cfg.timeout_ms;
    params.auto_reconnect = cfg.auto_reconnect;
    params.max_retries = RECONNECT_ATTEMPTS;
    params
}

/// Sends events to the controller and pokes the repaint hook.
pub(super) struct EventSink {
    pub(super) events: Sender<MachineEvent>,
    pub(super) repaint: SharedHook,
}

impl EventSink {
    pub(super) fn emit(&self, event: MachineEvent) {
        // The controller may already be gone during shutdown; nothing to tell then.
        if self.events.send(event).is_ok()
            && let Some(hook) = self
                .repaint
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref()
        {
            hook();
        }
    }
}

/// The program being streamed.
pub(super) struct Stream {
    pub(super) program: Arc<[String]>,
    pub(super) sent: usize,
    pub(super) acked: usize,
    pub(super) mode: StreamMode,
}

/// A [`StreamMode::StopOnError`] stream being halted: feed hold sent, waiting for it to
/// complete before the soft reset.
pub(super) struct Halt {
    /// Why (the controller's reply).
    pub(super) reason: String,
    /// When the feed hold was written.
    pub(super) started: Instant,
    /// When to soft-reset without a completed hold: [`HALT_TIMEOUT`] after the hold or the
    /// latest status report that did not complete it (a stale one, `Hold:1`, `Idle`, ...),
    /// never later than [`HALT_MAX_WAIT`] after the hold.
    pub(super) deadline: Instant,
    /// When to ask for the next status report.
    pub(super) next_query: Instant,
    /// Status reports still owed to `?`s written before the hold; they may describe the
    /// machine before it (even a `Hold:0` of an earlier pause), so they do not count as a
    /// completed hold.
    pub(super) stale_reports: usize,
}

/// The Repetier side of a worker on a Repetier connection: one command in flight at a time.
pub(super) struct RepetierLink {
    /// Numbers and checksums the lines written, keeps them for resends, frames the replies and
    /// stands in a GRBL-style status report for the position readout.
    pub(super) processor: RepetierProcessor,
    /// Acknowledgements (`ok`, `wait`) still owed to the command in flight; it may have been
    /// written as several numbered lines (a jog). Zero when nothing is in flight.
    pub(super) acks_due: usize,
    /// Whether the command in flight is one of `internal`'s (not a program or manual line).
    pub(super) internal_in_flight: bool,
    /// The line of the last `Resend:` and how many times in a row it was asked for.
    pub(super) resend_streak: (u32, u32),
    /// Lines the worker sends itself: the protocol's start-up commands (`M110`, `M115`) at
    /// connect and after the controller restarts.
    pub(super) internal: VecDeque<&'static str>,
    /// Pen mode ([`MachineController::set_pen_mode`](super::MachineController::set_pen_mode)), read per program line.
    pub(super) pen_mode: Arc<AtomicBool>,
    /// In pen mode, whether the pen is up; down at the start of each program, so its first
    /// `M5` lifts it.
    pub(super) pen_up: bool,
}

impl RepetierLink {
    /// A link that starts with the protocol's start-up commands.
    pub(super) fn new(pen_mode: Arc<AtomicBool>) -> Self {
        Self {
            pen_mode,
            pen_up: false,
            processor: RepetierProcessor::new(),
            acks_due: 0,
            internal_in_flight: false,
            resend_streak: (0, 0),
            internal: protocol_profile(ControllerProtocol::Repetier.as_str())
                .init_commands
                .iter()
                .copied()
                .collect(),
        }
    }
}

/// Everything the worker thread owns.
pub(super) struct WorkerState {
    pub(super) comm: Box<dyn Communicator>,
    /// The protocol the connection was opened for.
    pub(super) configured: ControllerProtocol,
    /// Set on a Repetier connection.
    pub(super) repetier: Option<RepetierLink>,
    /// Whether the controller's identity was checked against `configured`.
    pub(super) identity_checked: bool,
    /// The controller identified as another firmware: the worker closes the connection.
    pub(super) rejected: bool,
    /// Shared with the controller: the Repetier firmware was emergency-stopped (`M112`).
    pub(super) halted: Arc<AtomicBool>,
    pub(super) commands: Receiver<Command>,
    pub(super) sink: EventSink,
    pub(super) streaming: Arc<AtomicBool>,
    pub(super) ready: Arc<AtomicBool>,
    pub(super) window: CharCountWindow,
    /// One entry per line in `window`: true for a program line, false for a manual one.
    pub(super) in_flight_is_program: VecDeque<bool>,
    /// The state-line sequence number of each manual line in `window`, oldest first (0 for
    /// a jog).
    pub(super) in_flight_seq: VecDeque<u64>,
    /// Shared with the controller: the highest state-line sequence number answered or
    /// dropped, see [`MachineController::lines_answered`](super::MachineController::lines_answered).
    pub(super) answered: Arc<AtomicU64>,
    /// Shared with the controller: how many jogs (sequence number 0) were answered or
    /// dropped, see [`MachineController::lines_answered`](super::MachineController::lines_answered).
    pub(super) jogs_answered: Arc<AtomicU64>,
    pub(super) splitter: ResponseLineSplitter,
    /// Manual lines waiting for room, each with its state-line sequence number (0 for a jog).
    pub(super) manual: VecDeque<(String, u64)>,
    pub(super) stream: Option<Stream>,
    pub(super) paused: bool,
    /// The highest state-line sequence number written (or dropped, never to run).
    pub(super) written_seq: u64,
    /// Shared with the controller: the `written_seq` tag of the latest status report.
    pub(super) fresh: Arc<AtomicU64>,
    /// One tag per `?` written and not yet answered, oldest first: `written_seq` when it was
    /// written. A status report answers the oldest `?`, so its tag says which state lines it
    /// can describe; a report to a `?` written before `$H` does not count as after `$H`.
    /// Capped at [`MAX_QUERY_TAGS`](super::MAX_QUERY_TAGS): on overflow the two oldest tags merge into the lower one,
    /// so a lagging queue only ever under-states freshness.
    pub(super) query_tags: VecDeque<u64>,
    /// When each `?` not yet answered was written, oldest first: how many reports a halt must
    /// skip as possibly describing the machine before the hold. A status report pops the
    /// oldest entry (nothing when none is left, so an unsolicited report cannot underflow
    /// it). The count corrects itself and stays bounded: an entry older than
    /// [`Self::lost_reply_timeout`] is presumed lost and dropped (logged), at most
    /// [`MAX_UNANSWERED_QUERIES`](super::MAX_UNANSWERED_QUERIES) are kept (the oldest goes first), and a reset clears them.
    /// An unsolicited report (auto-reporting firmware) can still answer for a `?` and make
    /// the count low until that `?`'s own reply arrives; the next `?`s set it right again.
    /// A miscount cannot make a halt reset in motion: only a `Hold:0` or `Alarm` report
    /// completes a halt, see [`Halt`].
    pub(super) unanswered_queries: VecDeque<Instant>,
    /// The slowest recent reply: the age of the `?` each status report answered, decaying by
    /// 1/16 per report. A slow link (a Wi-Fi or telnet bridge) raises it, so its late replies
    /// are not presumed lost.
    pub(super) reply_latency: Duration,
    /// Whether lost replies were logged since the last status report, so a dead link logs
    /// once rather than at every `?`.
    pub(super) lost_logged: bool,
    /// A `$J` jog was queued and is not known to be over: set when it is queued and again when
    /// it is written, cleared by jog cancel (0x85, which also drops queued jogs), soft reset
    /// (0x18), or an `Idle` report after a `Jog` one received since it was written.
    pub(super) jog_outstanding: bool,
    /// A `Jog` state was reported since the outstanding jog was written.
    pub(super) jog_seen: bool,
    /// The stream being halted after an error, see [`StreamMode::StopOnError`].
    pub(super) halt: Option<Halt>,
    /// When the controller's last reset banner was read: manual lines sent before it are
    /// dropped when they arrive, as the ones already queued were.
    pub(super) reset_at: Option<Instant>,
}

/// Why the worker loop ended.
pub(super) enum Exit {
    Shutdown,
    TransportError(String),
}

/// How an automatic reconnect ended.
enum Reconnect {
    /// The transport is open again.
    Connected,
    /// The worker gave up, or was told to stop; the transport is closed and
    /// [`MachineEvent::Disconnected`] was reported.
    Stopped,
}

/// How long the worker waits before reconnect attempt `attempt` (from 1).
fn reconnect_delay(attempt: u32) -> Duration {
    let index = usize::try_from(attempt.saturating_sub(1)).unwrap_or(usize::MAX);
    RECONNECT_DELAYS[index.min(RECONNECT_DELAYS.len() - 1)]
}

impl WorkerState {
    pub(super) fn new(
        comm: Box<dyn Communicator>,
        commands: Receiver<Command>,
        sink: EventSink,
        streaming: Arc<AtomicBool>,
        ready: Arc<AtomicBool>,
        fresh: Arc<AtomicU64>,
        [answered, jogs_answered]: [Arc<AtomicU64>; 2],
    ) -> Self {
        Self {
            comm,
            configured: ControllerProtocol::Unknown,
            repetier: None,
            identity_checked: false,
            rejected: false,
            halted: Arc::new(AtomicBool::new(false)),
            commands,
            sink,
            streaming,
            ready,
            window: CharCountWindow::default(),
            in_flight_is_program: VecDeque::new(),
            in_flight_seq: VecDeque::new(),
            answered,
            jogs_answered,
            splitter: ResponseLineSplitter::new(),
            manual: VecDeque::new(),
            stream: None,
            paused: false,
            written_seq: 0,
            fresh,
            query_tags: VecDeque::new(),
            unanswered_queries: VecDeque::new(),
            reply_latency: Duration::ZERO,
            lost_logged: false,
            jog_outstanding: false,
            jog_seen: false,
            halt: None,
            reset_at: None,
        }
    }

    /// Connects, runs the I/O loop until shutdown or a transport error, then
    /// disconnects.
    pub(super) fn run(mut self, params: &ConnectionParams) {
        if let Err(e) = self.comm.connect(params) {
            self.sink.emit(MachineEvent::Error(format!(
                "{}: {e}",
                worker_text::CONNECT_FAILED
            )));
            // A stream queued while connecting never started.
            self.streaming.store(false, Ordering::SeqCst);
            self.sink.emit(MachineEvent::Disconnected);
            return;
        }
        self.ready.store(true, Ordering::SeqCst);
        self.sink.emit(MachineEvent::Connected);

        let mut exit = self.io_loop();
        // Auto Reconnect: a dropped link (not a Disconnect) is reopened with backoff.
        while params.auto_reconnect
            && let Exit::TransportError(msg) = exit
        {
            match self.reconnect(msg, params) {
                Reconnect::Connected => exit = self.io_loop(),
                Reconnect::Stopped => return,
            }
        }
        if let Exit::TransportError(msg) = exit {
            self.sink.emit(MachineEvent::Error(msg));
        } else if self.jog_outstanding && self.repetier.is_none() {
            // A jog toward the far end of travel must not outlive the connection.
            if let Err(e) = self.write(&[RealtimeCommand::JogCancel.byte()]) {
                tracing::warn!("jog cancel on disconnect failed: {e}");
            }
            self.jog_outstanding = false;
        }
        self.end_stream(false);
        if let Err(e) = self.comm.disconnect() {
            tracing::warn!("machine transport disconnect failed: {e}");
        }
        self.sink.emit(MachineEvent::Disconnected);
    }

    /// Handles a dropped connection with Auto Reconnect on: reports the error, stops a
    /// streaming program for good (it is not resumed after the reconnect), closes the
    /// transport and reports [`MachineEvent::Disconnected`], then tries to reopen it up to
    /// `params.max_retries` times, waiting [`RECONNECT_DELAYS`] before each attempt.
    ///
    /// While it waits, `ready` is false, so a Disconnect detaches the worker at once; a
    /// shutdown ends the attempts. Commands that need the connection are refused.
    ///
    /// # Arguments
    /// * `msg` - The transport error that ended the connection.
    /// * `params` - The connection to reopen.
    fn reconnect(&mut self, msg: String, params: &ConnectionParams) -> Reconnect {
        self.sink.emit(MachineEvent::Error(msg));
        if self.stream.is_some() {
            self.sink.emit(MachineEvent::Error(
                worker_text::JOB_STOPPED_CONNECTION_LOST.to_string(),
            ));
        }
        self.end_stream(false);
        self.ready.store(false, Ordering::SeqCst);
        if let Err(e) = self.comm.disconnect() {
            tracing::warn!("machine transport disconnect failed: {e}");
        }
        self.forget_connection();
        self.sink.emit(MachineEvent::Disconnected);

        let max_attempts = params.max_retries;
        for attempt in 1..=max_attempts {
            self.sink.emit(MachineEvent::Reconnecting {
                attempt,
                max_attempts,
            });
            if !self.wait_offline(reconnect_delay(attempt)) {
                tracing::info!("reconnect cancelled");
                return Reconnect::Stopped;
            }
            match self.comm.connect(params) {
                Ok(()) => {
                    tracing::info!(attempt, "machine connection reopened");
                    self.ready.store(true, Ordering::SeqCst);
                    self.sink.emit(MachineEvent::Connected);
                    return Reconnect::Connected;
                }
                Err(e) => tracing::warn!(attempt, max_attempts, "reconnect failed: {e}"),
            }
        }
        self.sink.emit(MachineEvent::Error(
            worker_text::RECONNECT_FAILED.to_string(),
        ));
        self.sink.emit(MachineEvent::Disconnected);
        Reconnect::Stopped
    }

    /// Forgets everything owed to the closed connection: lines in flight or queued, status
    /// queries, a pause or reset, the controller's identity and the Repetier line numbers.
    fn forget_connection(&mut self) {
        self.reset_buffers();
        self.paused = false;
        self.jog_seen = false;
        self.reset_at = None;
        self.identity_checked = false;
        self.lost_logged = false;
        self.reply_latency = Duration::ZERO;
        if let Some(link) = self.repetier.as_ref() {
            self.repetier = Some(RepetierLink::new(link.pen_mode.clone()));
        }
    }

    /// Waits `delay` with the transport closed, refusing commands that need it.
    ///
    /// # Returns
    /// False if the worker was told to shut down (or the controller is gone).
    fn wait_offline(&mut self, delay: Duration) -> bool {
        let deadline = Instant::now() + delay;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.commands.recv_timeout(left) {
                Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => return false,
                Ok(cmd) => self.refuse_offline(cmd),
                Err(RecvTimeoutError::Timeout) => return true,
            }
        }
    }

    /// Refuses `cmd` while the connection is being reopened: a program or line is reported
    /// as not sent (and counted as answered), real-time bytes are dropped.
    fn refuse_offline(&mut self, cmd: Command) {
        match cmd {
            Command::Stream(program, _, _) => {
                let total = program.len();
                self.sink
                    .emit(MachineEvent::Error(worker_text::NOT_CONNECTED.to_string()));
                self.streaming.store(false, Ordering::SeqCst);
                self.sink
                    .emit(MachineEvent::StreamStopped { acked: 0, total });
            }
            Command::Line(line, _) => {
                self.sink.emit(MachineEvent::Error(format!(
                    "{}: {line}",
                    worker_text::NOT_CONNECTED
                )));
                self.drop_line(0);
            }
            Command::StateLine(line, seq, _) => {
                self.sink.emit(MachineEvent::Error(format!(
                    "{}: {line}",
                    worker_text::NOT_CONNECTED
                )));
                self.drop_line(seq);
            }
            Command::Realtime(_) | Command::Pause | Command::Resume | Command::Stop => {
                tracing::debug!("real-time command dropped while reconnecting");
            }
            Command::Shutdown => {}
        }
    }

    fn io_loop(&mut self) -> Exit {
        let mut next_status = Instant::now();
        let mut idle = false;
        loop {
            if self.rejected {
                return Exit::Shutdown;
            }
            // 1. Commands first, so real-time bytes overtake queued lines.
            let mut wait = if idle { IDLE_WAIT } else { Duration::ZERO };
            loop {
                match self.commands.recv_timeout(wait) {
                    Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                        return Exit::Shutdown;
                    }
                    Ok(cmd) => {
                        if let Err(e) = self.handle_command(cmd) {
                            return Exit::TransportError(e);
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => break,
                }
                wait = Duration::ZERO;
            }

            // 2. Feed lines while the controller's RX buffer has room.
            let fed = if self.repetier.is_some() {
                self.feed_repetier()
            } else {
                self.feed()
            };
            if let Err(e) = fed {
                return Exit::TransportError(e);
            }

            // 2b. A halting stream: poll the hold, reset once it completes or times out.
            if let Err(e) = self.drive_halt() {
                return Exit::TransportError(e);
            }

            // 3. Periodic status request (GRBL's `?`; Repetier has none).
            let now = Instant::now();
            if now >= next_status && self.repetier.is_none() {
                if let Err(e) = self.write_status_query() {
                    return Exit::TransportError(e);
                }
                next_status = now + STATUS_INTERVAL;
            }

            // 4. Read and handle responses.
            match self.comm.receive() {
                Ok(data) if self.repetier.is_some() => {
                    idle = data.is_empty();
                    if !idle && let Err(e) = self.receive_repetier(data) {
                        return Exit::TransportError(e);
                    }
                }
                Ok(data) => {
                    idle = data.is_empty();
                    for line in self.splitter.push(&data) {
                        self.handle_response(&line);
                    }
                }
                Err(e) => {
                    return Exit::TransportError(format!("{}: {e}", worker_text::READ_FAILED));
                }
            }
        }
    }

    pub(super) fn write(&mut self, data: &[u8]) -> Result<(), String> {
        self.comm
            .send(data)
            .map(|_| ())
            .map_err(|e| format!("{}: {e}", worker_text::WRITE_FAILED))
    }

    fn handle_command(&mut self, cmd: Command) -> Result<(), String> {
        if self.repetier.is_some() {
            match &cmd {
                // A typed `M112` is the emergency stop too: written at once, unnumbered.
                Command::Line(line, _) | Command::StateLine(line, _, _) if is_m112(line) => {
                    if let Command::StateLine(_, seq, _) = cmd {
                        self.drop_line(seq);
                    } else {
                        self.drop_line(0);
                    }
                    return self.repetier_emergency_stop();
                }
                // Reached the worker after the controller was halted (still in the channel).
                Command::Line(line, _) | Command::StateLine(line, _, _)
                    if self.halted.load(Ordering::SeqCst) =>
                {
                    self.sink.emit(MachineEvent::Error(format!(
                        "{}: {line}",
                        worker_text::HALTED
                    )));
                    match cmd {
                        Command::StateLine(_, seq, _) => self.drop_line(seq),
                        _ => self.drop_line(0),
                    }
                    return Ok(());
                }
                Command::Stream(program, _, _) if self.halted.load(Ordering::SeqCst) => {
                    let total = program.len();
                    self.sink
                        .emit(MachineEvent::Error(worker_text::HALTED.to_string()));
                    self.streaming.store(false, Ordering::SeqCst);
                    self.sink
                        .emit(MachineEvent::StreamStopped { acked: 0, total });
                    return Ok(());
                }
                _ => {}
            }
        }
        if self.repetier.is_some()
            && matches!(
                cmd,
                Command::Realtime(_) | Command::Pause | Command::Resume | Command::Stop
            )
        {
            return self.handle_repetier_command(cmd);
        }
        match cmd {
            Command::Realtime(byte) if byte == RealtimeCommand::StatusQuery.byte() => {
                self.write_status_query()?;
            }
            // A cycle start during a halt would resume the moves buffered after the refused
            // line, and the reset that follows would then come in motion.
            Command::Realtime(byte)
                if byte == RealtimeCommand::CycleStart.byte() && self.halt.is_some() =>
            {
                self.refuse_cycle_start();
            }
            Command::Resume if self.halt.is_some() => self.refuse_cycle_start(),
            // A soft reset is a stop: nothing sent before it may be written after it.
            Command::Realtime(byte) if byte == RealtimeCommand::SoftReset.byte() => {
                self.soft_reset()?;
            }
            Command::Realtime(byte) => {
                self.write(&[byte])?;
                self.echo_realtime(byte);
                if byte == RealtimeCommand::JogCancel.byte() {
                    self.drop_queued_jogs();
                }
            }
            Command::Pause => {
                self.write(&[RealtimeCommand::FeedHold.byte()])?;
                self.echo_realtime(RealtimeCommand::FeedHold.byte());
                self.paused = true;
            }
            Command::Resume => {
                self.write(&[RealtimeCommand::CycleStart.byte()])?;
                self.echo_realtime(RealtimeCommand::CycleStart.byte());
                self.paused = false;
            }
            Command::Stop => self.soft_reset()?,
            Command::Line(line, sent) => self.queue_manual(line, 0, sent),
            Command::StateLine(line, seq, sent) => self.queue_manual(line, seq, sent),
            Command::Stream(program, _, sent)
                if self.reset_at.is_some_and(|reset| sent < reset) =>
            {
                // Started before the controller reset itself: it must not run after it.
                let total = program.len();
                tracing::info!(total, "program started before a controller reset dropped");
                self.sink.emit(MachineEvent::Error(format!(
                    "{}: {}",
                    worker_text::NOT_SENT_AFTER_RESET,
                    program.first().map_or("", String::as_str)
                )));
                self.streaming.store(false, Ordering::SeqCst);
                self.sink
                    .emit(MachineEvent::StreamStopped { acked: 0, total });
            }
            Command::Stream(program, mode, _) => {
                self.paused = false;
                if let Some(link) = self.repetier.as_mut() {
                    link.pen_up = false;
                }
                self.stream = Some(Stream {
                    program,
                    sent: 0,
                    acked: 0,
                    mode,
                });
                self.finish_if_done();
            }
            // Handled by the caller before dispatch.
            Command::Shutdown => {}
        }
        Ok(())
    }

    /// Reports a cycle start refused while a stream halts.
    fn refuse_cycle_start(&self) {
        tracing::warn!("cycle start ignored while a probe stream halts");
        self.sink.emit(MachineEvent::Error(
            worker_text::CYCLE_START_IGNORED.to_string(),
        ));
    }

    /// Reports a written real-time byte as [`MachineEvent::Sent`] when it has a label.
    fn echo_realtime(&self, byte: u8) {
        if let Some(label) = realtime_label(byte) {
            self.sink.emit(MachineEvent::Sent(label.to_string()));
        }
    }
}

impl WorkerState {
    /// Queues a manual line (`seq` > 0 for a state line), or reports one too long for the
    /// controller's buffer, which then never runs. (`send_line` already refuses such lines;
    /// this is defence in depth.)
    fn queue_manual(&mut self, line: String, seq: u64, sent: Instant) {
        // Sent before the controller reset itself, but still in the channel when its banner
        // was read: the reset dropped everything queued then, so this line goes too. A jog
        // written after the reset would move a machine the user saw reset.
        if self.reset_at.is_some_and(|reset| sent < reset) {
            tracing::info!(%line, "manual line sent before a controller reset dropped");
            self.sink.emit(MachineEvent::Error(format!(
                "{}: {line}",
                worker_text::NOT_SENT_AFTER_RESET
            )));
            self.drop_line(seq);
            return;
        }
        if !self.window.fits_at_all(&line) {
            self.sink.emit(MachineEvent::Error(format!(
                "{}: {line}",
                worker_text::LINE_TOO_LONG
            )));
            self.drop_line(seq);
            return;
        }
        if line.starts_with("$J=") {
            self.jog_outstanding = true;
            self.jog_seen = false;
        }
        self.manual.push_back((line, seq));
    }

    /// Counts a manual line that never runs (refused, or dropped from the queue) as
    /// answered: a jog (`seq` 0) at once; a state line once every lower state line still
    /// queued or in flight is, by handing its sequence number to the last of them.
    pub(super) fn drop_line(&mut self, seq: u64) {
        if seq == 0 {
            self.jogs_answered.fetch_add(1, Ordering::SeqCst);
            return;
        }
        match self.manual.iter_mut().rev().find(|(_, s)| *s > 0) {
            Some((_, queued)) => *queued = (*queued).max(seq),
            None => {
                self.written_seq = self.written_seq.max(seq);
                // Answered with the last state line in flight, or at once when none is.
                match self.in_flight_seq.iter_mut().rev().find(|s| **s > 0) {
                    Some(in_flight) => *in_flight = (*in_flight).max(seq),
                    None => {
                        self.answered.fetch_max(seq, Ordering::SeqCst);
                    }
                }
            }
        }
    }

    /// Writes manual lines, then program lines, while they fit in the RX buffer. Nothing is
    /// written while a stream halts: the reset that ends the halt drops queued manual lines.
    fn feed(&mut self) -> Result<(), String> {
        if self.halt.is_some() {
            return Ok(());
        }
        while let Some((line, seq)) = self.manual.front() {
            if !self.window.can_send(line) {
                return Ok(());
            }
            let seq = *seq;
            let is_jog = line.starts_with("$J=");
            let line = format!("{line}\n");
            self.write(line.as_bytes())?;
            if is_jog {
                // Only reports after the jog is written can say it ran and finished.
                self.jog_outstanding = true;
                self.jog_seen = false;
            }
            self.window.on_sent(line.trim_end_matches('\n'));
            self.sink
                .emit(MachineEvent::Sent(line.trim_end_matches('\n').to_string()));
            self.in_flight_is_program.push_back(false);
            self.in_flight_seq.push_back(seq);
            self.written_seq = self.written_seq.max(seq);
            self.manual.pop_front();
        }

        if self.paused {
            return Ok(());
        }
        let Some(stream) = self.stream.as_ref() else {
            return Ok(());
        };
        let program = stream.program.clone();
        let mut sent = stream.sent;
        let start = sent;
        while let Some(raw) = program.get(sent) {
            let line = raw.trim();
            if !self.window.fits_at_all(line) {
                self.sink.emit(MachineEvent::Error(format!(
                    "{}: {}",
                    worker_text::PROGRAM_LINE_TOO_LONG,
                    sent + 1
                )));
                self.end_stream(false);
                return Ok(());
            }
            if !self.window.can_send(line) {
                break;
            }
            self.write(format!("{line}\n").as_bytes())?;
            self.window.on_sent(line);
            self.in_flight_is_program.push_back(true);
            sent += 1;
        }
        if sent != start
            && let Some(stream) = self.stream.as_mut()
        {
            stream.sent = sent;
            let event = MachineEvent::StreamProgress {
                sent,
                acked: stream.acked,
                total: stream.program.len(),
            };
            self.sink.emit(event);
        }
        Ok(())
    }

    fn handle_response(&mut self, line: &str) {
        if self.rejected || self.check_identity(line) {
            return;
        }
        match classify_response(line) {
            GrblResponseLine::Ok => self.on_ack(None),
            GrblResponseLine::Error(code) => self.on_ack(Some(code)),
            GrblResponseLine::Alarm(code) => {
                self.sink.emit(MachineEvent::Error(format!("ALARM:{code}")));
                self.end_stream(false);
            }
            GrblResponseLine::Status(report) => {
                let status = StatusParser::parse_full(&report);
                self.answer_query();
                let state = status.machine_state.as_deref();
                // GRBL 1.1 and grblHAL enter Hold on `!` from any running or idle state (from
                // Idle the hold is complete at once: `Hold:0`), so after the hold only
                // `Hold:0` proves the machine stopped; an `Alarm` has stopped it too. `Idle`
                // or `Run` can only be a report from before the hold (a late reply on a slow
                // link, or a miscounted one), and `Hold:1` says it still decelerates.
                let stopped =
                    state.is_some_and(|s| s.starts_with("Hold:0") || s.starts_with("Alarm"));
                if let Some(halt) = self.halt.as_mut() {
                    if halt.stale_reports == 0 && stopped {
                        self.finish_halt();
                    } else {
                        // A report owed to a `?` from before the hold, or one that does not
                        // say the machine stopped: replies are still coming, so wait for the
                        // next one rather than reset in motion (never past the cap).
                        halt.stale_reports = halt.stale_reports.saturating_sub(1);
                        let cap = halt.started + HALT_MAX_WAIT;
                        halt.deadline = halt.deadline.max(Instant::now() + HALT_TIMEOUT).min(cap);
                    }
                }
                self.follow_jog(status.machine_state.as_deref());
                self.sink.emit(MachineEvent::StatusReport(Box::new(status)));
                // After the event: a caller that sees the report fresh has the report too.
                // The report answers the oldest `?`; without one tracked, leave it be.
                if let Some(tag) = self.query_tags.pop_front() {
                    self.fresh.store(tag, Ordering::SeqCst);
                }
            }
            GrblResponseLine::Reset(banner) => {
                // The controller restarted: nothing in flight will be acknowledged, and no
                // line sent before it may run after it. Stamped before the banner is
                // reported, so a line sent in answer to it (the `$G` that asks the active
                // system again) is never taken for one sent before the reset.
                self.reset_at = Some(Instant::now());
                // Report the banner before the stream end it causes, so a consumer
                // that stops at `StreamStopped` has already seen why.
                self.sink.emit(MachineEvent::Received(banner));
                self.reset_buffers();
                self.end_stream(false);
            }
            GrblResponseLine::Other(text) => self.sink.emit(MachineEvent::Received(text)),
        }
    }

    /// Clears the outstanding jog once the controller reports `Idle` after `Jog`.
    fn follow_jog(&mut self, state: Option<&str>) {
        if !self.jog_outstanding {
            return;
        }
        match state {
            Some(s) if s.starts_with("Jog") => self.jog_seen = true,
            Some(s) if s.starts_with("Idle") && self.jog_seen => {
                self.jog_outstanding = false;
                self.jog_seen = false;
            }
            Some(_) | None => {}
        }
    }

    /// Handles one `ok` (`error` is `None`) or `error:N` reply.
    fn on_ack(&mut self, error: Option<String>) {
        if !self.window.on_ack() {
            if let Some(code) = error {
                self.sink.emit(MachineEvent::Error(format!("error:{code}")));
            }
            return;
        }
        self.complete_oldest(error);
    }

    /// Takes the oldest line in flight as answered (`error` is `None` for `ok`): counts a
    /// manual line answered, a program line acknowledged.
    pub(super) fn complete_oldest(&mut self, error: Option<String>) {
        let is_program = self.in_flight_is_program.pop_front().unwrap_or(false);
        if !is_program && let Some(seq) = self.in_flight_seq.pop_front() {
            if seq == 0 {
                self.jogs_answered.fetch_add(1, Ordering::SeqCst);
            } else {
                self.answered.fetch_max(seq, Ordering::SeqCst);
            }
        }
        let stream = match self.stream.as_mut() {
            Some(stream) if is_program => stream,
            _ => {
                if let Some(code) = error {
                    self.sink.emit(MachineEvent::Error(format!("error:{code}")));
                }
                return;
            }
        };
        stream.acked += 1;
        let (sent, acked, total) = (stream.sent, stream.acked, stream.program.len());
        let halt = error.is_some() && stream.mode == StreamMode::StopOnError;
        if let Some(code) = error {
            let message = format!("error:{code} {} {acked}", worker_text::ON_PROGRAM_LINE);
            self.sink.emit(MachineEvent::Error(message.clone()));
            if halt && self.halt.is_none() {
                self.begin_halt(message);
            }
        }
        self.sink
            .emit(MachineEvent::StreamProgress { sent, acked, total });
        if self.halt.is_none() {
            self.finish_if_done();
        }
    }

    /// Emits `StreamFinished` once every program line is acknowledged.
    pub(super) fn finish_if_done(&mut self) {
        if self
            .stream
            .as_ref()
            .is_some_and(|s| s.acked == s.program.len())
        {
            self.end_stream(true);
        }
    }

    /// Starts halting the stream: no line is fed from now on, and a feed hold is written.
    /// A failed write is retried by the reset ([`Self::drive_halt`]).
    fn begin_halt(&mut self, reason: String) {
        self.forget_lost_queries();
        let now = Instant::now();
        self.halt = Some(Halt {
            reason,
            started: now,
            deadline: now + HALT_TIMEOUT,
            next_query: now,
            stale_reports: self.unanswered_queries.len(),
        });
        match self.write(&[RealtimeCommand::FeedHold.byte()]) {
            Ok(()) => self.echo_realtime(RealtimeCommand::FeedHold.byte()),
            Err(e) => tracing::warn!("feed hold for a halted stream failed: {e}"),
        }
    }

    /// While halting: asks for status every [`HALT_QUERY_INTERVAL`], and soft-resets once
    /// the halt's deadline passes without a completed hold ([`HALT_TIMEOUT`] after the hold
    /// or the latest report that did not complete it, at most [`HALT_MAX_WAIT`] after the
    /// hold).
    fn drive_halt(&mut self) -> Result<(), String> {
        let Some(halt) = self.halt.as_mut() else {
            return Ok(());
        };
        let now = Instant::now();
        if now >= halt.deadline {
            tracing::warn!("feed hold not confirmed in time; soft-resetting");
            self.finish_halt();
            return Ok(());
        }
        if now >= halt.next_query {
            halt.next_query = now + HALT_QUERY_INTERVAL;
            self.write_status_query()?;
        }
        Ok(())
    }

    /// The hold completed (or timed out): soft-reset, drop everything in flight and end the
    /// stream, saying why.
    fn finish_halt(&mut self) {
        let Some(halt) = self.halt.take() else {
            return;
        };
        match self.write(&[RealtimeCommand::SoftReset.byte()]) {
            Ok(()) => self.echo_realtime(RealtimeCommand::SoftReset.byte()),
            Err(e) => tracing::error!("soft reset of a halted stream failed: {e}"),
        }
        // Manual lines queued during the halt were never written; the reset drops them.
        let dropped: Vec<String> = self.manual.iter().map(|(line, _)| line.clone()).collect();
        for line in dropped {
            self.sink.emit(MachineEvent::Error(format!(
                "{}: {line}",
                worker_text::DROPPED_AFTER_PROBE_ERROR
            )));
        }
        self.reset_buffers();
        self.sink.emit(MachineEvent::StreamHalted {
            reason: halt.reason,
        });
        self.end_stream(false);
    }

    /// Ends the current stream, reporting it as finished or stopped.
    pub(super) fn end_stream(&mut self, finished: bool) {
        self.paused = false;
        // A stream that ends otherwise (alarm, Stop, reset, disconnect) is not halted.
        self.halt = None;
        let Some(stream) = self.stream.take() else {
            return;
        };
        self.streaming.store(false, Ordering::SeqCst);
        let total = stream.program.len();
        self.sink.emit(if finished {
            MachineEvent::StreamFinished { lines: total }
        } else {
            MachineEvent::StreamStopped {
                acked: stream.acked,
                total,
            }
        });
    }

    /// Writes the soft reset byte and forgets every line in flight or queued, ending the
    /// stream (and any halt): the controller drops its buffered lines, and none queued
    /// before the reset may be written after it.
    fn soft_reset(&mut self) -> Result<(), String> {
        self.write(&[RealtimeCommand::SoftReset.byte()])?;
        self.echo_realtime(RealtimeCommand::SoftReset.byte());
        self.jog_outstanding = false;
        self.reset_buffers();
        self.end_stream(false);
        Ok(())
    }

    /// Forgets in-flight and queued lines after the controller was reset.
    pub(super) fn reset_buffers(&mut self) {
        self.jog_outstanding = false;
        self.query_tags.clear();
        self.unanswered_queries.clear();
        self.window = CharCountWindow::default();
        self.in_flight_is_program.clear();
        // Every jog in flight or queued is answered or dropped.
        let jogs = self
            .manual
            .iter()
            .map(|(_, seq)| *seq)
            .chain(self.in_flight_seq.iter().copied())
            .filter(|seq| *seq == 0)
            .count();
        self.jogs_answered
            .fetch_add(u64::try_from(jogs).unwrap_or(u64::MAX), Ordering::SeqCst);
        // Dropped state lines never run: a report after the reset describes the machine.
        let dropped = self.manual.drain(..).map(|(_, seq)| seq).max().unwrap_or(0);
        self.written_seq = self.written_seq.max(dropped);
        // Every line in flight or queued is answered or dropped.
        self.in_flight_seq.clear();
        self.answered.fetch_max(self.written_seq, Ordering::SeqCst);
        self.splitter.clear();
    }
}
