//! # Machine controller
//!
//! [`MachineController`] drives a GRBL-style controller from a front end without ever
//! blocking the caller on I/O. It owns one worker thread that holds the transport (a
//! [`Communicator`] from `gcodekit5-communication`); the caller talks to it over two
//! `std::sync::mpsc` channels:
//!
//! * commands (stream, pause, jog, real-time bytes, shutdown) go to the worker, and
//! * [`MachineEvent`]s come back, collected by [`MachineController::poll`].
//!
//! The worker loop drains pending commands first, so a real-time byte (feed hold,
//! cycle start, soft reset, overrides) is written as soon as the worker sees it,
//! ahead of any program line still waiting in the queue. Program lines are streamed
//! with GRBL's character-counting protocol ([`grbl_stream::CharCountWindow`]).
//!
//! No lock is held across I/O: the transport is owned by the worker alone, and the
//! only shared state is the repaint hook and a per-connection atomic "streaming" flag.
//!
//! Depends on `gcodekit5-communication` (transport, response parsing) and
//! `gcodekit5-settings` ([`ConnectionSettings`]).

pub mod grbl_stream;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use gcodekit5_communication::firmware::grbl::status_parser::{FullStatus, StatusParser};
use gcodekit5_communication::firmware::grbl::{FEED_OVERRIDE_CODES, SPINDLE_OVERRIDE_CODES};
use gcodekit5_communication::{Communicator, ConnectionParams, SerialCommunicator};
use gcodekit5_settings::{ConnectionSettings, ConnectionType};

use grbl_stream::{
    CharCountWindow, GRBL_RX_BUFFER_BYTES, GrblResponseLine, MAX_LINE_LENGTH, ResponseLineSplitter,
    classify_response, linear_jog_command,
};

/// How often the worker asks for a status report while it waits for a feed hold to complete.
const HALT_QUERY_INTERVAL: Duration = Duration::from_millis(50);
/// How long the worker waits for a feed hold to complete before it soft-resets anyway, when
/// no fresh status report says the machine is still decelerating (`Hold:1`).
const HALT_TIMEOUT: Duration = Duration::from_secs(1);
/// The longest a halt waits for a feed hold to complete, however long fresh `Hold:1`
/// reports say the machine is still decelerating; it soft-resets then.
pub const HALT_MAX_WAIT: Duration = Duration::from_secs(10);
/// How often the worker asks the controller for a status report (`?`).
const STATUS_INTERVAL: Duration = Duration::from_millis(200);
/// The most status queries whose replies are tracked; see [`WorkerState::query_tags`].
const MAX_QUERY_TAGS: usize = 3;
/// How long a `?` may stay unanswered before the worker presumes its reply lost (a reply
/// dropped by a reset or the transport); see [`WorkerState::unanswered_queries`].
const QUERY_REPLY_TIMEOUT: Duration = Duration::from_secs(2);
/// The most unanswered `?`s the worker remembers; see [`WorkerState::unanswered_queries`].
const MAX_UNANSWERED_QUERIES: usize = 64;
/// How long an idle worker waits for a command before reading the transport again.
const IDLE_WAIT: Duration = Duration::from_millis(2);

/// Errors returned by [`MachineController`] calls.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MachineError {
    /// `connect` was called while a connection is open.
    #[error("already connected")]
    AlreadyConnected,
    /// The call needs an open connection.
    #[error("not connected")]
    NotConnected,
    /// `start_stream` was called while a program is streaming.
    #[error("a program is already streaming")]
    StreamInProgress,
    /// The connection settings are not usable.
    #[error("invalid connection settings: {0}")]
    InvalidSettings(String),
    /// The worker thread could not be started.
    #[error("cannot start the machine worker: {0}")]
    WorkerStart(String),
    /// A command line was empty.
    #[error("the command line is empty")]
    EmptyLine,
    /// A command line held a byte outside printable ASCII (a line break, a control byte, a
    /// non-ASCII character); GRBL acts on some of them as real-time commands.
    #[error("the command line holds the byte 0x{0:02X}, which is not printable ASCII")]
    NonPrintable(u8),
    /// A command line held `!`, `~` or `?`, which GRBL acts on as real-time commands
    /// wherever they appear.
    #[error("the command line holds '{0}', a real-time command character")]
    RealtimeCharacter(char),
    /// A command line was longer than the controller's line buffer.
    #[error("the command line has {length} characters; the maximum is {max}")]
    LineTooLong {
        /// The line's length.
        length: usize,
        /// The longest line accepted.
        max: usize,
    },
}

/// A machine axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// Linear X.
    X,
    /// Linear Y.
    Y,
    /// Linear Z.
    Z,
    /// Rotary A.
    A,
    /// Rotary B.
    B,
    /// Rotary C.
    C,
}

