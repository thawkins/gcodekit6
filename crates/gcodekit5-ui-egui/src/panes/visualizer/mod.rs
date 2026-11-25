//! The Visualizer pane: the program's toolpath in 3D, drawn with wgpu.
//!
//! The toolpath is built from the program the machine streams ([`Machine::program`]) when
//! that program changes (by `Arc` identity), on a background thread, and uploaded to the GPU
//! once ([`pipeline`]); a frame only writes the camera and the stream progress. Rapids and
//! moves with the spindle/laser off are grey, cuts are coloured by power, and the lines the
//! machine has acknowledged are drawn in the accent colour. The tool marker follows the work
//! position the machine reports.
//!
//! Mouse, as in GTK: left drag orbits, Shift + left drag pans, the wheel zooms. The nav cube
//! ([`nav_cube`]) and the View commands switch to standard views, zoom and fit; the OSD
//! ([`osd`]) shows the bounds, the tool position and the mouse help.
//!
//! Ported from the GTK `ui/gtk/visualizer/mod.rs`, `renderer_3d.rs`, `shaders.rs` and
//! `nav_cube.rs`. Not ported: the 2D mode, the sidebar (show/hide rapids, cuts, grid,
//! machine bounds and tool marker; grid spacing), machine bounds (no device profiles in the
//! egui front end yet), stock removal simulation, run preview, scrollbars, fit to device,
//! intensity statistics and depth testing.
//!
//! [`Machine::program`]: crate::machine::Machine::program
//!
//! Dependencies: `egui`, `egui-wgpu`, `glam`, `gcodekit5-visualizer` (parser, camera),
//! `gcodekit5-app` (machine events, translations), `gcodekit5-core` (units).

pub mod camera;
pub mod nav_cube;
pub mod osd;
pub mod pipeline;
pub mod toolpath;

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::MachineEvent;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_settings::config::Config;
use glam::Vec3;

pub use camera::{Aabb, Camera, StdView};
pub use nav_cube::NavAction;
pub use pipeline::install;
use pipeline::{UploadedMesh, VisualizerCallback};
pub use toolpath::ToolpathMesh;

use crate::commands::Command;
use crate::workspace::{Pane, PaneContext, PaneId};

/// Millimetres the Zoom In / Zoom Out buttons and commands move the eye (GTK's step).
pub const ZOOM_STEP: f32 = 10.0;
/// Millimetres of zoom per point of wheel scroll (GTK: 5 mm per wheel notch).
const ZOOM_PER_SCROLL_POINT: f32 = 0.1;

/// The msgids the pane shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    nav_cube::msgids()
        .into_iter()
        .chain(osd::msgids())
        .collect()
}

/// A toolpath being built on a background thread.
#[derive(Debug)]
struct PendingBuild {
    generation: u64,
    result: Receiver<ToolpathMesh>,
}

/// The Visualizer pane.
#[derive(Debug)]
pub struct VisualizerPane {
    camera: Camera,
    /// The program the toolpath is (being) built from.
    program: Option<Arc<[String]>>,
    /// The built toolpath.
    mesh: Option<UploadedMesh>,
    /// The build in progress.
    pending: Option<PendingBuild>,
    /// Counts programs loaded, to tag each mesh.
    generation: u64,
    /// Program lines the machine acknowledged in the current or last stream.
    progress: u32,
    units: MeasurementSystem,
}

impl Default for VisualizerPane {
    fn default() -> Self {
        Self::new()
    }
}

impl VisualizerPane {
    /// An empty visualizer.
    pub fn new() -> Self {
        Self {
            camera: Camera::default(),
            program: None,
            mesh: None,
            pending: None,
            generation: 0,
            progress: 0,
            units: MeasurementSystem::Metric,
        }
    }

