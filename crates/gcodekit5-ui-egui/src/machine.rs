//! The app's machine connection: the shared [`MachineController`], its events of the current
//! frame and the state derived from them.
//!
//! [`Machine::poll`] runs once per frame (from `GcodeKitApp`), takes every event the
//! controller reported, keeps only the latest [`MachineEvent::StreamProgress`] of the frame
//! and folds the events into [`MachineStatus`]. Everything that shows machine state reads the
//! same frame's events and status from here (the Machine Control pane now; the console and
//! status bar later), so no consumer polls the controller a second time.
//!
//! The controller is shared as `Arc<Mutex<MachineController>>`; every call on it returns
//! without waiting for I/O (its worker thread owns the transport), so the lock is held only
//! for the length of one call.
//!
//! Dependencies: `gcodekit5-app` (controller, events, program preparation),
//! `gcodekit5-communication` (status report types).

use std::sync::{Arc, Mutex, PoisonError};

use gcodekit5_app::machine::grbl_stream::prepare_program;
use gcodekit5_app::machine::{MachineController, MachineEvent};
use gcodekit5_communication::firmware::grbl::status_parser::{FullStatus, WorkCoordinateOffset};

/// Longest line a program may hold, as grblHAL accepts (GRBL's own limit is 80).
const MAX_LINE_LENGTH: usize = 256;

/// Stream progress: program lines written and acknowledged out of `total`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// Lines written to the controller.
    pub sent: usize,
    /// Lines acknowledged (`ok` or `error:N`).
    pub acked: usize,
    /// Program length.
    pub total: usize,
}

/// How the last stream ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamEnd {
    /// Every line was acknowledged.
    Finished,
    /// It ended early (stop, reset, alarm, disconnect).
    Stopped,
}

/// Machine state derived from the controller's events.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MachineStatus {
    /// The transport is open.
    pub connected: bool,
    /// A program is streaming: from its first progress report until it finishes or stops.
    pub streaming: bool,
    /// The latest stream progress, kept after the stream ends.
    pub progress: Option<Progress>,
    /// How the last stream ended, if one did.
    pub stream_end: Option<StreamEnd>,
    /// The latest status report.
    pub report: Option<FullStatus>,
    /// The latest work coordinate offset (GRBL sends it only every few reports).
    pub wco: Option<WorkCoordinateOffset>,
    /// The latest error reported by the controller or transport.
    pub last_error: Option<String>,
}

impl MachineStatus {
    /// Folds `events` (oldest first) into the status.
    ///
    /// # Arguments
    /// * `events` - Events of one frame.
    pub fn apply(&mut self, events: &[MachineEvent]) {
        for event in events {
            match event {
                MachineEvent::Connected => {
                    self.connected = true;
                    self.last_error = None;
                }
                MachineEvent::Disconnected => {
                    if self.streaming {
                        self.stream_end = Some(StreamEnd::Stopped);
                    }
                    self.connected = false;
                    self.streaming = false;
                    self.report = None;
                    self.wco = None;
                }
                MachineEvent::StatusReport(report) => {
                    if let Some(wco) = report.wco {
                        self.wco = Some(wco);
                    }
                    self.report = Some((**report).clone());
                }
                MachineEvent::Received(_) => {}
                MachineEvent::StreamProgress { sent, acked, total } => {
                    if !self.streaming {
                        self.stream_end = None;
                    }
                    self.streaming = true;
                    self.progress = Some(Progress {
                        sent: *sent,
                        acked: *acked,
                        total: *total,
                    });
                }
                MachineEvent::StreamFinished { lines } => {
                    self.streaming = false;
                    self.stream_end = Some(StreamEnd::Finished);
                    self.progress = Some(Progress {
                        sent: *lines,
                        acked: *lines,
                        total: *lines,
                    });
                }
                MachineEvent::StreamStopped { acked, total } => {
                    self.streaming = false;
                    self.stream_end = Some(StreamEnd::Stopped);
                    let sent = self.progress.map_or(*acked, |p| p.sent.max(*acked));
                    self.progress = Some(Progress {
                        sent,
                        acked: *acked,
                        total: *total,
                    });
                }
                MachineEvent::Error(message) => self.last_error = Some(message.clone()),
            }
        }
    }
}