impl Axis {
    /// The axis letter used in G-code.
    pub fn letter(self) -> char {
        match self {
            Axis::X => 'X',
            Axis::Y => 'Y',
            Axis::Z => 'Z',
            Axis::A => 'A',
            Axis::B => 'B',
            Axis::C => 'C',
        }
    }
}

/// A GRBL real-time command: one byte that the controller acts on immediately,
/// outside its line buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RealtimeCommand {
    /// `?` status report request.
    StatusQuery,
    /// `!` feed hold.
    FeedHold,
    /// `~` cycle start / resume.
    CycleStart,
    /// `0x18` (Ctrl-X) soft reset.
    SoftReset,
    /// `0x85` jog cancel.
    JogCancel,
    /// `0x90` feed override back to 100%.
    FeedOverrideReset,
    /// `0x91` feed override +10%.
    FeedOverridePlus10,
    /// `0x92` feed override -10%.
    FeedOverrideMinus10,
    /// `0x93` feed override +1%.
    FeedOverridePlus1,
    /// `0x94` feed override -1%.
    FeedOverrideMinus1,
    /// `0x95` rapid override 100%.
    RapidOverride100,
    /// `0x96` rapid override 50%.
    RapidOverride50,
    /// `0x97` rapid override 25%.
    RapidOverride25,
    /// `0x99` spindle override back to 100%.
    SpindleOverrideReset,
    /// `0x9A` spindle override +10%.
    SpindleOverridePlus10,
    /// `0x9B` spindle override -10%.
    SpindleOverrideMinus10,
    /// `0x9C` spindle override +1%.
    SpindleOverridePlus1,
    /// `0x9D` spindle override -1%.
    SpindleOverrideMinus1,
    /// `0x9E` toggle spindle stop (during feed hold).
    SpindleStop,
}

impl RealtimeCommand {
    /// The byte written to the controller.
    pub fn byte(self) -> u8 {
        match self {
            RealtimeCommand::StatusQuery => b'?',
            RealtimeCommand::FeedHold => b'!',
            RealtimeCommand::CycleStart => b'~',
            RealtimeCommand::SoftReset => 0x18,
            RealtimeCommand::JogCancel => 0x85,
            RealtimeCommand::FeedOverrideReset => FEED_OVERRIDE_CODES.reset,
            RealtimeCommand::FeedOverridePlus10 => FEED_OVERRIDE_CODES.plus_10,
            RealtimeCommand::FeedOverrideMinus10 => FEED_OVERRIDE_CODES.minus_10,
            RealtimeCommand::FeedOverridePlus1 => FEED_OVERRIDE_CODES.plus_1,
            RealtimeCommand::FeedOverrideMinus1 => FEED_OVERRIDE_CODES.minus_1,
            RealtimeCommand::RapidOverride100 => 0x95,
            RealtimeCommand::RapidOverride50 => 0x96,
            RealtimeCommand::RapidOverride25 => 0x97,
            RealtimeCommand::SpindleOverrideReset => SPINDLE_OVERRIDE_CODES.reset,
            RealtimeCommand::SpindleOverridePlus10 => SPINDLE_OVERRIDE_CODES.plus_10,
            RealtimeCommand::SpindleOverrideMinus10 => SPINDLE_OVERRIDE_CODES.minus_10,
            RealtimeCommand::SpindleOverridePlus1 => SPINDLE_OVERRIDE_CODES.plus_1,
            RealtimeCommand::SpindleOverrideMinus1 => SPINDLE_OVERRIDE_CODES.minus_1,
            RealtimeCommand::SpindleStop => 0x9E,
        }
    }
}

/// Something that happened on the machine connection, reported by
/// [`MachineController::poll`].
#[derive(Debug, Clone, PartialEq)]
pub enum MachineEvent {
    /// The transport opened.
    Connected,
    /// The transport closed (on request or after an error).
    Disconnected,
    /// A parsed `<...>` status report.
    StatusReport(Box<FullStatus>),
    /// A response line that is not an ack or status report (`[MSG:...]`, banner, ...).
    Received(String),
    /// Stream progress: lines written and lines acknowledged out of `total`.
    StreamProgress {
        /// Program lines written to the controller.
        sent: usize,
        /// Program lines acknowledged (`ok` or `error:N`).
        acked: usize,
        /// Program length.
        total: usize,
    },
    /// A [`StreamMode::StopOnError`] stream was halted: the controller refused a line, so the
    /// worker stopped feeding, feed-held and soft-reset it. Reported just before the
    /// stream's [`MachineEvent::StreamStopped`].
    StreamHalted {
        /// Why (the controller's reply).
        reason: String,
    },
    /// Every program line was acknowledged.
    StreamFinished {
        /// Program length.
        lines: usize,
    },
    /// The stream ended early (stop, alarm, reset or invalid line).
    StreamStopped {
        /// Program lines acknowledged before it ended.
        acked: usize,
        /// Program length.
        total: usize,
    },
    /// A manual command line (`$H`, `$X`, an MDI line, a `$J` jog) or a real-time command
    /// (shown as its label, e.g. `0x18 (Reset)`) was written to the controller. Program
    /// lines of a stream are not reported (as in the GTK console, which did not echo them).
    Sent(String),
    /// An error from the controller or the transport.
    Error(String),
}

