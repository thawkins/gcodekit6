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
//! On a Repetier connection ([`MachineController::connect_with`] with
//! [`ControllerProtocol::Repetier`]) the worker speaks Repetier's flow-control protocol
//! instead, as the GTK front end's `RepetierSender` did: every line is numbered and
//! checksummed by `gcodekit5-communication`'s [`RepetierProcessor`], one command is in flight
//! at a time, a `Resend:N` replays the numbered lines from `N`, and no GRBL real-time byte or
//! `$` command is ever written. The emergency stop is `M112`.
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
use gcodekit5_communication::firmware::protocol_registry::detect_protocol;
use gcodekit5_communication::firmware::repetier::RepetierProcessor;
use gcodekit5_communication::firmware::repetier::processor::RepetierEvent;
use gcodekit5_communication::firmware::{ControllerProtocol, protocol_profile};
use gcodekit5_communication::{Communicator, ConnectionParams, SerialCommunicator};
use gcodekit5_settings::{ConnectionSettings, ConnectionType};

use grbl_stream::{
    CharCountWindow, GRBL_RX_BUFFER_BYTES, GrblResponseLine, LINEAR_JOG_DECIMALS, MAX_LINE_LENGTH,
    ROTARY_JOG_DECIMALS, ResponseLineSplitter, classify_response, jog_writes_zero,
    linear_jog_command, rotary_jog_line,
};

/// How often the worker asks for a status report while it waits for a feed hold to complete.
const HALT_QUERY_INTERVAL: Duration = Duration::from_millis(50);
/// How long the worker waits for a feed hold to complete before it soft-resets anyway, after
/// the hold or the latest status report that did not complete it.
const HALT_TIMEOUT: Duration = Duration::from_secs(1);
/// The longest a halt waits for a feed hold to complete, however long fresh `Hold:1`
/// reports say the machine is still decelerating; it soft-resets then.
pub const HALT_MAX_WAIT: Duration = Duration::from_secs(10);
/// How often the worker asks the controller for a status report (`?`).
const STATUS_INTERVAL: Duration = Duration::from_millis(200);
/// The most status queries whose replies are tracked; see [`WorkerState::query_tags`].
const MAX_QUERY_TAGS: usize = 3;
/// The shortest time a `?` may stay unanswered before the worker presumes its reply lost (a
/// reply dropped by a reset or the transport); on a slower link the time follows the
/// observed reply latency, see [`WorkerState::lost_reply_timeout`].
const QUERY_REPLY_TIMEOUT: Duration = Duration::from_secs(2);
/// The most unanswered `?`s the worker remembers; see [`WorkerState::unanswered_queries`].
const MAX_UNANSWERED_QUERIES: usize = 256;
/// How many times in a row the worker replays the same line on a Repetier `Resend:` before it
/// stops the stream: a line the firmware keeps refusing (one longer than its buffer, a noisy
/// link) would otherwise be replayed for ever.
const MAX_REPETIER_RESENDS: u32 = 5;
/// How long an idle worker waits for a command before reading the transport again.
const IDLE_WAIT: Duration = Duration::from_millis(2);

