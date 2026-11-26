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

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::grbl_stream::{
    GrblResponseLine, MAX_LINE_LENGTH, classify_response, prepare_program,
};
use gcodekit5_app::machine::{MachineController, MachineEvent, worker_text};
use gcodekit5_app::work_zero::{Wcs, WcsWord, parser_state_wcs, wcs_in_line};
use gcodekit5_communication::firmware::grbl::status_parser::{
    FullStatus, WorkCoordinateOffset, parse_probe_report_in,
};
use gcodekit5_core::ProbeResult;

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
    /// X, Y and Z max travel in mm (`$130`-`$132`), once the controller reported them.
    pub grbl_travel: [Option<f32>; 3],
    /// Whether the controller reports positions in inches (`$13=1`). Until a `$$` line says
    /// so, reports are taken as mm, GRBL's default (as the GTK front end does).
    pub report_inches: bool,
    /// The active work coordinate system: from the controller's parser state (`[GC:...]`)
    /// or a G54-G59 line sent since. `None` until known, and again after a controller reset
    /// (which restores the default) until the controller reports it. A stream's program may
    /// change it too; the Machine Control pane asks the controller again after one.
    pub active_wcs: Option<Wcs>,
    /// Whether the controller reported its parser state on this connection; until it does,
    /// a front end may fall back to the system the user last selected.
    pub wcs_reported: bool,
    /// Whether the active system is outside G54-G59 (`G59.1`, `G54.1`), as reported or sent;
    /// [`MachineStatus::active_wcs`] is then `None`. Cleared by a G54-G59 report or line.
    pub wcs_outside: bool,
}

/// X, Y and Z in millimetres.
pub type Xyz = [f32; 3];

/// The `$13` report-inches setting a `$$` line reports, if the line is one.
///
/// # Arguments
/// * `line` - One line received from the controller.
pub fn parse_report_inches(line: &str) -> Option<bool> {
    let (key, value) = line.trim().split_once('=')?;
    (key == "$13").then_some(())?;
    let value: f32 = value.trim().parse().ok()?;
    Some(value >= 1.0)
}

/// A `[PRB:...]` probe report in mm: converted from inches when the controller reports in
/// inches (`$13=1`), as status reports are.
///
/// # Arguments
/// * `line` - One line received from the controller.
/// * `report_inches` - Whether the controller reports in inches.
pub fn probe_report(line: &str, report_inches: bool) -> Option<ProbeResult> {
    parse_probe_report_in(line.trim(), report_inches)
}

/// The `$130`-`$132` max-travel setting a `$$` line reports: axis index and value in mm.
fn parse_travel(line: &str) -> Option<(usize, f32)> {
    let (key, value) = line.trim().split_once('=')?;
    let index = match key {
        "$130" => 0,
        "$131" => 1,
        "$132" => 2,
        _ => return None,
    };
    let value: f32 = value.trim().parse().ok()?;
    (value.is_finite() && value > 0.0).then_some((index, value))
}

impl MachineStatus {
    /// Whether the latest status report allows jogging: its state is `Idle` or `Jog`. Without
    /// a report, or in any other state (Home, Alarm, Hold, Check, Run, Door, ...), a `$J`
    /// would wait in the controller's buffer and run later from a stale position.
    pub fn jog_allowed(&self) -> bool {
        self.report
            .as_ref()
            .and_then(|r| r.machine_state.as_deref())
            .is_some_and(|state| state.starts_with("Idle") || state.starts_with("Jog"))
    }

    /// The machine position (`MPos`) of the latest report, or `WPos + WCO` when the report
    /// carries only the work position.
    pub fn machine_position(&self) -> Option<Xyz> {
        let report = self.report.as_ref()?;
        let xyz = |x: f64, y: f64, z: f64| [x as f32, y as f32, z as f32];
        report.mpos.map(|p| xyz(p.x, p.y, p.z)).or_else(|| {
            let (w, o) = (report.wpos?, self.wco?);
            Some(xyz(w.x + o.x, w.y + o.y, w.z + o.z))
        })
    }

    /// The work position (`WPos`) of the latest report, or `MPos - WCO` when the report
    /// carries only the machine position.
    pub fn work_position(&self) -> Option<Xyz> {
        let report = self.report.as_ref()?;
        let xyz = |x: f64, y: f64, z: f64| [x as f32, y as f32, z as f32];
        report.wpos.map(|p| xyz(p.x, p.y, p.z)).or_else(|| {
            let (m, o) = (report.mpos?, self.wco?);
            Some(xyz(m.x - o.x, m.y - o.y, m.z - o.z))
        })
    }

    /// Which of A, B and C the latest report carries (in `MPos` or `WPos`): the rotary axes
    /// the machine has, whose readout rows are shown.
    pub fn rotary_reported(&self) -> [bool; 3] {
        let Some(report) = self.report.as_ref() else {
            return [false; 3];
        };
        let m = report.mpos.map_or([None; 3], |p| [p.a, p.b, p.c]);
        let w = report.wpos.map_or([None; 3], |p| [p.a, p.b, p.c]);
        [0, 1, 2].map(|i| m[i].is_some() || w[i].is_some())
    }