/// How a stream reacts to an `error:N` reply to one of its lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamMode {
    /// Report the error and keep streaming (a job: GRBL skips the refused line).
    #[default]
    Normal,
    /// Stop at once (a probe routine, whose later lines are relative moves that must not run
    /// after a refused probe line): stop feeding lines, feed hold (`!`), wait up to a second
    /// for the hold to complete (`Hold:0` or `Idle`, polling `?`), then soft reset (0x18),
    /// which keeps the machine position once the motion has stopped. Reported as
    /// [`MachineEvent::StreamHalted`] then [`MachineEvent::StreamStopped`].
    StopOnError,
}

/// How the console names a real-time byte that is worth echoing, or `None` for the ones
/// that are not (`?` polls, overrides).
///
/// # Arguments
/// * `byte` - The real-time byte written.
pub fn realtime_label(byte: u8) -> Option<&'static str> {
    match byte {
        b'!' => Some("! (Feed hold)"),
        b'~' => Some("~ (Cycle start)"),
        0x18 => Some("0x18 (Reset)"),
        0x85 => Some("0x85 (Jog cancel)"),
        _ => None,
    }
}

/// Builds a fresh, unconnected transport for each [`MachineController::connect`].
pub type TransportFactory = Box<dyn Fn() -> Box<dyn Communicator> + Send>;

/// Called by the worker after it emits an event, so a UI can schedule a repaint.
pub type RepaintHook = Box<dyn Fn() + Send + Sync>;

type SharedHook = Arc<Mutex<Option<RepaintHook>>>;

/// Requests sent from the controller to its worker.
#[derive(Debug)]
enum Command {
    Stream(Arc<[String]>, StreamMode),
    Pause,
    Resume,
    Stop,
    Line(String),
    /// A command line that may change the machine state, with its sequence number.
    StateLine(String, u64),
    Realtime(u8),
    Shutdown,
}

/// The running worker thread and its channels.
struct Worker {
    commands: Sender<Command>,
    events: Receiver<MachineEvent>,
    handle: JoinHandle<()>,
    /// Set by the worker once the transport is open (just before `Connected`).
    ready: Arc<AtomicBool>,
}

/// Non-blocking front-end handle to a machine connection.
///
/// Every method returns without waiting for I/O. Call [`poll`](Self::poll) regularly
/// (every frame) to collect [`MachineEvent`]s. Dropping the controller disconnects and
/// joins the worker thread, except while a connect is still in progress: that worker is
/// told to shut down and left to finish on its own, so the caller never waits for a
/// connect timeout.
pub struct MachineController {
    factory: TransportFactory,
    repaint: SharedHook,
    streaming: Arc<AtomicBool>,
    /// Sequence number of the last command line sent with [`send_line`](Self::send_line).
    requested: u64,
    /// The highest such sequence number written before the latest status report.
    fresh: Arc<AtomicU64>,
    worker: Option<Worker>,
    /// Workers told to shut down while still connecting; reaped once finished.
    closing: Vec<JoinHandle<()>>,
    pending: Vec<MachineEvent>,
}