/// The English texts of the messages the controller and its worker word themselves in
/// [`MachineEvent::Error`] and [`MachineEvent::StreamHalted`], and of the fixed
/// [`MachineError`]s (which also reach the UI as such events). Each is a msgid: this crate
/// stays toolkit-free and emits English, and a front end translates at display time. A
/// message is a text alone, or a text followed by `": "` and a detail (a line, a transport
/// error); [`ON_PROGRAM_LINE`](worker_text::ON_PROGRAM_LINE) joins a controller reply
/// (`error:N`) to its line number. The controller's own replies are protocol, not texts.
pub mod worker_text {
    /// The transport could not be opened; followed by the transport's error.
    pub const CONNECT_FAILED: &str = "connect failed";
    /// A write to the transport failed; followed by the transport's error.
    pub const WRITE_FAILED: &str = "write failed";
    /// A read from the transport failed; followed by the transport's error.
    pub const READ_FAILED: &str = "read failed";
    /// The worker thread ended unexpectedly.
    pub const WORKER_PANICKED: &str = "machine worker thread panicked";
    /// Cycle start refused while a stop-on-error stream halts.
    pub const CYCLE_START_IGNORED: &str = "cycle start ignored: probe stream halting";
    /// A manual line too long for the controller; followed by the line.
    pub const LINE_TOO_LONG: &str = "line too long for the controller buffer";
    /// A program line too long for the controller; followed by its line number.
    pub const PROGRAM_LINE_TOO_LONG: &str = "program line too long for the controller buffer";
    /// Joins a controller reply to the program line it refused: `error:9 on program line 41`.
    pub const ON_PROGRAM_LINE: &str = "on program line";
    /// A manual line dropped by a halt's reset; followed by the line.
    pub const DROPPED_AFTER_PROBE_ERROR: &str = "dropped: controller reset after probe error";
    /// A manual line or a program start sent before the controller reset itself, dropped
    /// so it never runs after the reset; followed by the line.
    pub const NOT_SENT_AFTER_RESET: &str = "not sent: the controller was reset";
    /// A jog with an unusable distance or feed; followed by both.
    pub const JOG_INVALID: &str = "jog ignored: invalid distance or feed";
    /// A jog refused while a program streams.
    pub const JOG_WHILE_STREAMING: &str = "jog ignored: a program is streaming";
    /// [`MachineError::AlreadyConnected`](super::MachineError::AlreadyConnected).
    pub const ALREADY_CONNECTED: &str = "already connected";
    /// [`MachineError::NotConnected`](super::MachineError::NotConnected).
    pub const NOT_CONNECTED: &str = "not connected";
    /// [`MachineError::StreamInProgress`](super::MachineError::StreamInProgress).
    pub const STREAM_IN_PROGRESS: &str = "a program is already streaming";
    /// A command Repetier firmware has no equivalent of (a GRBL `$` command, a real-time
    /// override, a stop-on-error stream), refused on a Repetier connection; followed by it.
    pub const NOT_ON_REPETIER: &str = "not supported by Repetier firmware";
    /// A program line with no Repetier equivalent, skipped; followed by the line.
    pub const NO_REPETIER_EQUIVALENT: &str = "skipped: no Repetier equivalent";
    /// A Repetier `Resend:N` for a line no longer known (or asked for too often); followed
    /// by the line number.
    pub const RESEND_FAILED: &str =
        "resend request could not be honoured; reconnect to the controller";
    /// [`MachineError::Halted`](super::MachineError::Halted): after a Repetier `M112`.
    pub const HALTED: &str = "the controller is emergency-stopped (M112): reset it or reconnect";

    /// Every text above, for a front end's translation table.
    pub const ALL: [&str; 19] = [
        CONNECT_FAILED,
        WRITE_FAILED,
        READ_FAILED,
        WORKER_PANICKED,
        CYCLE_START_IGNORED,
        LINE_TOO_LONG,
        PROGRAM_LINE_TOO_LONG,
        ON_PROGRAM_LINE,
        DROPPED_AFTER_PROBE_ERROR,
        NOT_SENT_AFTER_RESET,
        JOG_INVALID,
        JOG_WHILE_STREAMING,
        ALREADY_CONNECTED,
        NOT_CONNECTED,
        STREAM_IN_PROGRESS,
        NOT_ON_REPETIER,
        NO_REPETIER_EQUIVALENT,
        RESEND_FAILED,
        HALTED,
    ];
}

