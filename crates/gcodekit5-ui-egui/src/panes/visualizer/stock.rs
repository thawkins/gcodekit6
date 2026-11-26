//! The 3D stock removal simulation: cuts a height map of the stock with the program's
//! toolpath and meshes what is left, on a worker thread, so neither the allocation, the
//! cutting nor the meshing blocks the UI or the machine stream.
//!
//! Ported from the GTK `ui/gtk/visualizer/mod.rs` (`run_stock_simulation`,
//! `restart_stock_simulation` and the "Show Stock Removal" toggle). Each run has its own
//! cancel token: [`StockSimulation::start`] cancels the running one and drops its result, so
//! a stale run can never overwrite a newer one. The worker sends whole-percent progress and
//! the mesh, and wakes the UI with a repaint request. The stock and tool are checked on the
//! UI thread first ([`StockSimulator3D::validate`], which allocates nothing).
//!
//! The simulation itself is the toolkit-free `gcodekit5-visualizer` `stock_removal_3d`
//! (height map, flat end mill, greedy surface mesh), used unchanged.
//!
//! Dependencies: `egui` (repaint), `gcodekit5-visualizer` (parser, simulation),
//! `gcodekit5-app` (translations).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};

use gcodekit5_app::i18n::Translator;
use gcodekit5_visualizer::Visualizer;
use gcodekit5_visualizer::visualizer::stock_removal_3d::MESH_FLOATS_PER_VERTEX;
use gcodekit5_visualizer::visualizer::{
    StockSimulator3D, StockXyReference, StockZeroReference, detect_stock_zero_reference,
    toolpath_from_commands,
};

/// Cell size of the simulation in mm (GTK's).
pub const RESOLUTION: f32 = 0.19;

/// The msgids the simulation shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Cannot simulate stock removal: {reason}"),
        n_!("Nothing to simulate: the program has no moves."),
        n_!("Stock removal simulation failed."),
        n_!("Stock removal too large to display on this GPU."),
    ]
}

/// The stock and tool a run simulates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StockSetup {
    /// Stock size in X (mm).
    pub width: f32,
    /// Stock size in Y (mm).
    pub height: f32,
    /// Stock size in Z (mm).
    pub thickness: f32,
    /// Flat end mill diameter (mm).
    pub tool_diameter: f32,
    /// Which stock face is work Z0.
    pub zero: StockZeroReference,
    /// Where on the stock work X0 Y0 is.
    pub xy: StockXyReference,
}

impl Default for StockSetup {
    /// GTK's defaults: 200 x 200 x 10 mm stock and a 1/8" (3.175 mm) end mill, zeroed on the
    /// stock's top lower-left corner.
    fn default() -> Self {
        Self {
            width: 200.0,
            height: 200.0,
            thickness: 10.0,
            tool_diameter: 3.175,
            zero: StockZeroReference::default(),
            xy: StockXyReference::default(),
        }
    }
}

impl StockSetup {
    /// Checks the stock and tool without allocating.
    ///
    /// # Errors
    /// The simulator's message when a size is not a positive number or the stock is too large
    /// to simulate at [`RESOLUTION`].
    pub fn validate(&self) -> Result<(), String> {
        StockSimulator3D::validate(
            self.width,
            self.height,
            self.thickness,
            RESOLUTION,
            self.tool_diameter / 2.0,
        )
    }
}

/// Why the simulation shows no stock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// The stock or tool is invalid; the simulator's (English) reason.
    Invalid(String),
    /// The program has no moves (or no program is loaded).
    NoMoves,
    /// The worker stopped without a result.
    Failed,
    /// The mesh is larger than the GPU's buffer limit.
    TooLarge,
}

impl Notice {
    /// The notice in the UI language.
    ///
    /// # Arguments
    /// * `tr` - Translator for the current UI language.
    pub fn text(&self, tr: &Translator) -> String {
        match self {
            Self::Invalid(reason) => tr
                .tr("Cannot simulate stock removal: {reason}")
                .replace("{reason}", reason),
            Self::NoMoves => tr
                .tr("Nothing to simulate: the program has no moves.")
                .into_owned(),
            Self::Failed => tr.tr("Stock removal simulation failed.").into_owned(),
            Self::TooLarge => tr
                .tr("Stock removal too large to display on this GPU.")
                .into_owned(),
        }
    }
}