/// Keeps only the last [`MachineEvent::StreamProgress`] of `events`, at its position; every
/// other event is kept in order. Progress reports carry absolute counts, so the last one
/// says everything the earlier ones did.
///
/// # Arguments
/// * `events` - One frame's events, oldest first.
pub fn coalesce_progress(events: Vec<MachineEvent>) -> Vec<MachineEvent> {
    let last = events
        .iter()
        .rposition(|e| matches!(e, MachineEvent::StreamProgress { .. }));
    events
        .into_iter()
        .enumerate()
        .filter(|(i, e)| !matches!(e, MachineEvent::StreamProgress { .. }) || Some(*i) == last)
        .map(|(_, e)| e)
        .collect()
}

/// Locks `controller`, recovering it if a previous holder panicked (the controller's state
/// lives on its worker thread, so a panic while locked leaves nothing half-updated).
///
/// # Arguments
/// * `controller` - The shared controller.
pub fn lock(controller: &Mutex<MachineController>) -> std::sync::MutexGuard<'_, MachineController> {
    controller.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The app's machine connection.
#[derive(Debug)]
pub struct Machine {
    controller: Arc<Mutex<MachineController>>,
    status: MachineStatus,
    events: Vec<MachineEvent>,
    program: Option<Arc<[String]>>,
}

impl Machine {
    /// Wraps `controller`.
    ///
    /// # Arguments
    /// * `controller` - The controller the app drives.
    pub fn new(controller: MachineController) -> Self {
        Self {
            controller: Arc::new(Mutex::new(controller)),
            status: MachineStatus::default(),
            events: Vec::new(),
            program: None,
        }
    }

    /// The shared controller, for the panes that act on the machine.
    pub fn controller(&self) -> &Arc<Mutex<MachineController>> {
        &self.controller
    }

    /// State derived from every event so far.
    pub fn status(&self) -> &MachineStatus {
        &self.status
    }

    /// This frame's events, oldest first, with stream progress coalesced.
    pub fn events(&self) -> &[MachineEvent] {
        &self.events
    }

    /// Whether a program is streaming.
    pub fn job_running(&self) -> bool {
        self.status.streaming
    }

    /// Takes the controller's events, replacing the previous frame's, and updates the status.
    /// Call once per frame. Never blocks on I/O.
    pub fn poll(&mut self) {
        let events = lock(&self.controller).poll();
        self.events = coalesce_progress(events);
        self.status.apply(&self.events);
    }

    /// Adds `events` taken from the controller outside [`Machine::poll`] to this frame's
    /// events and the status, so no consumer misses them.
    ///
    /// # Arguments
    /// * `events` - Events polled from the controller directly.
    pub fn absorb(&mut self, events: Vec<MachineEvent>) {
        self.status.apply(&events);
        self.events.extend(events);
        self.events = coalesce_progress(std::mem::take(&mut self.events));
    }

    /// The program the Start command streams, if one is loaded.
    pub fn program(&self) -> Option<&Arc<[String]>> {
        self.program.as_ref()
    }

    /// Prepares `gcode` for streaming (comments removed, empty lines dropped, line length
    /// checked) and keeps it as the program to stream. The editor loads its text here once it
    /// exists (Task 9).
    ///
    /// # Arguments
    /// * `gcode` - The program text.
    ///
    /// # Errors
    /// Names the first line that is too long for the controller.
    pub fn set_program(&mut self, gcode: &str) -> Result<usize, String> {
        let prepared = prepare_program(gcode, false, MAX_LINE_LENGTH)?;
        let lines: Arc<[String]> = prepared.lines().map(str::to_string).collect();
        let count = lines.len();
        self.program = Some(lines);
        Ok(count)
    }

    /// Closes the connection, releasing the serial port, and folds the events this causes
    /// into the status.
    pub fn disconnect(&mut self) {
        let events = {
            let mut controller = lock(&self.controller);
            controller.disconnect();
            controller.poll()
        };
        self.absorb(events);
    }
}
