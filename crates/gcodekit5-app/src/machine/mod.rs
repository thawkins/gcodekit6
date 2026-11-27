//! # Machine controller
//!
//! [`MachineController`] drives a GRBL-style controller from a front end without ever
//! blocking the caller on I/O. It owns one worker thread that holds the transport (a
//! [`Communicator`](gcodekit5_communication::Communicator) from `gcodekit5-communication`); the caller talks to it over two
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
//! [`ControllerProtocol::Repetier`](gcodekit5_communication::firmware::ControllerProtocol::Repetier)) the worker speaks Repetier's flow-control protocol
//! instead: every line is numbered and
//! checksummed by `gcodekit5-communication`'s [`RepetierProcessor`](gcodekit5_communication::firmware::repetier::RepetierProcessor), one command is in flight
//! at a time, a `Resend:N` replays the numbered lines from `N`, and no GRBL real-time byte or
//! `$` command is ever written. The emergency stop is `M112`.
//!
//! No lock is held across I/O: the transport is owned by the worker alone, and the
//! only shared state is the repaint hook and a per-connection atomic "streaming" flag.
//!
//! Depends on `gcodekit5-communication` (transport, response parsing) and
//! `gcodekit5-settings` ([`ConnectionSettings`](gcodekit5_settings::ConnectionSettings)).

mod controller;
pub mod grbl_stream;
mod repetier;
mod status_tracker;
mod types;
mod worker;

use std::time::Duration;

pub use controller::MachineController;
pub use types::{
    Axis, MachineError, MachineEvent, REPETIER_ESTOP_LABEL, RealtimeCommand, RepaintHook,
    StreamMode, TransportFactory, realtime_label, worker_text,
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
/// The most status queries whose replies are tracked; see [`WorkerState::query_tags`](worker::WorkerState::query_tags).
const MAX_QUERY_TAGS: usize = 3;
/// The shortest time a `?` may stay unanswered before the worker presumes its reply lost (a
/// reply dropped by a reset or the transport); on a slower link the time follows the
/// observed reply latency, see [`WorkerState::lost_reply_timeout`](worker::WorkerState::lost_reply_timeout).
const QUERY_REPLY_TIMEOUT: Duration = Duration::from_secs(2);
/// The most unanswered `?`s the worker remembers; see [`WorkerState::unanswered_queries`](worker::WorkerState::unanswered_queries).
const MAX_UNANSWERED_QUERIES: usize = 256;
/// How many times in a row the worker replays the same line on a Repetier `Resend:` before it
/// stops the stream: a line the firmware keeps refusing (one longer than its buffer, a noisy
/// link) would otherwise be replayed for ever.
const MAX_REPETIER_RESENDS: u32 = 5;
/// How long the worker waits before each automatic reconnect attempt: the first attempts
/// come quickly (a controller rebooting), later ones at the last interval.
const RECONNECT_DELAYS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(5),
];
/// How many times the worker tries to reopen a dropped connection (Auto Reconnect) before it
/// gives up: about 40 s in all with [`RECONNECT_DELAYS`].
pub const RECONNECT_ATTEMPTS: u32 = 10;
/// How long an idle worker waits for a command before reading the transport again.
const IDLE_WAIT: Duration = Duration::from_millis(2);
