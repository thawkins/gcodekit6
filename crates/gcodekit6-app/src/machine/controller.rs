//! [`MachineController`], the non-blocking front-end handle that owns the worker thread.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Instant;

use gcodekit6_communication::firmware::ControllerProtocol;
use gcodekit6_communication::{Communicator, SerialCommunicator};
use gcodekit6_settings::ConnectionSettings;

use super::grbl_stream::{
    GRBL_RX_BUFFER_BYTES, LINEAR_JOG_DECIMALS, MAX_LINE_LENGTH, ROTARY_JOG_DECIMALS,
    jog_writes_zero, linear_jog_command, rotary_jog_line,
};
use super::types::{
    Axis, Command, MachineError, MachineEvent, RealtimeCommand, RepaintHook, SharedHook,
    StreamMode, TransportFactory, worker_text,
};
use super::worker::{EventSink, WorkerState, connection_params, is_m112, repetier_translates};

/// The running worker thread and its channels.
pub(super) struct Worker {
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
    /// The program of the last stream started, see
    /// [`MachineController::streamed_program`].
    streamed: Option<Arc<[String]>>,
    /// Shared with the worker, per connection: set by a Repetier `M112`, cleared by the
    /// firmware's `start`; see [`MachineController::is_halted`].
    halted: Arc<AtomicBool>,
    /// Repetier pen mode, shared with every worker; see
    /// [`MachineController::set_pen_mode`].
    pen_mode: Arc<AtomicBool>,
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
            pen_mode: Arc::new(AtomicBool::new(false)),
            streamed: None,
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
        // The last connection's program is not this one's: the WCS watch reads it.
        self.streamed = None;
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
        .with_protocol(protocol, self.halted.clone(), self.pen_mode.clone());
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

    /// Turns Repetier pen mode on or off (the active device profile's Pen Mode):
    /// on a Repetier connection, a program streamed from then on
    /// has `M3`/`M4` written as `G0 Z0` (pen down) and `M5` as `G0 Z5` (pen up), and a
    /// command that repeats the pen's state is skipped. Manual lines are sent as typed; other
    /// protocols ignore it.
    ///
    /// # Arguments
    /// * `on` - Whether pen mode is on.
    pub fn set_pen_mode(&self, on: bool) {
        self.pen_mode.store(on, Ordering::Relaxed);
    }

    /// The protocol of the last connection started ([`connect_with`](Self::connect_with)),
    /// [`ControllerProtocol::Unknown`] for one started with [`connect`](Self::connect).
    pub fn protocol(&self) -> ControllerProtocol {
        self.protocol
    }

    /// The program of the last stream started (a job, a probe routine, any caller's), kept
    /// after it ends: what the controller actually ran, whatever the editor holds.
    pub fn streamed_program(&self) -> Option<&Arc<[String]>> {
        self.streamed.as_ref()
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
    /// with [`grbl_stream::prepare_program`](super::grbl_stream::prepare_program) (comments removed, length checked).
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
        if let Err(e) = self.send(Command::Stream(program.clone(), mode, sent)) {
            self.streaming.store(false, Ordering::SeqCst);
            return Err(e);
        }
        self.streamed = Some(program);
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

    /// Writes a real-time byte ahead of any queued program line. A soft reset is
    /// [`stop`](Self::stop): every line in flight or queued is dropped, never to be written
    /// after the reset.
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