/// The stock mesh's floats waiting for the GPU (see [`super::pipeline::Upload`]).
pub type MeshUpload = super::pipeline::Upload<f32>;

/// A finished stock mesh, tagged with its run so it is uploaded once.
#[derive(Debug, Clone)]
pub struct StockMesh {
    /// Changes with every run.
    pub generation: u64,
    /// Number of vertices.
    pub vertices: usize,
    /// The mesh ([`gcodekit5_visualizer::visualizer::stock_removal_3d::MESH_FLOATS_PER_VERTEX`]
    /// floats per vertex: position, normal, colour), until uploaded.
    pub upload: Arc<MeshUpload>,
}

/// What the worker sends.
enum Message {
    /// Whole-percent progress, sent when it changes.
    Progress(u8),
    /// The remaining stock's surface mesh in work coordinates.
    Done(Vec<f32>),
    /// The program has no moves.
    NoMoves,
    /// The simulator refused the stock (checked on the UI thread, so unexpected).
    Invalid(String),
}

/// A running simulation.
struct Run {
    cancel: Arc<AtomicBool>,
    results: Receiver<Message>,
}

/// Header lines (comments and blanks) of `lines` and the first command, joined: all that
/// [`detect_stock_zero_reference`] reads, without joining a whole program on the UI thread.
fn header(lines: &[String]) -> String {
    let mut text = String::new();
    for line in lines {
        text.push_str(line);
        text.push('\n');
        let trimmed = line.trim();
        let comment = trimmed.is_empty()
            || trimmed == "%"
            || trimmed.starts_with(';')
            || trimmed.starts_with('(');
        if !comment {
            break;
        }
    }
    text
}

/// The stock Z0 convention a program records in its header, if any.
///
/// # Arguments
/// * `lines` - The program's lines.
pub fn recorded_zero(lines: &[String]) -> Option<StockZeroReference> {
    detect_stock_zero_reference(&header(lines))
}

