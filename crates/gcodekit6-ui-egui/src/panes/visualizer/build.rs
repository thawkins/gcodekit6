//! Background toolpath builds: one worker thread, and only the latest program is built.
//!
//! The editor hands the machine a new program after every typing pause, and parsing a
//! million-line program takes about half a second, so builds must not pile up. Requests go
//! into a single slot that a newer request overwrites; the worker takes the slot, and checks
//! the latest generation before and after each phase (comparing with the previous program,
//! parsing), dropping work that a newer request has made stale. At most one build runs, and
//! at most one waits.
//!
//! Each build also says whether the program replaces the one shown (a newly opened file)
//! rather than editing it ([`is_content_replacement`]), so the pane refits
//! the view only for a new program.
//!
//! Dependencies: `egui` (repaint), `gcodekit6-visualizer` (replacement rule).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, PoisonError};

use gcodekit6_visualizer::visualizer::is_content_replacement;

use super::toolpath::{self, ToolpathMesh};

/// A finished build.
#[derive(Debug)]
pub struct Built {
    /// The generation it was requested with.
    pub generation: u64,
    /// The toolpath.
    pub mesh: ToolpathMesh,
    /// Whether the program replaces the previous one (a new file) rather than editing it.
    pub replaced: bool,
}

/// What the worker reports for a request.
#[derive(Debug)]
pub enum Outcome {
    /// The build finished.
    Done(Built),
    /// The build panicked; the worker is still alive.
    Failed {
        /// The generation that was being built.
        generation: u64,
    },
}

/// Test hook: when set, the next build panics (once), to exercise the worker's recovery.
#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
pub static PANIC_NEXT_BUILD: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// A build to run.
struct Request {
    generation: u64,
    lines: Arc<[String]>,
    previous: Option<Arc<[String]>>,
    repaint: egui::Context,
    /// The longest arc chord (mm) to draw with.
    arc_segment_length: f64,
}

/// The waiting request, and whether the worker should stop.
#[derive(Default)]
struct Slot {
    request: Option<Request>,
    closed: bool,
}

/// State shared with the worker.
#[derive(Default)]
struct Shared {
    slot: Mutex<Slot>,
    wake: Condvar,
    /// The generation of the newest request (or cancellation).
    latest: AtomicU64,
}

impl Shared {
    fn is_current(&self, generation: u64) -> bool {
        self.latest.load(Ordering::Acquire) == generation
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Slot> {
        self.slot.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Runs `request` unless it goes stale; checked between phases.
fn run(shared: &Shared, request: Request) -> Option<Built> {
    let Request {
        generation,
        lines,
        previous,
        repaint: _,
        arc_segment_length,
    } = request;
    let replaced = match previous {
        Some(previous) => {
            shared.is_current(generation).then_some(())?;
            is_content_replacement(&previous.join("\n"), &lines.join("\n"))
        }
        None => true,
    };
    shared.is_current(generation).then_some(())?;
    #[cfg(any(test, feature = "test-support"))]
    if PANIC_NEXT_BUILD.swap(false, Ordering::AcqRel) {
        panic!("toolpath build panic requested by test hook");
    }
    let mesh = toolpath::build_with(&lines, arc_segment_length);
    shared.is_current(generation).then_some(())?;
    Some(Built {
        generation,
        mesh,
        replaced,
    })
}

/// Runs `request` like [`run`], turning a panic into [`Outcome::Failed`] and logging it, so a
/// bad program cannot kill the worker.
fn run_guarded(shared: &Shared, request: Request) -> Option<Outcome> {
    let generation = request.generation;
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(shared, request))) {
        Ok(built) => built.map(Outcome::Done),
        Err(_) => {
            tracing::error!(
                generation,
                "toolpath build panicked; the worker keeps running"
            );
            Some(Outcome::Failed { generation })
        }
    }
}

/// The worker's loop: wait for a request, build it, send it, until closed.
fn work(shared: &Shared, results: &Sender<Outcome>) {
    loop {
        let request = {
            let mut slot = shared.lock();
            loop {
                if slot.closed {
                    return;
                }
                if let Some(request) = slot.request.take() {
                    break request;
                }
                slot = shared
                    .wake
                    .wait(slot)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        };
        let repaint = request.repaint.clone();
        if let Some(built) = run_guarded(shared, request) {
            if results.send(built).is_err() {
                return;
            }
            repaint.request_repaint();
        }
    }
}

/// Builds toolpaths on a background thread, latest request first.
pub struct BuildWorker {
    shared: Arc<Shared>,
    results: Receiver<Outcome>,
    /// Set when the thread could not start: builds then run on the caller's thread.
    inline: Option<Sender<Outcome>>,
    /// The arc segment length (mm) later requests build with.
    arc_segment_length: f64,
}

impl std::fmt::Debug for BuildWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuildWorker")
            .field("latest", &self.shared.latest.load(Ordering::Relaxed))
            .field("inline", &self.inline.is_some())
            .finish_non_exhaustive()
    }
}