    /// The work position of A, B and C in degrees, for each rotary axis the latest report
    /// carries (`WPos`, or `MPos - WCO`); `None` for an axis it does not.
    pub fn work_rotary(&self) -> [Option<f32>; 3] {
        let Some(report) = self.report.as_ref() else {
            return [None; 3];
        };
        let off = |o: Option<f64>| o.unwrap_or(0.0);
        if let Some(w) = report.wpos {
            return [w.a, w.b, w.c].map(|v| v.map(|v| v as f32));
        }
        match (report.mpos, self.wco) {
            (Some(m), Some(o)) => {
                [(m.a, o.a), (m.b, o.b), (m.c, o.c)].map(|(v, o)| v.map(|v| (v - off(o)) as f32))
            }
            (Some(_) | None, _) => [None; 3],
        }
    }

    /// The machine position of A, B and C in degrees, for each rotary axis the latest
    /// report carries (`MPos`, or `WPos + WCO`); `None` for an axis it does not.
    pub fn machine_rotary(&self) -> [Option<f32>; 3] {
        let Some(report) = self.report.as_ref() else {
            return [None; 3];
        };
        let off = |o: Option<f64>| o.unwrap_or(0.0);
        if let Some(m) = report.mpos {
            return [m.a, m.b, m.c].map(|v| v.map(|v| v as f32));
        }
        match (report.wpos, self.wco) {
            (Some(w), Some(o)) => {
                [(w.a, o.a), (w.b, o.b), (w.c, o.c)].map(|(v, o)| v.map(|v| (v + off(o)) as f32))
            }
            (Some(_) | None, _) => [None; 3],
        }
    }

    /// The planner blocks and RX bytes free of the latest report's `Bf:` field, as
    /// `plan/rx` (GTK's "Buffer:" readout); `None` when the controller does not send it.
    pub fn buffer_text(&self) -> Option<String> {
        let buffer = self.report.as_ref()?.buffer?;
        Some(format!("{}/{}", buffer.plan, buffer.rx))
    }

    /// Takes `word` as the active system: a G54-G59 one, or one outside them (then unknown
    /// to the zero commands).
    fn set_wcs(&mut self, word: WcsWord) {
        match word {
            WcsWord::Known(wcs) => {
                self.active_wcs = Some(wcs);
                self.wcs_outside = false;
            }
            WcsWord::Other => {
                self.active_wcs = None;
                self.wcs_outside = true;
            }
        }
    }

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
                    self.grbl_travel = [None; 3];
                    self.report_inches = false;
                    self.active_wcs = None;
                    self.wcs_reported = false;
                    self.wcs_outside = false;
                }
                MachineEvent::StatusReport(report) => {
                    // Positions, offsets and feed are kept in mm whatever `$13` says.
                    let report = (**report).clone().into_mm(self.report_inches);
                    if let Some(wco) = report.wco {
                        self.wco = Some(wco);
                    }
                    self.report = Some(report);
                }
                MachineEvent::Received(line) => {
                    if let Some(word) = parser_state_wcs(line) {
                        self.set_wcs(word);
                        self.wcs_reported = true;
                    } else if matches!(classify_response(line), GrblResponseLine::Reset(_)) {
                        // A reset restores the default system: unknown until reported
                        // again (without reports the last one stays as the fallback).
                        if self.wcs_reported {
                            self.active_wcs = None;
                        }
                    }
                    if let Some((index, travel)) = parse_travel(line) {
                        self.grbl_travel[index] = Some(travel);
                    }
                    if let Some(inches) = parse_report_inches(line)
                        && inches != self.report_inches
                    {
                        // A cached offset (and report) in the old units would be wrong:
                        // drop them and wait for the controller's next report, as GTK does.
                        self.report_inches = inches;
                        self.wco = None;
                        self.report = None;
                    }
                }
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
                MachineEvent::Sent(line) => {
                    // Lines run in order, so a later `G10 L20` acts on this system.
                    if let Some(word) = wcs_in_line(line) {
                        self.set_wcs(word);
                    }
                }
                MachineEvent::Error(message) => self.last_error = Some(message.clone()),
                MachineEvent::StreamHalted { reason } => self.last_error = Some(reason.clone()),
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

/// The controller's own message texts, for the translation check.
pub fn worker_msgids() -> Vec<&'static str> {
    worker_text::ALL.to_vec()
}

/// Translates `message` when it is one of `msgids`, alone or followed by `": "` and a
/// detail (which is kept as it is); anything else is returned unchanged.
///
/// # Arguments
/// * `message` - The English message.
/// * `msgids` - The texts it may start with.
/// * `tr` - Translator for the current UI language.
pub fn translate_prefixed(message: &str, msgids: &[&str], tr: &Translator) -> String {
    for msgid in msgids {
        if let Some(rest) = message.strip_prefix(msgid)
            && (rest.is_empty() || rest.starts_with(": "))
        {
            return format!("{}{rest}", tr.tr(msgid));
        }
    }
    message.to_string()
}