    /// The camera.
    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    /// The camera, to move it.
    pub fn camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }

    /// The built toolpath, once the current program's build has finished.
    pub fn mesh(&self) -> Option<&ToolpathMesh> {
        self.mesh.as_ref().map(|m| m.mesh.as_ref())
    }

    /// Whether a toolpath is still being built.
    pub fn is_building(&self) -> bool {
        self.pending.is_some()
    }

    /// Program lines acknowledged in the current or last stream.
    pub fn progress(&self) -> u32 {
        self.progress
    }

    /// Shows `program` (the program as the machine streams it), building its toolpath on a
    /// background thread when it is not the program already shown. `None` clears the view.
    ///
    /// # Arguments
    /// * `program` - The program's lines, or `None`.
    /// * `repaint` - Context to wake when the build finishes.
    pub fn load_program(&mut self, program: Option<Arc<[String]>>, repaint: &egui::Context) {
        let same = match (&self.program, &program) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        };
        if same {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        self.progress = 0;
        self.program = program.clone();
        self.pending = None;
        let Some(lines) = program else {
            self.mesh = None;
            return;
        };
        let (sender, result) = std::sync::mpsc::channel();
        let ctx = repaint.clone();
        let spawned = std::thread::Builder::new()
            .name("gcodekit5-toolpath".into())
            .spawn(move || {
                if sender.send(toolpath::build(&lines)).is_ok() {
                    ctx.request_repaint();
                } else {
                    // The receiver is dropped only when a newer program replaced this one.
                    tracing::debug!("toolpath of a replaced program discarded");
                }
            });
        if let Err(error) = spawned {
            tracing::warn!(%error, "toolpath thread not started; building on the UI thread");
            if let Some(lines) = &self.program {
                self.set_mesh(toolpath::build(lines));
            }
            return;
        }
        self.pending = Some(PendingBuild {
            generation: self.generation,
            result,
        });
    }

    /// Takes the finished build, if any.
    fn poll_build(&mut self) {
        let Some(pending) = &self.pending else {
            return;
        };
        match pending.result.try_recv() {
            Ok(mesh) => {
                let generation = pending.generation;
                self.pending = None;
                if generation == self.generation {
                    self.set_mesh(mesh);
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                tracing::warn!("toolpath build ended without a result");
                self.pending = None;
            }
        }
    }

    /// Shows `mesh` and fits it into an isometric view, as GTK does on load.
    fn set_mesh(&mut self, mesh: ToolpathMesh) {
        if let Some(bounds) = mesh.bounds {
            self.camera.view(StdView::IsoSe);
            self.camera.fit(bounds);
        }
        self.mesh = Some(UploadedMesh {
            generation: self.generation,
            mesh: Arc::new(mesh),
        });
    }

    /// Fits the toolpath into the view, or resets the view when there is none.
    fn fit(&mut self) {
        match self.mesh().and_then(|m| m.bounds) {
            Some(bounds) => self.camera.fit(bounds),
            None => self.camera.reset(),
        }
    }

    /// Runs a nav cube action.
    ///
    /// # Arguments
    /// * `action` - What the user clicked.
    pub fn apply(&mut self, action: NavAction) {
        match action {
            NavAction::View(view) => self.camera.view(view),
            NavAction::ZoomIn => self.camera.zoom(ZOOM_STEP),
            NavAction::ZoomOut => self.camera.zoom(-ZOOM_STEP),
            NavAction::Fit => self.fit(),
        }
    }

    /// Orbits, pans and zooms with the mouse over `response`.
    fn handle_input(&mut self, ui: &egui::Ui, response: &egui::Response) {
        if response.dragged_by(egui::PointerButton::Primary) {
            let delta = response.drag_delta();
            if ui.input(|i| i.modifiers.shift) {
                self.camera.pan(delta.x, delta.y);
            } else {
                self.camera.orbit(delta.x, delta.y);
            }
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                // GTK zooms in as the wheel turns toward the user.
                self.camera.zoom(-scroll * ZOOM_PER_SCROLL_POINT);
            }
        }
    }
}

impl Pane for VisualizerPane {
    fn id(&self) -> PaneId {
        PaneId::Visualizer
    }