impl std::fmt::Debug for MachineController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MachineController")
            .field("connected", &self.is_worker_running())
            .field("streaming", &self.streaming.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

impl Default for MachineController {
    fn default() -> Self {
        Self::new()
    }
}

impl MachineController {
    /// Creates a controller that connects over serial or TCP with
    /// [`SerialCommunicator`].
    pub fn new() -> Self {
        Self::with_transport(Box::new(|| {
            Box::new(SerialCommunicator::new()) as Box<dyn Communicator>
        }))
    }

    /// Creates a controller whose transports come from `factory` (one per connect).
    ///
    /// # Arguments
    /// * `factory` - Returns a fresh, unconnected transport.
    pub fn with_transport(factory: TransportFactory) -> Self {
        Self {
            factory,
            repaint: Arc::new(Mutex::new(None)),
            streaming: Arc::new(AtomicBool::new(false)),
            requested: 0,
            fresh: Arc::new(AtomicU64::new(0)),
            worker: None,
            closing: Vec::new(),
            pending: Vec::new(),
        }
    }

    /// Starts connecting with `cfg` on a new worker thread.
    ///
    /// Returns as soon as the worker is started; [`MachineEvent::Connected`] or
    /// [`MachineEvent::Error`] followed by [`MachineEvent::Disconnected`] reports the
    /// outcome through [`poll`](Self::poll).
    ///
    /// # Errors
    /// [`MachineError::AlreadyConnected`] if a connection is open, or
    /// [`MachineError::InvalidSettings`] if `cfg` cannot describe a connection (including
    /// an empty or unresolved `"Auto"` port), or
    /// [`MachineError::WorkerStart`] if the worker thread cannot be spawned.
    pub fn connect(&mut self, cfg: &ConnectionSettings) -> Result<(), MachineError> {
        if self.is_worker_running() {
            return Err(MachineError::AlreadyConnected);
        }
        self.reap_worker();
        self.reap_closing();
        let port = cfg.port.trim();
        if port.is_empty() || port.eq_ignore_ascii_case("auto") {
            return Err(MachineError::InvalidSettings(format!(
                "port '{port}' must name a device or host"
            )));
        }
        let params = connection_params(cfg);
        params
            .validate()
            .map_err(|e| MachineError::InvalidSettings(e.to_string()))?;

        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (ev_tx, ev_rx) = mpsc::channel();
        let ready = Arc::new(AtomicBool::new(false));
        // A fresh flag per connection: a detached worker from an earlier session keeps
        // its own clone, so it can never clear this session's streaming state.
        self.streaming = Arc::new(AtomicBool::new(false));
        self.requested = 0;
        self.fresh = Arc::new(AtomicU64::new(0));
        let state = WorkerState::new(
            (self.factory)(),
            cmd_rx,
            EventSink {
                events: ev_tx,
                repaint: self.repaint.clone(),
            },
            self.streaming.clone(),
            ready.clone(),
            self.fresh.clone(),
        );
        let handle = std::thread::Builder::new()
            .name("machine-controller".to_string())
            .spawn(move || state.run(&params))
            .map_err(|e| MachineError::WorkerStart(e.to_string()))?;
        self.worker = Some(Worker {
            commands: cmd_tx,
            events: ev_rx,
            handle,
            ready,
        });
        Ok(())
    }

    /// Closes the connection without waiting on I/O.
    ///
    /// A connected worker is joined (it exits within one loop iteration), and the
    /// events it emitted on the way out (including [`MachineEvent::Disconnected`]) stay
    /// available to [`poll`](Self::poll). A worker still inside the transport's
    /// `connect` is told to shut down and detached: [`MachineEvent::Disconnected`] is
    /// reported at once, its later events are discarded, and [`poll`](Self::poll) or the
    /// next [`connect`](Self::connect) joins it once it has finished.
    pub fn disconnect(&mut self) {
        let Some(worker) = self.worker.as_ref() else {
            return;
        };
        // A send error means the worker already exited; joining is still right.
        let _ = worker.commands.send(Command::Shutdown);
        if worker.ready.load(Ordering::SeqCst) || worker.handle.is_finished() {
            self.reap_worker();
            return;
        }
        if let Some(worker) = self.worker.take() {
            // Dropping the channels makes the worker exit as soon as connect returns.
            self.closing.push(worker.handle);
            self.pending.push(MachineEvent::Disconnected);
            self.streaming.store(false, Ordering::SeqCst);
        }
    }

    /// Queues `program` for streaming and returns immediately.
    ///
    /// Lines are sent as given (trimmed), so the program must already be prepared
    /// with [`grbl_stream::prepare_program`] (comments removed, length checked).
    ///
    /// # Errors
    /// [`MachineError::NotConnected`] without a connection, or
    /// [`MachineError::StreamInProgress`] while another program streams.
    pub fn start_stream(&mut self, program: Arc<[String]>) -> Result<(), MachineError> {
        self.start_stream_with(program, StreamMode::Normal)
    }

    /// Queues `program` for streaming in `mode` and returns immediately (see
    /// [`start_stream`](Self::start_stream)).
    ///
    /// # Errors
    /// As [`start_stream`](Self::start_stream).
    pub fn start_stream_with(
        &mut self,
        program: Arc<[String]>,
        mode: StreamMode,
    ) -> Result<(), MachineError> {
        if !self.is_worker_running() {
            return Err(MachineError::NotConnected);
        }
        if self.streaming.swap(true, Ordering::SeqCst) {
            return Err(MachineError::StreamInProgress);
        }
        if let Err(e) = self.send(Command::Stream(program, mode)) {
            self.streaming.store(false, Ordering::SeqCst);
            return Err(e);
        }
        Ok(())
    }

    /// Feed hold, and stop feeding program lines until [`resume`](Self::resume).
    pub fn pause(&mut self) {
        self.send_or_report(Command::Pause);
    }

    /// Cycle start, and continue feeding program lines.
    pub fn resume(&mut self) {
        self.send_or_report(Command::Resume);
    }

    /// Soft-resets the controller and drops the rest of the program.
    pub fn stop(&mut self) {
        self.send_or_report(Command::Stop);
    }

    /// Jogs `axis` by `distance_mm` (relative) at `feed_mm_min` with a `$J=` command.
    ///
    /// Refused (reported as [`MachineEvent::Error`]) for a non-finite distance, a feed
    /// that is not positive, or while a program streams.
    pub fn jog(&mut self, axis: Axis, distance_mm: f32, feed_mm_min: f32) {
        if !distance_mm.is_finite() || !(feed_mm_min.is_finite() && feed_mm_min > 0.0) {
            self.pending.push(MachineEvent::Error(format!(
                "jog ignored: invalid distance {distance_mm} or feed {feed_mm_min}"
            )));
            return;
        }
        if self.streaming.load(Ordering::SeqCst) {
            self.pending.push(MachineEvent::Error(
                "jog ignored: a program is streaming".to_string(),
            ));
            return;
        }
        let line = linear_jog_command(axis.letter(), distance_mm, feed_mm_min);
        self.send_or_report(Command::Line(line));
    }

    /// Queues one command line (`$H`, `$X`, an MDI line), trimmed of spaces, behind any
    /// manual line still waiting.
    ///
    /// # Errors
    /// [`MachineError::EmptyLine`], [`MachineError::NonPrintable`] for a byte outside
    /// printable ASCII (0x20-0x7E), [`MachineError::RealtimeCharacter`] for `!`, `~` or `?`,
    /// [`MachineError::LineTooLong`] for a line that, with its newline, does not fit the
    /// controller's RX buffer ([`GRBL_RX_BUFFER_BYTES`]; [`MAX_LINE_LENGTH`] is higher),
    /// [`MachineError::NotConnected`] without a connection, or
    /// [`MachineError::StreamInProgress`] while a program streams.
    pub fn send_line(&mut self, line: &str) -> Result<(), MachineError> {
        let line = line.trim_matches(' ');
        if line.is_empty() {
            return Err(MachineError::EmptyLine);
        }
        if let Some(&byte) = line.as_bytes().iter().find(|b| !(0x20..=0x7e).contains(*b)) {
            return Err(MachineError::NonPrintable(byte));
        }
        if let Some(c) = line.chars().find(|c| matches!(c, '!' | '~' | '?')) {
            return Err(MachineError::RealtimeCharacter(c));
        }
        // The line and its newline must fit the controller's RX buffer at once; the worker
        // would refuse it otherwise.
        let max = MAX_LINE_LENGTH.min(GRBL_RX_BUFFER_BYTES - 1);
        if line.len() > max {
            return Err(MachineError::LineTooLong {
                length: line.len(),
                max,
            });
        }
        if !self.is_worker_running() {
            return Err(MachineError::NotConnected);
        }
        if self.streaming.load(Ordering::SeqCst) {
            return Err(MachineError::StreamInProgress);
        }
        self.requested += 1;
        self.send(Command::StateLine(line.to_string(), self.requested))
    }

    /// Queues `line` for the worker as [`send_line`](Self::send_line) would, without its
    /// checks (a stream in progress included), so tests can reach the worker's own guards.
    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn queue_line_unchecked(&mut self, line: &str) {
        self.requested += 1;
        self.send_or_report(Command::StateLine(line.to_string(), self.requested));
    }

    /// Whether the latest status report was read after every line sent with
    /// [`send_line`](Self::send_line) had been written. A line such as `$H` changes the
    /// machine state, so until then the last report may describe the machine before it (Idle
    /// before homing): a front end must not jog on it. True before any line is sent.
    pub fn report_fresh(&self) -> bool {
        self.fresh.load(Ordering::SeqCst) >= self.requested
    }

    /// Whether a program is streaming: from a successful
    /// [`start_stream`](Self::start_stream) until it finishes or stops.
    pub fn is_streaming(&self) -> bool {
        self.streaming.load(Ordering::SeqCst)
    }

    /// Writes a real-time byte ahead of any queued program line.
    pub fn realtime(&mut self, cmd: RealtimeCommand) {
        self.send_or_report(Command::Realtime(cmd.byte()));
    }

    /// Returns every event since the last call, oldest first. Never blocks.
    pub fn poll(&mut self) -> Vec<MachineEvent> {
        self.reap_closing();
        let mut events = std::mem::take(&mut self.pending);
        if let Some(worker) = self.worker.as_ref() {
            events.extend(worker.events.try_iter());
        }
        events
    }

    /// Sets the function the worker calls after emitting an event (for example
    /// `egui::Context::request_repaint`). Replaces any previous hook.
    pub fn set_repaint_hook(&mut self, hook: RepaintHook) {
        *self.repaint.lock().unwrap_or_else(PoisonError::into_inner) = Some(hook);
    }

    /// True while the worker thread is alive (connecting or connected).
    fn is_worker_running(&self) -> bool {
        self.worker
            .as_ref()
            .is_some_and(|worker| !worker.handle.is_finished())
    }

    fn send(&self, cmd: Command) -> Result<(), MachineError> {
        let worker = self.worker.as_ref().ok_or(MachineError::NotConnected)?;
        worker
            .commands
            .send(cmd)
            .map_err(|_| MachineError::NotConnected)
    }

    fn send_or_report(&mut self, cmd: Command) {
        if let Err(e) = self.send(cmd) {
            self.pending.push(MachineEvent::Error(e.to_string()));
        }
    }

    /// Joins a worker (which must be exiting) and keeps its remaining events.
    fn reap_worker(&mut self) {
        let Some(worker) = self.worker.take() else {
            return;
        };
        let joined = worker.handle.join();
        self.pending.extend(worker.events.try_iter());
        if joined.is_err() {
            self.pending.push(MachineEvent::Error(
                "machine worker thread panicked".to_string(),
            ));
            self.pending.push(MachineEvent::Disconnected);
        }
        self.streaming.store(false, Ordering::SeqCst);
    }
}

impl MachineController {
    /// Joins the detached workers that have finished; leaves the others running.
    fn reap_closing(&mut self) {
        let (done, running): (Vec<_>, Vec<_>) = std::mem::take(&mut self.closing)
            .into_iter()
            .partition(JoinHandle::is_finished);
        self.closing = running;
        for handle in done {
            if handle.join().is_err() {
                tracing::warn!("detached machine worker thread panicked");
            }
        }
    }
}

impl Drop for MachineController {
    fn drop(&mut self) {
        self.disconnect();
    }
}

/// Maps the stored connection settings to transport parameters.
fn connection_params(cfg: &ConnectionSettings) -> ConnectionParams {
    let mut params = match cfg.connection_type {
        ConnectionType::Serial => ConnectionParams::serial(&cfg.port, cfg.baud_rate),
        ConnectionType::Tcp => ConnectionParams::tcp(&cfg.port, cfg.tcp_port),
        ConnectionType::WebSocket => ConnectionParams::websocket(&cfg.port, cfg.tcp_port),
    };
    params.timeout_ms = cfg.timeout_ms;
    params.auto_reconnect = cfg.auto_reconnect;
    params
}

/// Sends events to the controller and pokes the repaint hook.
struct EventSink {
    events: Sender<MachineEvent>,
    repaint: SharedHook,
}

impl EventSink {
    fn emit(&self, event: MachineEvent) {
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
struct Stream {
    program: Arc<[String]>,
    sent: usize,
    acked: usize,
    mode: StreamMode,
}

/// A [`StreamMode::StopOnError`] stream being halted: feed hold sent, waiting for it to
/// complete before the soft reset.
struct Halt {
    /// Why (the controller's reply).
    reason: String,
    /// When the feed hold was written.
    started: Instant,
    /// When to soft-reset without a completed hold: [`HALT_TIMEOUT`] after the hold or the
    /// latest fresh `Hold:1` report, never later than [`HALT_MAX_WAIT`] after the hold.
    deadline: Instant,
    /// When to ask for the next status report.
    next_query: Instant,
    /// Status reports still owed to `?`s written before the hold; they may describe the
    /// machine before it, so they do not count as a completed hold.
    stale_reports: usize,
}

/// Everything the worker thread owns.
struct WorkerState {
    comm: Box<dyn Communicator>,
    commands: Receiver<Command>,
    sink: EventSink,
    streaming: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
    window: CharCountWindow,
    /// One entry per line in `window`: true for a program line, false for a manual one.
    in_flight_is_program: VecDeque<bool>,
    splitter: ResponseLineSplitter,
    /// Manual lines waiting for room, each with its state-line sequence number (0 for a jog).
    manual: VecDeque<(String, u64)>,
    stream: Option<Stream>,
    paused: bool,
    /// The highest state-line sequence number written (or dropped, never to run).
    written_seq: u64,
    /// Shared with the controller: the `written_seq` tag of the latest status report.
    fresh: Arc<AtomicU64>,
    /// One tag per `?` written and not yet answered, oldest first: `written_seq` when it was
    /// written. A status report answers the oldest `?`, so its tag says which state lines it
    /// can describe; a report to a `?` written before `$H` does not count as after `$H`.
    /// Capped at [`MAX_QUERY_TAGS`]: on overflow the two oldest tags merge into the lower one,
    /// so a lagging queue only ever under-states freshness.
    query_tags: VecDeque<u64>,
    /// When each `?` not yet answered was written, oldest first: how many reports a halt must
    /// skip as possibly describing the machine before the hold. A status report pops the
    /// oldest entry (nothing when none is left, so an unsolicited report cannot underflow
    /// it). The count corrects itself and stays bounded: an entry older than
    /// [`QUERY_REPLY_TIMEOUT`] is presumed lost and dropped, at most
    /// [`MAX_UNANSWERED_QUERIES`] are kept (the oldest goes first), and a reset clears them.
    /// An unsolicited report (auto-reporting firmware) can still answer for a `?` and make
    /// the count low until that `?`'s own reply arrives; the next `?`s set it right again.
    unanswered_queries: VecDeque<Instant>,
    /// A `$J` jog was queued and is not known to be over: set when it is queued and again when
    /// it is written, cleared by jog cancel (0x85, which also drops queued jogs), soft reset
    /// (0x18), or an `Idle` report after a `Jog` one received since it was written.
    jog_outstanding: bool,
    /// A `Jog` state was reported since the outstanding jog was written.
    jog_seen: bool,
    /// The stream being halted after an error, see [`StreamMode::StopOnError`].
    halt: Option<Halt>,
}

/// Why the worker loop ended.
enum Exit {
    Shutdown,
    TransportError(String),
}

impl WorkerState {
    fn new(
        comm: Box<dyn Communicator>,
        commands: Receiver<Command>,
        sink: EventSink,
        streaming: Arc<AtomicBool>,
        ready: Arc<AtomicBool>,
        fresh: Arc<AtomicU64>,
    ) -> Self {
        Self {
            comm,
            commands,
            sink,
            streaming,
            ready,
            window: CharCountWindow::default(),
            in_flight_is_program: VecDeque::new(),
            splitter: ResponseLineSplitter::new(),
            manual: VecDeque::new(),
            stream: None,
            paused: false,
            written_seq: 0,
            fresh,
            query_tags: VecDeque::new(),
            unanswered_queries: VecDeque::new(),
            jog_outstanding: false,
            jog_seen: false,
            halt: None,
        }
    }

    /// Connects, runs the I/O loop until shutdown or a transport error, then
    /// disconnects.
    fn run(mut self, params: &ConnectionParams) {
        if let Err(e) = self.comm.connect(params) {
            self.sink
                .emit(MachineEvent::Error(format!("connect failed: {e}")));
            // A stream queued while connecting never started.
            self.streaming.store(false, Ordering::SeqCst);
            self.sink.emit(MachineEvent::Disconnected);
            return;
        }
        self.ready.store(true, Ordering::SeqCst);
        self.sink.emit(MachineEvent::Connected);

        let exit = self.io_loop();
        if let Exit::TransportError(msg) = exit {
            self.sink.emit(MachineEvent::Error(msg));
        } else if self.jog_outstanding {
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

    fn io_loop(&mut self) -> Exit {
        let mut next_status = Instant::now();
        let mut idle = false;
        loop {
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
            if let Err(e) = self.feed() {
                return Exit::TransportError(e);
            }

            // 2b. A halting stream: poll the hold, reset once it completes or times out.
            if let Err(e) = self.drive_halt() {
                return Exit::TransportError(e);
            }

            // 3. Periodic status request.
            let now = Instant::now();
            if now >= next_status {
                if let Err(e) = self.write_status_query() {
                    return Exit::TransportError(e);
                }
                next_status = now + STATUS_INTERVAL;
            }

            // 4. Read and handle responses.
            match self.comm.receive() {
                Ok(data) => {
                    idle = data.is_empty();
                    for line in self.splitter.push(&data) {
                        self.handle_response(&line);
                    }
                }
                Err(e) => return Exit::TransportError(format!("read failed: {e}")),
            }
        }
    }

    fn write(&mut self, data: &[u8]) -> Result<(), String> {
        self.comm
            .send(data)
            .map(|_| ())
            .map_err(|e| format!("write failed: {e}"))
    }

    fn handle_command(&mut self, cmd: Command) -> Result<(), String> {
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
            Command::Realtime(byte) => {
                self.write(&[byte])?;
                self.echo_realtime(byte);
                if byte == RealtimeCommand::JogCancel.byte()
                    || byte == RealtimeCommand::SoftReset.byte()
                {
                    // A jog still waiting in the queue must not start after the cancel or
                    // the reset.
                    self.manual.retain(|(line, _)| !line.starts_with("$J="));
                    self.jog_outstanding = false;
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
            Command::Stop => {
                self.write(&[RealtimeCommand::SoftReset.byte()])?;
                self.echo_realtime(RealtimeCommand::SoftReset.byte());
                self.jog_outstanding = false;
                self.reset_buffers();
                self.end_stream(false);
            }
            Command::Line(line) => self.queue_manual(line, 0),
            Command::StateLine(line, seq) => self.queue_manual(line, seq),
            Command::Stream(program, mode) => {
                self.paused = false;
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
            "cycle start ignored: probe stream halting".to_string(),
        ));
    }

    /// Reports a written real-time byte as [`MachineEvent::Sent`] when it has a label.
    fn echo_realtime(&self, byte: u8) {
        if let Some(label) = realtime_label(byte) {
            self.sink.emit(MachineEvent::Sent(label.to_string()));
        }
    }

    /// Writes a `?` and tags it with the state lines written so far.
    fn write_status_query(&mut self) -> Result<(), String> {
        self.write(&[RealtimeCommand::StatusQuery.byte()])?;
        if self.query_tags.len() >= MAX_QUERY_TAGS
            && let Some(oldest) = self.query_tags.pop_front()
            && let Some(next) = self.query_tags.front_mut()
        {
            *next = (*next).min(oldest);
        }
        self.query_tags.push_back(self.written_seq);
        self.forget_lost_queries();
        if self.unanswered_queries.len() >= MAX_UNANSWERED_QUERIES {
            self.unanswered_queries.pop_front();
        }
        self.unanswered_queries.push_back(Instant::now());
        Ok(())
    }

    /// Drops the unanswered `?`s older than [`QUERY_REPLY_TIMEOUT`]: their replies are
    /// presumed lost.
    fn forget_lost_queries(&mut self) {
        let now = Instant::now();
        while self
            .unanswered_queries
            .front()
            .is_some_and(|at| now.duration_since(*at) >= QUERY_REPLY_TIMEOUT)
        {
            self.unanswered_queries.pop_front();
        }
    }

    /// Queues a manual line (`seq` > 0 for a state line), or reports one too long for the
    /// controller's buffer, which then never runs. (`send_line` already refuses such lines;
    /// this is defence in depth.)
    fn queue_manual(&mut self, line: String, seq: u64) {
        if !self.window.fits_at_all(&line) {
            self.sink.emit(MachineEvent::Error(format!(
                "line too long for the controller buffer: {line}"
            )));
            // The refused line never runs, but a lower state line still queued must be
            // written first: hand the sequence number to the last queued state line.
            match self.manual.iter_mut().rev().find(|(_, s)| *s > 0) {
                Some((_, queued)) => *queued = (*queued).max(seq),
                None => self.written_seq = self.written_seq.max(seq),
            }
            return;
        }
        if line.starts_with("$J=") {
            self.jog_outstanding = true;
            self.jog_seen = false;
        }
        self.manual.push_back((line, seq));
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
                    "line {} is too long for the controller buffer",
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
        match classify_response(line) {
            GrblResponseLine::Ok => self.on_ack(None),
            GrblResponseLine::Error(code) => self.on_ack(Some(code)),
            GrblResponseLine::Alarm(code) => {
                self.sink.emit(MachineEvent::Error(format!("ALARM:{code}")));
                self.end_stream(false);
            }
            GrblResponseLine::Status(report) => {
                let status = StatusParser::parse_full(&report);
                self.unanswered_queries.pop_front();
                let state = status.machine_state.as_deref();
                let held = state.is_some_and(|s| s.starts_with("Hold:0") || s.starts_with("Idle"));
                let decelerating = state.is_some_and(|s| s.starts_with("Hold:1"));
                if let Some(halt) = self.halt.as_mut() {
                    if halt.stale_reports > 0 {
                        halt.stale_reports -= 1;
                    } else if held {
                        self.finish_halt();
                    } else if decelerating {
                        // Still slowing down: a reset now would come in motion.
                        let cap = halt.started + HALT_MAX_WAIT;
                        halt.deadline = (Instant::now() + HALT_TIMEOUT).min(cap);
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
                // Report the banner before the stream end it causes, so a consumer
                // that stops at `StreamStopped` has already seen why.
                self.sink.emit(MachineEvent::Received(banner));
                // The controller restarted: nothing in flight will be acknowledged.
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
        let is_program = self.in_flight_is_program.pop_front().unwrap_or(false);
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
            self.sink.emit(MachineEvent::Error(format!(
                "error:{code} on program line {acked}"
            )));
            if halt && self.halt.is_none() {
                self.begin_halt(format!("error:{code} on program line {acked}"));
            }
        }
        self.sink
            .emit(MachineEvent::StreamProgress { sent, acked, total });
        if self.halt.is_none() {
            self.finish_if_done();
        }
    }

    /// Emits `StreamFinished` once every program line is acknowledged.
    fn finish_if_done(&mut self) {
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
    /// or the latest fresh `Hold:1`, at most [`HALT_MAX_WAIT`] after the hold).
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
                "dropped: controller reset after probe error: {line}"
            )));
        }
        self.reset_buffers();
        self.sink.emit(MachineEvent::StreamHalted {
            reason: halt.reason,
        });
        self.end_stream(false);
    }

    /// Ends the current stream, reporting it as finished or stopped.
    fn end_stream(&mut self, finished: bool) {
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

    /// Forgets in-flight and queued lines after the controller was reset.
    fn reset_buffers(&mut self) {
        self.jog_outstanding = false;
        self.query_tags.clear();
        self.unanswered_queries.clear();
        self.window = CharCountWindow::default();
        self.in_flight_is_program.clear();
        // Dropped state lines never run: a report after the reset describes the machine.
        let dropped = self.manual.drain(..).map(|(_, seq)| seq).max().unwrap_or(0);
        self.written_seq = self.written_seq.max(dropped);
        self.splitter.clear();
    }
}