/// The text shown for a controller or worker message ([`MachineEvent::Error`],
/// [`MachineEvent::StreamHalted`]): the controller's own English texts
/// ([`worker_text`]) translated, its details and the controller's replies (`error:N`,
/// `ALARM:N`) kept as they are.
///
/// # Arguments
/// * `message` - The message as the controller reported it.
/// * `tr` - Translator for the current UI language.
pub fn worker_message(message: &str, tr: &Translator) -> String {
    let joiner = format!(" {} ", worker_text::ON_PROGRAM_LINE);
    if let Some((reply, line)) = message.split_once(&joiner) {
        return format!("{reply} {} {line}", tr.tr(worker_text::ON_PROGRAM_LINE));
    }
    translate_prefixed(message, &worker_text::ALL, tr)
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
    /// Events not yet handed to the observers (panes that keep a log), see
    /// [`Machine::take_pending`].
    pending: Vec<MachineEvent>,
    program: Option<Arc<[String]>>,
    /// Whether the controller's latest report was fresh when this frame polled it.
    report_fresh: bool,
    /// Whether every line sent had been answered just before the last poll.
    answered_at_poll: bool,
    /// How many times [`Machine::poll`] ran.
    polls: u64,
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
            pending: Vec::new(),
            program: None,
            report_fresh: true,
            answered_at_poll: true,
            polls: 0,
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

    /// Whether a program is streaming, also between a successful start and its first
    /// progress report. This is the app's one "machine busy" predicate: it covers any
    /// stream the controller runs, a probe routine's included, and the controller's halt of
    /// a stop-on-error stream (the controller streams until the halt's reset ends it).
    pub fn job_running(&self) -> bool {
        self.status.streaming || lock(&self.controller).is_streaming()
    }

    /// Takes the controller's events, replacing the previous frame's, and updates the status.
    /// Call once per frame. Never blocks on I/O.
    pub fn poll(&mut self) {
        let (fresh, answered, events) = {
            let mut controller = lock(&self.controller);
            // Freshness first: the worker marks a report fresh after sending it, so a fresh
            // report is already among the events polled next. Likewise a line's reply text
            // (`[GC:...]`) is read before its `ok`.
            (
                controller.report_fresh(),
                controller.lines_answered(),
                controller.poll(),
            )
        };
        self.report_fresh = fresh;
        self.answered_at_poll = answered;
        self.polls += 1;
        self.events = coalesce_progress(events);
        self.status.apply(&self.events);
        self.pending.extend(self.events.iter().cloned());
    }

    /// How many times [`Machine::poll`] ran; with [`Machine::lines_answered_at_poll`] a
    /// consumer tells whether a line it sent had its reply before a later poll.
    pub fn polls(&self) -> u64 {
        self.polls
    }

    /// Whether every line sent had been answered (see `MachineController::lines_answered`)
    /// just before the last poll, so the replies' text lines are among the events it took.
    pub fn lines_answered_at_poll(&self) -> bool {
        self.answered_at_poll
    }

    /// Takes the events that no observer has taken yet: those of [`Machine::poll`] and
    /// [`Machine::absorb`] since the last call. The app calls this at every point events may
    /// have arrived and hands them to every pane, drawn or not, so a pane that keeps a log
    /// (the console) misses nothing while its page is hidden.
    pub fn take_pending(&mut self) -> Vec<MachineEvent> {
        std::mem::take(&mut self.pending)
    }

    /// Whether jogging is allowed now: the last status report says Idle or Jog
    /// ([`MachineStatus::jog_allowed`]) and was read after every command line sent so far
    /// (`$H`, `$X`, an MDI line), both as of this frame's poll and right now, so a line sent
    /// since the poll blocks jogging at once. Until a fresh report arrives the last one may
    /// describe the machine before the line (Idle before homing), and a `$J` would wait in
    /// the controller's buffer and run afterwards from a stale position.
    pub fn jog_allowed(&self) -> bool {
        self.status.jog_allowed() && self.report_fresh && lock(&self.controller).report_fresh()
    }

    /// Adds `events` taken from the controller outside [`Machine::poll`] to this frame's
    /// events and the status, so no consumer misses them.
    ///
    /// # Arguments
    /// * `events` - Events polled from the controller directly.
    pub fn absorb(&mut self, events: Vec<MachineEvent>) {
        self.status.apply(&events);
        self.pending.extend(events.iter().cloned());
        self.events.extend(events);
        self.events = coalesce_progress(std::mem::take(&mut self.events));
    }

    /// The program the Start command streams, if one is loaded.
    pub fn program(&self) -> Option<&Arc<[String]>> {
        self.program.as_ref()
    }

    /// Forgets the program, so Start has nothing to stream (the editor's text is empty).
    pub fn clear_program(&mut self) {
        self.program = None;
    }

    /// Prepares `gcode` for streaming (comments removed, empty lines dropped, line length
    /// checked) and keeps it as the program to stream. The app hands the editor's text here
    /// (see `GcodeKitApp`).
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