    fn title(&self, tr: &Translator) -> String {
        tr.tr(PaneId::Visualizer.title_msgid()).into_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext) {
        let mut tool = None;
        if let Some(machine) = ctx.machine() {
            self.load_program(machine.program().cloned(), ui.ctx());
            let status = machine.status();
            tool = status.work_position().filter(|_| status.connected);
        }
        self.poll_build();
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        self.camera
            .set_aspect(rect.width() / rect.height().max(1.0));
        self.handle_input(ui, &response);

        let dark = ui.visuals().dark_mode;
        let background = if dark {
            crate::theme::BACKGROUND_DARK
        } else {
            crate::theme::BACKGROUND_LIGHT
        };
        ui.painter().rect_filled(rect, 0.0, background);
        let mode = if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        let callback = VisualizerCallback {
            view_proj: self.camera.view_proj(rect.width() / rect.height().max(1.0)),
            progress: self.progress,
            done_color: srgb_floats(crate::theme::palette(mode).accent),
            mesh: self.mesh.clone(),
            tool: tool.map(Vec3::from_array),
        };
        ui.painter()
            .add(egui_wgpu::Callback::new_paint_callback(rect, callback));

        if let Some(action) = nav_cube::show(ui, rect, ctx.tr) {
            self.apply(action);
        }
        let loaded = self.program.is_some();
        let bounds = self.mesh().and_then(|m| m.cut_bounds.or(m.bounds));
        osd::show(ui, rect, loaded, bounds.as_ref(), tool, self.units, ctx.tr);
    }

    fn command(&mut self, command: Command, _ctx: &mut PaneContext) -> bool {
        let action = match command {
            Command::ZoomFit => NavAction::Fit,
            Command::ZoomIn => NavAction::ZoomIn,
            Command::ZoomOut => NavAction::ZoomOut,
            Command::ViewTop => NavAction::View(StdView::Top),
            Command::ViewFront => NavAction::View(StdView::Front),
            Command::ViewIso => NavAction::View(StdView::IsoSe),
            Command::ResetView => {
                self.camera.reset();
                self.fit();
                return true;
            }
            Command::FileNew
            | Command::FileNew3d
            | Command::FileOpen
            | Command::FileSave
            | Command::FileSaveAs
            | Command::FileImport
            | Command::FileImportImage
            | Command::FileExportGcode
            | Command::FileExportSvg
            | Command::FileRun
            | Command::Quit
            | Command::Undo
            | Command::Redo
            | Command::Cut
            | Command::Copy
            | Command::Paste
            | Command::Connect
            | Command::Disconnect
            | Command::StreamStart
            | Command::StreamPause
            | Command::StreamStop
            | Command::FeedHold
            | Command::CycleStart
            | Command::SoftReset
            | Command::Home
            | Command::Unlock
            | Command::JogXPos
            | Command::JogXNeg
            | Command::JogYPos
            | Command::JogYNeg
            | Command::JogZPos
            | Command::JogZNeg
            | Command::OpenSettings
            | Command::OpenDeviceManager
            | Command::OpenToolsManager
            | Command::OpenMaterialsManager
            | Command::OpenHelp
            | Command::About => return false,
        };
        self.apply(action);
        true
    }

    fn configure(&mut self, config: &Config) {
        self.units = config.ui.measurement_system;
    }

    fn machine_events(&mut self, events: &[MachineEvent], _tr: &Translator) {
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        for event in events {
            match event {
                MachineEvent::StreamProgress { acked, .. }
                | MachineEvent::StreamStopped { acked, .. } => self.progress = count(*acked),
                MachineEvent::StreamFinished { lines } => self.progress = count(*lines),
                MachineEvent::Connected
                | MachineEvent::Disconnected
                | MachineEvent::StatusReport(_)
                | MachineEvent::Received(_)
                | MachineEvent::Sent(_)
                | MachineEvent::Error(_) => {}
            }
        }
    }
}

/// `color` as opaque sRGB floats.
fn srgb_floats(color: egui::Color32) -> [f32; 4] {
    let [r, g, b, _] = color.to_array();
    [
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        1.0,
    ]
}