impl Default for BuildWorker {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildWorker {
    /// Starts the worker thread (or, if it cannot start, builds on the caller's thread).
    pub fn new() -> Self {
        let shared = Arc::new(Shared::default());
        let (sender, results) = std::sync::mpsc::channel();
        let worker_shared = Arc::clone(&shared);
        let worker_sender = sender.clone();
        let spawned = std::thread::Builder::new()
            .name("gcodekit6-toolpath".into())
            .spawn(move || work(&worker_shared, &worker_sender));
        let inline = match spawned {
            Ok(_) => None,
            Err(error) => {
                tracing::warn!(%error, "toolpath thread not started; building on the UI thread");
                Some(sender)
            }
        };
        Self {
            shared,
            results,
            inline,
            arc_segment_length: toolpath::DEFAULT_ARC_SEGMENT_LENGTH,
        }
    }

    /// Sets the Preferences "Arc Segment Length" (mm) that later requests build with.
    ///
    /// # Arguments
    /// * `mm` - The longest arc chord; see [`toolpath::arc_segment_count`].
    pub fn set_arc_segment_length(&mut self, mm: f64) {
        self.arc_segment_length = mm;
    }

    /// Asks for the toolpath of `lines`, replacing any waiting request and making running
    /// work stale.
    ///
    /// # Arguments
    /// * `generation` - Tags the result; newer requests must use a different value.
    /// * `lines` - The program as streamed.
    /// * `previous` - The program shown now, to tell a new file from an edit.
    /// * `repaint` - Context to wake when the build is done.
    pub fn request(
        &self,
        generation: u64,
        lines: Arc<[String]>,
        previous: Option<Arc<[String]>>,
        repaint: &egui::Context,
    ) {
        self.shared.latest.store(generation, Ordering::Release);
        let request = Request {
            generation,
            lines,
            previous,
            repaint: repaint.clone(),
            arc_segment_length: self.arc_segment_length,
        };
        if let Some(sender) = &self.inline {
            if let Some(built) = run_guarded(&self.shared, request)
                && sender.send(built).is_err()
            {
                tracing::debug!("toolpath result dropped");
            }
            return;
        }
        self.shared.lock().request = Some(request);
        self.shared.wake.notify_one();
    }

    /// Drops any waiting or running build: nothing older than `generation` is delivered.
    ///
    /// # Arguments
    /// * `generation` - The new generation (no build is requested with it).
    pub fn cancel(&self, generation: u64) {
        self.shared.latest.store(generation, Ordering::Release);
        self.shared.lock().request = None;
    }

    /// The finished build of the latest request, if it is done; stale results are dropped.
    pub fn take(&self) -> Option<Outcome> {
        let latest = self.shared.latest.load(Ordering::Acquire);
        self.results
            .try_iter()
            .filter(|outcome| match outcome {
                Outcome::Done(built) => built.generation == latest,
                Outcome::Failed { generation } => *generation == latest,
            })
            .last()
    }
}

impl Drop for BuildWorker {
    fn drop(&mut self) {
        self.shared.lock().closed = true;
        self.shared.wake.notify_all();
    }
}