/// Errors returned by [`MachineController`] calls.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MachineError {
    /// `connect` was called while a connection is open.
    #[error("{}", worker_text::ALREADY_CONNECTED)]
    AlreadyConnected,
    /// The call needs an open connection.
    #[error("{}", worker_text::NOT_CONNECTED)]
    NotConnected,
    /// `start_stream` was called while a program is streaming.
    #[error("{}", worker_text::STREAM_IN_PROGRESS)]
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
    /// The request has no equivalent on the connected Repetier firmware (a GRBL `$` command
    /// other than `$H` and `$J=`, which are translated, or a stop-on-error stream); holds it.
    #[error("{text}: {0}", text = worker_text::NOT_ON_REPETIER)]
    NotOnRepetier(String),
    /// A Repetier controller was emergency-stopped (`M112`): nothing but `M112` is sent
    /// until its firmware restarts (`start`) or the connection is opened again.
    #[error("{}", worker_text::HALTED)]
    Halted,
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
    /// The controller identified itself as another firmware than the connection was opened
    /// for (the active device profile's), so the worker closed the connection, as the GTK
    /// front end rejected it. [`MachineEvent::Disconnected`] follows.
    ProtocolMismatch {
        /// The protocol the connection was opened for.
        configured: ControllerProtocol,
        /// The protocol the controller's identity names.
        detected: ControllerProtocol,
    },
}

/// How a stream reacts to an `error:N` reply to one of its lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamMode {
    /// Report the error and keep streaming (a job: GRBL skips the refused line).
    #[default]
    Normal,
    /// Stop at once (a probe routine, whose later lines are relative moves that must not run
    /// after a refused probe line): stop feeding lines, feed hold (`!`), wait for the hold to
    /// complete (a `Hold:0` or `Alarm` report after the hold, polling `?`; each report that
    /// does not complete it extends the wait by a second, up to [`HALT_MAX_WAIT`]; a second
    /// without any report ends it), then soft reset (0x18), which keeps the machine position
    /// once the motion has stopped. Reported as
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

/// How [`MachineEvent::Sent`] names the Repetier emergency stop (`M112`), so a front end
/// tells it from an `M112` typed as a command line.
pub const REPETIER_ESTOP_LABEL: &str = "M112 (Emergency stop)";

/// Builds a fresh, unconnected transport for each [`MachineController::connect`].
pub type TransportFactory = Box<dyn Fn() -> Box<dyn Communicator> + Send>;

/// Called by the worker after it emits an event, so a UI can schedule a repaint.
pub type RepaintHook = Box<dyn Fn() + Send + Sync>;

type SharedHook = Arc<Mutex<Option<RepaintHook>>>;

