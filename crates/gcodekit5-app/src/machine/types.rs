//! The public value types of the machine controller: errors, events, axes, real-time
//! commands, stream modes, the worker's message texts, and the worker command channel type.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use gcodekit5_communication::Communicator;
use gcodekit5_communication::firmware::ControllerProtocol;
use gcodekit5_communication::firmware::grbl::status_parser::FullStatus;
use gcodekit5_communication::firmware::grbl::{FEED_OVERRIDE_CODES, SPINDLE_OVERRIDE_CODES};

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
    /// [`MachineError::EmptyLine`](super::MachineError::EmptyLine).
    pub const EMPTY_LINE: &str = "the command line is empty";
    /// [`MachineError::Halted`](super::MachineError::Halted): after a Repetier `M112`.
    pub const HALTED: &str = "the controller is emergency-stopped (M112): reset it or reconnect";

    /// Every text above, for a front end's translation table.
    pub const ALL: [&str; 20] = [
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
        EMPTY_LINE,
        HALTED,
    ];
}

/// Errors returned by [`MachineController`](super::MachineController) calls.
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
    #[error("{}", worker_text::EMPTY_LINE)]
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
/// [`MachineController::poll`](super::MachineController::poll).
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
    /// lines of a stream are not reported (the console does not echo them).
    Sent(String),
    /// An error from the controller or the transport.
    Error(String),
    /// The controller identified itself as another firmware than the connection was opened
    /// for (the active device profile's), so the worker closed the connection.
    /// [`MachineEvent::Disconnected`] follows.
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
    /// does not complete it extends the wait by a second, up to [`HALT_MAX_WAIT`](super::HALT_MAX_WAIT); a second
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

/// Builds a fresh, unconnected transport for each [`MachineController::connect`](super::MachineController::connect).
pub type TransportFactory = Box<dyn Fn() -> Box<dyn Communicator> + Send>;

/// Called by the worker after it emits an event, so a UI can schedule a repaint.
pub type RepaintHook = Box<dyn Fn() + Send + Sync>;

pub(super) type SharedHook = Arc<Mutex<Option<RepaintHook>>>;

/// Requests sent from the controller to its worker.
#[derive(Debug)]
pub(super) enum Command {
    /// A program to stream, with when it was started.
    Stream(Arc<[String]>, StreamMode, Instant),
    Pause,
    Resume,
    Stop,
    /// A line that does not change the machine state (a `$J=` jog, a `$G` query), answered
    /// through [`MachineController::lines_answered`](super::MachineController::lines_answered)'s own count rather than a state-line
    /// sequence number, so it never makes the latest report stale. With when it was sent.
    Line(String, Instant),
    /// A command line that may change the machine state, with its sequence number and when
    /// it was sent.
    StateLine(String, u64, Instant),
    Realtime(u8),
    Shutdown,
}