/// Runs a simulation of `lines` on `setup` and sends its progress and mesh. Runs on the
/// worker; sends nothing more once `cancel` is set.
fn simulate(
    setup: StockSetup,
    lines: &[String],
    cancel: &AtomicBool,
    gate: &AtomicBool,
    tx: &Sender<Message>,
    repaint: &egui::Context,
) {
    let cancelled = || cancel.load(Ordering::Acquire);
    while gate.load(Ordering::Acquire) && !cancelled() {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let send = |message: Message| {
        if !cancelled() && tx.send(message).is_ok() {
            repaint.request_repaint();
        }
    };
    let mut vis = Visualizer::new();
    vis.parse_gcode(&lines.join("\n"));
    let toolpath = toolpath_from_commands(vis.commands());
    drop(vis);
    if toolpath.is_empty() {
        send(Message::NoMoves);
        return;
    }
    let simulator = StockSimulator3D::new(
        setup.width,
        setup.height,
        setup.thickness,
        RESOLUTION,
        setup.tool_diameter / 2.0,
    );
    let mut simulator = match simulator {
        Ok(s) => s
            .with_stock_zero_reference(setup.zero)
            .with_stock_xy_reference(setup.xy),
        Err(reason) => {
            send(Message::Invalid(reason));
            return;
        }
    };
    let mut last = u8::MAX;
    let completed = simulator.simulate_toolpath_with_progress(&toolpath, |fraction| {
        let pct = (fraction * 100.0).round().clamp(0.0, 100.0) as u8;
        if pct != last {
            last = pct;
            send(Message::Progress(pct));
        }
        !cancelled()
    });
    if completed && let Some(mesh) = simulator.get_mesh_cancellable(&cancelled) {
        send(Message::Done(mesh));
    }
}

/// The stock removal simulation of the visualizer: at most one run, and its result.
pub struct StockSimulation {
    run: Option<Run>,
    /// Progress of the running simulation, in percent.
    progress: u8,
    mesh: Option<StockMesh>,
    notice: Option<Notice>,
    generation: u64,
    /// Test hook: while set, a new run waits before it starts simulating.
    gate: Arc<AtomicBool>,
}

impl std::fmt::Debug for StockSimulation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StockSimulation")
            .field("running", &self.run.is_some())
            .field("progress", &self.progress)
            .field("mesh", &self.mesh)
            .field("notice", &self.notice)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl Default for StockSimulation {
    fn default() -> Self {
        Self::new()
    }
}

impl StockSimulation {
    /// No simulation.
    pub fn new() -> Self {
        Self {
            run: None,
            progress: 0,
            mesh: None,
            notice: None,
            generation: 0,
            gate: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Cancels the running simulation, if any, and drops the result and notice.
    pub fn stop(&mut self) {
        if let Some(run) = self.run.take() {
            run.cancel.store(true, Ordering::Release);
        }
        self.generation = self.generation.wrapping_add(1);
        self.progress = 0;
        self.mesh = None;
        self.notice = None;
    }

    /// Starts simulating `program` on `setup`, cancelling any running simulation (GTK's
    /// restart). An invalid stock or no program gives a notice instead of a run.
    ///
    /// # Arguments
    /// * `setup` - The stock and tool.
    /// * `program` - The program's lines, as the machine streams them.
    /// * `repaint` - Context to wake on progress and when the mesh is ready.
    pub fn start(
        &mut self,
        setup: StockSetup,
        program: Option<Arc<[String]>>,
        repaint: &egui::Context,
    ) {
        self.stop();
        if let Err(reason) = setup.validate() {
            tracing::warn!("Stock simulation not started: {reason}");
            self.notice = Some(Notice::Invalid(reason));
            return;
        }
        let Some(lines) = program else {
            self.notice = Some(Notice::NoMoves);
            return;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, results) = std::sync::mpsc::channel();
        let worker_cancel = Arc::clone(&cancel);
        let gate = Arc::clone(&self.gate);
        let repaint = repaint.clone();
        let spawned = std::thread::Builder::new()
            .name("gcodekit5-stock-sim".into())
            .spawn(move || simulate(setup, &lines, &worker_cancel, &gate, &tx, &repaint));
        match spawned {
            Ok(_) => self.run = Some(Run { cancel, results }),
            Err(error) => {
                tracing::error!(%error, "stock simulation thread not started");
                self.notice = Some(Notice::Failed);
            }
        }
    }

    /// Takes the worker's progress and result, if any.
    pub fn poll(&mut self) {
        let Some(run) = &self.run else {
            return;
        };
        loop {
            match run.results.try_recv() {
                Ok(Message::Progress(pct)) => self.progress = pct,
                Ok(Message::Done(mesh)) => {
                    let vertices = mesh.len() / MESH_FLOATS_PER_VERTEX;
                    self.mesh = Some(StockMesh {
                        generation: self.generation,
                        vertices,
                        upload: Arc::new(MeshUpload::new(mesh)),
                    });
                    self.run = None;
                    return;
                }
                Ok(Message::NoMoves) => {
                    self.notice = Some(Notice::NoMoves);
                    self.run = None;
                    return;
                }
                Ok(Message::Invalid(reason)) => {
                    tracing::warn!("Stock simulation failed: {reason}");
                    self.notice = Some(Notice::Invalid(reason));
                    self.run = None;
                    return;
                }
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    tracing::error!("Stock simulation stopped without a result");
                    self.notice = Some(Notice::Failed);
                    self.run = None;
                    return;
                }
            }
        }
    }

    /// Whether a simulation is running.
    pub fn is_running(&self) -> bool {
        self.run.is_some()
    }

    /// The running simulation's progress in percent, or `None` when none runs.
    pub fn progress(&self) -> Option<u8> {
        self.run.as_ref().map(|_| self.progress)
    }

    /// The finished mesh, if any.
    pub fn mesh(&self) -> Option<&StockMesh> {
        self.mesh.as_ref()
    }

    /// Why no stock is shown, if a run ended without one or its mesh is too large to draw.
    pub fn notice(&self) -> Option<Notice> {
        let too_large = self
            .mesh
            .as_ref()
            .is_some_and(|mesh| mesh.upload.too_large());
        if too_large {
            Some(Notice::TooLarge)
        } else {
            self.notice.clone()
        }
    }

    /// Test hook: while `paused`, new runs wait before simulating, so a test can see a run in
    /// progress.
    #[doc(hidden)]
    #[cfg(feature = "test-support")]
    pub fn pause_worker(&self, paused: bool) {
        self.gate.store(paused, Ordering::Release);
    }
}

impl Drop for StockSimulation {
    fn drop(&mut self) {
        if let Some(run) = self.run.take() {
            run.cancel.store(true, Ordering::Release);
        }
    }
}