/// Requests sent from the controller to its worker.
#[derive(Debug)]
enum Command {
    /// A program to stream, with when it was started.
    Stream(Arc<[String]>, StreamMode, Instant),
    Pause,
    Resume,
    Stop,
    /// A line that does not change the machine state (a `$J=` jog, a `$G` query), answered
    /// through [`MachineController::lines_answered`]'s own count rather than a state-line
    /// sequence number, so it never makes the latest report stale. With when it was sent.
    Line(String, Instant),
    /// A command line that may change the machine state, with its sequence number and when
    /// it was sent.
    StateLine(String, u64, Instant),
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
    /// The settings of the last connection started (kept after it closes).
    last_connection: Option<ConnectionSettings>,
    /// The protocol of the last connection started (kept after it closes).
    protocol: ControllerProtocol,
    /// Shared with the worker, per connection: set by a Repetier `M112`, cleared by the
    /// firmware's `start`; see [`MachineController::is_halted`].
    halted: Arc<AtomicBool>,
    repaint: SharedHook,
    streaming: Arc<AtomicBool>,
    /// Sequence number of the last command line sent with [`send_line`](Self::send_line).
    requested: u64,
    /// The highest such sequence number written before the latest status report.
    fresh: Arc<AtomicU64>,
    /// The highest such sequence number whose reply (`ok` or `error:N`) was read, or which
    /// a reset dropped; see [`lines_answered`](Self::lines_answered).
    answered: Arc<AtomicU64>,
    /// How many jogs and queries ([`send_query`](Self::send_query)) were handed to the worker
    /// on this connection.
    jogs_requested: u64,
    /// How many of those were answered or dropped (never to run).
    jogs_answered: Arc<AtomicU64>,
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
            last_connection: None,
            protocol: ControllerProtocol::Unknown,
            halted: Arc::new(AtomicBool::new(false)),
            repaint: Arc::new(Mutex::new(None)),
            streaming: Arc::new(AtomicBool::new(false)),
            requested: 0,
            fresh: Arc::new(AtomicU64::new(0)),
            answered: Arc::new(AtomicU64::new(0)),
            jogs_requested: 0,
            jogs_answered: Arc::new(AtomicU64::new(0)),
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
        self.connect_with(cfg, ControllerProtocol::Unknown)
    }

    /// Starts connecting with `cfg` to a controller speaking `protocol` (the active device
    /// profile's), as [`connect`](Self::connect) does.
    ///
    /// [`ControllerProtocol::Repetier`] streams with Repetier's flow control (see the module
    /// docs); any other protocol streams with GRBL's. Once connected, a controller that
    /// identifies itself as another firmware than a known `protocol` is disconnected with
    /// [`MachineEvent::ProtocolMismatch`]; [`ControllerProtocol::Unknown`] checks nothing.
    ///
    /// # Errors
    /// As [`connect`](Self::connect).
    pub fn connect_with(
        &mut self,
        cfg: &ConnectionSettings,
        protocol: ControllerProtocol,
    ) -> Result<(), MachineError> {
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
        self.answered = Arc::new(AtomicU64::new(0));
        self.jogs_requested = 0;
        self.jogs_answered = Arc::new(AtomicU64::new(0));
        self.halted = Arc::new(AtomicBool::new(false));
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
            [self.answered.clone(), self.jogs_answered.clone()],
        )
        .with_protocol(protocol, self.halted.clone());
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
        self.last_connection = Some(cfg.clone());
        self.protocol = protocol;
        Ok(())
    }

    /// The protocol of the last connection started ([`connect_with`](Self::connect_with)),
    /// [`ControllerProtocol::Unknown`] for one started with [`connect`](Self::connect).
    pub fn protocol(&self) -> ControllerProtocol {
        self.protocol
    }

    /// Whether the last connection started speaks Repetier's protocol: no GRBL real-time
    /// byte or `$` command is written on it, and the emergency stop is `M112`.
    pub fn is_repetier(&self) -> bool {
        self.protocol == ControllerProtocol::Repetier
    }

    /// Whether the Repetier controller was emergency-stopped (`M112`, from [`stop`](Self::stop)
    /// or typed as a line): every line, query, jog and stream but `M112` is then refused with
    /// [`MachineError::Halted`] until its firmware restarts (`start`) or the connection is
    /// opened again.
    pub fn is_halted(&self) -> bool {
        self.halted.load(Ordering::SeqCst)
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
        self.start_stream_at(program, mode, Instant::now())
    }

    /// [`start_stream_with`](Self::start_stream_with) as if it had been called at `sent`, so
    /// tests can reproduce a program start still in the worker's channel at a reset.
    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn start_stream_sent_earlier(
        &mut self,
        program: Arc<[String]>,
        sent: Instant,
    ) -> Result<(), MachineError> {
        self.start_stream_at(program, StreamMode::default(), sent)
    }

    fn start_stream_at(
        &mut self,
        program: Arc<[String]>,
        mode: StreamMode,
        sent: Instant,
    ) -> Result<(), MachineError> {
        if !self.is_worker_running() {
            return Err(MachineError::NotConnected);
        }
        // Repetier acknowledges every line with `ok`, refused or not, so a stop-on-error
        // stream (a probe routine) could never stop on its error.
        if self.is_halted() {
            return Err(MachineError::Halted);
        }
        if mode == StreamMode::StopOnError && self.is_repetier() {
            return Err(MachineError::NotOnRepetier(
                program.first().cloned().unwrap_or_default(),
            ));
        }
        if self.streaming.swap(true, Ordering::SeqCst) {
            return Err(MachineError::StreamInProgress);
        }
        if let Err(e) = self.send(Command::Stream(program, mode, sent)) {
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

    /// Soft-resets the controller and drops the rest of the program. On a Repetier
    /// connection: the emergency stop `M112`, written ahead of anything queued, and every line
    /// sent before it is dropped, never to run.
    pub fn stop(&mut self) {
        self.send_or_report(Command::Stop);
    }

    /// Jogs `axis` by `distance_mm` (relative) at `feed_mm_min` with a `$J=` command.
    ///
    /// A linear axis is written with 2 decimals (mm), a rotary one with 3 (degrees).
    /// Refused (reported as [`MachineEvent::Error`]) for a non-finite distance, one that
    /// would be written as zero (no zero-length jog is sent), a feed that is not positive,
    /// or while a program streams.
    pub fn jog(&mut self, axis: Axis, distance_mm: f32, feed_mm_min: f32) {
        self.jog_sent_at(axis, distance_mm, feed_mm_min, Instant::now());
    }

    /// [`jog`](Self::jog) as if it had been sent at `sent`, so tests can reproduce a jog that
    /// was still in the worker's channel when the controller reset.
    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn jog_sent_earlier(
        &mut self,
        axis: Axis,
        distance_mm: f32,
        feed_mm_min: f32,
        sent: Instant,
    ) {
        self.jog_sent_at(axis, distance_mm, feed_mm_min, sent);
    }

    fn jog_sent_at(&mut self, axis: Axis, distance_mm: f32, feed_mm_min: f32, sent: Instant) {
        let rotary = matches!(axis, Axis::A | Axis::B | Axis::C);
        // Judged on the text the line writes: zero there means no motion.
        let decimals = if rotary {
            ROTARY_JOG_DECIMALS
        } else {
            LINEAR_JOG_DECIMALS
        };
        let written_zero = jog_writes_zero(distance_mm, decimals);
        if !distance_mm.is_finite()
            || written_zero
            || !(feed_mm_min.is_finite() && feed_mm_min > 0.0)
        {
            self.pending.push(MachineEvent::Error(format!(
                "{}: {distance_mm}, {feed_mm_min}",
                worker_text::JOG_INVALID
            )));
            return;
        }
        if self.streaming.load(Ordering::SeqCst) {
            self.pending.push(MachineEvent::Error(
                worker_text::JOG_WHILE_STREAMING.to_string(),
            ));
            return;
        }
        if self.is_halted() {
            self.pending
                .push(MachineEvent::Error(worker_text::HALTED.to_string()));
            return;
        }
        let line = if rotary {
            rotary_jog_line(axis.letter(), distance_mm, feed_mm_min)
        } else {
            linear_jog_command(axis.letter(), distance_mm, feed_mm_min)
        };
        match self.send(Command::Line(line, sent)) {
            Ok(()) => self.jogs_requested += 1,
            Err(e) => self.pending.push(MachineEvent::Error(e.to_string())),
        }
    }

    /// Queues one command line (`$H`, `$X`, an MDI line), trimmed of spaces, behind any
    /// manual line still waiting.
    ///
    /// # Errors
    /// [`MachineError::EmptyLine`], [`MachineError::NonPrintable`] for a byte outside
    /// printable ASCII (0x20-0x7E), [`MachineError::RealtimeCharacter`] for `!`, `~` or `?`,
    /// [`MachineError::LineTooLong`] for a line that, with its newline, does not fit the
    /// controller's RX buffer ([`GRBL_RX_BUFFER_BYTES`]; [`MAX_LINE_LENGTH`] is higher),
    /// [`MachineError::NotOnRepetier`] for a `$` command other than `$H` and `$J=` on a
    /// Repetier connection, [`MachineError::NotConnected`] without a connection, or
    /// [`MachineError::StreamInProgress`] while a program streams.
    pub fn send_line(&mut self, line: &str) -> Result<(), MachineError> {
        let line = self.check_line(line)?;
        self.requested += 1;
        self.send(Command::StateLine(
            line.to_string(),
            self.requested,
            Instant::now(),
        ))
    }

    /// Queues a line that only asks the controller something (`$G`), behind any manual line
    /// still waiting. It is answered like a [`send_line`](Self::send_line) line (see
    /// [`lines_answered`](Self::lines_answered)), but as it changes nothing on the machine it
    /// does not make the latest report stale ([`report_fresh`](Self::report_fresh)).
    ///
    /// # Errors
    /// As [`send_line`](Self::send_line).
    pub fn send_query(&mut self, line: &str) -> Result<(), MachineError> {
        let line = self.check_line(line)?;
        self.send(Command::Line(line.to_string(), Instant::now()))?;
        self.jogs_requested += 1;
        Ok(())
    }

    /// The checks of [`send_line`](Self::send_line); returns the line trimmed of spaces.
    fn check_line<'a>(&self, line: &'a str) -> Result<&'a str, MachineError> {
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
        if self.is_halted() && !is_m112(line) {
            return Err(MachineError::Halted);
        }
        if self.is_repetier() && !repetier_translates(line) {
            return Err(MachineError::NotOnRepetier(line.to_string()));
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
        Ok(line)
    }

    /// Queues `line` for the worker as [`send_line`](Self::send_line) would, without its
    /// checks (a stream in progress included), so tests can reach the worker's own guards.
    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn queue_line_unchecked(&mut self, line: &str) {
        self.requested += 1;
        self.send_or_report(Command::StateLine(
            line.to_string(),
            self.requested,
            Instant::now(),
        ));
    }

    /// Whether the latest status report was read after every line sent with
    /// [`send_line`](Self::send_line) had been written. A line such as `$H` changes the
    /// machine state, so until then the last report may describe the machine before it (Idle
    /// before homing): a front end must not jog on it. True before any line is sent.
    pub fn report_fresh(&self) -> bool {
        self.fresh.load(Ordering::SeqCst) >= self.requested
    }

    /// The settings of the last connection started (port or host, type), kept after it
    /// closes so events read before the close can still be attributed to it.
    pub fn last_connection(&self) -> Option<&ConnectionSettings> {
        self.last_connection.as_ref()
    }

    /// Whether every line sent with [`send_line`](Self::send_line), [`jog`](Self::jog) or
    /// [`send_query`](Self::send_query) has
    /// had its reply (`ok` or `error:N`) or was dropped (by a reset, or a jog cancel for a
    /// queued jog). The controller reports no event for a manual line's `ok`, so a front end
    /// that tells a line's answer by the order of the replies (a setting write) must start
    /// only when nothing earlier is unanswered: a jog's `error:15` must not be taken for the
    /// write's. True before any line is sent.
    pub fn lines_answered(&self) -> bool {
        self.answered.load(Ordering::SeqCst) >= self.requested
            && self.jogs_answered.load(Ordering::SeqCst) >= self.jogs_requested
    }

    /// Whether a program is streaming: from a successful
    /// [`start_stream`](Self::start_stream) until it finishes or stops.
    pub fn is_streaming(&self) -> bool {
        self.streaming.load(Ordering::SeqCst)
    }

    /// Writes a real-time byte ahead of any queued program line.
    ///
    /// On a Repetier connection no real-time byte is written: soft reset is the emergency
    /// stop (`M112`, as [`stop`](Self::stop)), feed hold and cycle start pause and resume
    /// feeding program lines, status query and jog cancel do nothing (Repetier has neither),
    /// and the overrides are refused with [`MachineEvent::Error`].
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
                worker_text::WORKER_PANICKED.to_string(),
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

/// Whether a command line has a Repetier equivalent: anything but a GRBL `$` command, except
/// homing (`$H`, sent as `G28`) and jogging (`$J=`, sent as `G1` moves), which
/// [`RepetierProcessor`] translates.
///
/// # Arguments
/// * `line` - One command line.
fn repetier_translates(line: &str) -> bool {
    let upper = line.trim().to_ascii_uppercase();
    !upper.starts_with('$') || upper.starts_with("$H") || upper.starts_with("$J=")
}

/// Whether `line` is the emergency stop `M112` (and nothing else).
fn is_m112(line: &str) -> bool {
    line.trim().eq_ignore_ascii_case("M112")
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
    /// latest status report that did not complete it (a stale one, `Hold:1`, `Idle`, ...),
    /// never later than [`HALT_MAX_WAIT`] after the hold.
    deadline: Instant,
    /// When to ask for the next status report.
    next_query: Instant,
    /// Status reports still owed to `?`s written before the hold; they may describe the
    /// machine before it (even a `Hold:0` of an earlier pause), so they do not count as a
    /// completed hold.
    stale_reports: usize,
}

/// The Repetier side of a worker on a Repetier connection: one command in flight at a time,
/// as GTK's `RepetierSender` streamed.
struct RepetierLink {
    /// Numbers and checksums the lines written, keeps them for resends, frames the replies and
    /// stands in a GRBL-style status report for the position readout.
    processor: RepetierProcessor,
    /// Acknowledgements (`ok`, `wait`) still owed to the command in flight; it may have been
    /// written as several numbered lines (a jog). Zero when nothing is in flight.
    acks_due: usize,
    /// Whether the command in flight is one of `internal`'s (not a program or manual line).
    internal_in_flight: bool,
    /// The line of the last `Resend:` and how many times in a row it was asked for.
    resend_streak: (u32, u32),
    /// Lines the worker sends itself: the protocol's start-up commands (`M110`, `M115`) at
    /// connect and after the controller restarts.
    internal: VecDeque<&'static str>,
}

impl RepetierLink {
    /// A link that starts with the protocol's start-up commands.
    fn new() -> Self {
        Self {
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
struct WorkerState {
    comm: Box<dyn Communicator>,
    /// The protocol the connection was opened for.
    configured: ControllerProtocol,
    /// Set on a Repetier connection.
    repetier: Option<RepetierLink>,
    /// Whether the controller's identity was checked against `configured`.
    identity_checked: bool,
    /// The controller identified as another firmware: the worker closes the connection.
    rejected: bool,
    /// Shared with the controller: the Repetier firmware was emergency-stopped (`M112`).
    halted: Arc<AtomicBool>,
    commands: Receiver<Command>,
    sink: EventSink,
    streaming: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
    window: CharCountWindow,
    /// One entry per line in `window`: true for a program line, false for a manual one.
    in_flight_is_program: VecDeque<bool>,
    /// The state-line sequence number of each manual line in `window`, oldest first (0 for
    /// a jog).
    in_flight_seq: VecDeque<u64>,
    /// Shared with the controller: the highest state-line sequence number answered or
    /// dropped, see [`MachineController::lines_answered`].
    answered: Arc<AtomicU64>,
    /// Shared with the controller: how many jogs (sequence number 0) were answered or
    /// dropped, see [`MachineController::lines_answered`].
    jogs_answered: Arc<AtomicU64>,
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
    /// [`Self::lost_reply_timeout`] is presumed lost and dropped (logged), at most
    /// [`MAX_UNANSWERED_QUERIES`] are kept (the oldest goes first), and a reset clears them.
    /// An unsolicited report (auto-reporting firmware) can still answer for a `?` and make
    /// the count low until that `?`'s own reply arrives; the next `?`s set it right again.
    /// A miscount cannot make a halt reset in motion: only a `Hold:0` or `Alarm` report
    /// completes a halt, see [`Halt`].
    unanswered_queries: VecDeque<Instant>,
    /// The slowest recent reply: the age of the `?` each status report answered, decaying by
    /// 1/16 per report. A slow link (a Wi-Fi or telnet bridge) raises it, so its late replies
    /// are not presumed lost.
    reply_latency: Duration,
    /// Whether lost replies were logged since the last status report, so a dead link logs
    /// once rather than at every `?`.
    lost_logged: bool,
    /// A `$J` jog was queued and is not known to be over: set when it is queued and again when
    /// it is written, cleared by jog cancel (0x85, which also drops queued jogs), soft reset
    /// (0x18), or an `Idle` report after a `Jog` one received since it was written.
    jog_outstanding: bool,
    /// A `Jog` state was reported since the outstanding jog was written.
    jog_seen: bool,
    /// The stream being halted after an error, see [`StreamMode::StopOnError`].
    halt: Option<Halt>,
    /// When the controller's last reset banner was read: manual lines sent before it are
    /// dropped when they arrive, as the ones already queued were.
    reset_at: Option<Instant>,
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
    fn run(mut self, params: &ConnectionParams) {
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

        let exit = self.io_loop();
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

    fn write(&mut self, data: &[u8]) -> Result<(), String> {
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
            Command::Realtime(byte) => {
                self.write(&[byte])?;
                self.echo_realtime(byte);
                if byte == RealtimeCommand::JogCancel.byte()
                    || byte == RealtimeCommand::SoftReset.byte()
                {
                    self.drop_queued_jogs();
                }
                if byte == RealtimeCommand::SoftReset.byte() {
                    // The reset drops the controller's buffered program lines: end the
                    // stream (and any halt) now rather than feed a resetting controller.
                    self.end_stream(false);
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

    /// How long a `?` may stay unanswered before its reply is presumed lost: twice the
    /// slowest recent reply, at least [`QUERY_REPLY_TIMEOUT`], at most [`HALT_MAX_WAIT`].
    fn lost_reply_timeout(&self) -> Duration {
        (self.reply_latency * 2).clamp(QUERY_REPLY_TIMEOUT, HALT_MAX_WAIT)
    }

    /// Drops the unanswered `?`s older than [`Self::lost_reply_timeout`]: their replies are
    /// presumed lost. Logged, once per silence (or always while halting).
    fn forget_lost_queries(&mut self) {
        let now = Instant::now();
        let timeout = self.lost_reply_timeout();
        let before = self.unanswered_queries.len();
        while self
            .unanswered_queries
            .front()
            .is_some_and(|at| now.duration_since(*at) >= timeout)
        {
            self.unanswered_queries.pop_front();
        }
        let forgotten = before - self.unanswered_queries.len();
        if forgotten > 0 && (self.halt.is_some() || !self.lost_logged) {
            tracing::warn!(
                forgotten,
                ?timeout,
                halting = self.halt.is_some(),
                "status replies presumed lost"
            );
            self.lost_logged = true;
        }
    }

    /// Takes the oldest unanswered `?` as answered by a status report, and learns the reply
    /// latency from it.
    fn answer_query(&mut self) {
        self.lost_logged = false;
        let decayed = self.reply_latency.saturating_sub(self.reply_latency / 16);
        if let Some(at) = self.unanswered_queries.pop_front() {
            self.reply_latency = decayed.max(at.elapsed());
        } else {
            self.reply_latency = decayed;
        }
    }

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
    fn drop_line(&mut self, seq: u64) {
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
    fn complete_oldest(&mut self, error: Option<String>) {
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

impl WorkerState {
    /// The same worker on a connection opened for `configured`: Repetier's flow control on a
    /// Repetier connection, and the identity check for a known protocol. `halted` is the
    /// controller's flag for a Repetier `M112`.
    fn with_protocol(mut self, configured: ControllerProtocol, halted: Arc<AtomicBool>) -> Self {
        self.configured = configured;
        self.halted = halted;
        self.repetier = (configured == ControllerProtocol::Repetier).then(RepetierLink::new);
        self
    }

    /// Drops the jogs still waiting in the queue (after a jog cancel or a reset): they must
    /// not start afterwards, and are answered by never running.
    fn drop_queued_jogs(&mut self) {
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
    fn check_identity(&mut self, line: &str) -> bool {
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
    /// GRBL real-time byte is ever written (see [`MachineController::realtime`]).
    fn handle_repetier_command(&mut self, cmd: Command) -> Result<(), String> {
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
    fn repetier_emergency_stop(&mut self) -> Result<(), String> {
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
    fn feed_repetier(&mut self) -> Result<(), String> {
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

    /// Handles bytes read on a Repetier connection: acknowledgements and resend requests
    /// first, then the reply lines (with the processor's stand-in status report).
    fn receive_repetier(&mut self, data: Vec<u8>) -> Result<(), String> {
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
        if line.is_empty() || lower.starts_with("ok") || lower.contains("wait") {
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
                *link = RepetierLink::new();
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
