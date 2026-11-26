//! The Visualizer pane: the program's toolpath in 3D, drawn with wgpu.
//!
//! The toolpath is built from the program the machine streams ([`Machine::program`]) when
//! that program changes (by `Arc` identity), on one background worker that builds only the
//! latest program ([`build`]), and uploaded to the GPU once ([`pipeline`]); a frame only
//! writes the camera and the stream progress. A newly opened program is fitted into an
//! isometric view; an edit of the shown program keeps the user's view (GTK's rule). Rapids and
//! moves with the spindle/laser off are grey, cuts are coloured by power, and the lines the
//! machine has acknowledged are drawn in the accent colour. The tool marker follows the work
//! position the machine reports.
//!
//! Mouse, as in GTK: left drag orbits, Shift + left drag pans, the wheel zooms. The nav cube
//! ([`nav_cube`]) and the View commands switch to standard views, zoom and fit; the OSD
//! ([`osd`]) shows the bounds and the tool position. The left sidebar ([`sidebar`]) has
//! GTK's toolbar (Fit to Content, Reset View, Hide) and Inspector.
//!
//! With the `enable_stock_removal_3d` setting on, the sidebar's Simulation section shows
//! GTK's "Show Stock Removal": the stock is cut by the program on a worker ([`stock`]) and
//! drawn lit and depth-tested with the toolpath. The run restarts when the program or the
//! stock changes, and stops when the toggle or the setting is turned off.
//!
//! Ported from the GTK `ui/gtk/visualizer/mod.rs`, `renderer_3d.rs`, `shaders.rs` and
//! `nav_cube.rs`. Not ported: the 2D mode, the sidebar's toggles (show/hide rapids, cuts,
//! grid, machine bounds and tool marker; grid spacing), machine bounds (no device profiles
//! in the egui front end yet), run preview, scrollbars, fit to device, the zoom/centre/cursor
//! status line, intensity statistics, and copying the Designer's stock into the simulation
//! (the stock is edited in the sidebar instead).
//!
//! [`Machine::program`]: crate::machine::Machine::program
//!
//! Dependencies: `egui`, `egui-wgpu`, `glam`, `gcodekit5-visualizer` (parser, camera),
//! `gcodekit5-app` (machine events, translations), `gcodekit5-core` (units).

pub mod build;
pub mod camera;
pub mod nav_cube;
pub mod osd;
pub mod pipeline;
pub mod sidebar;
pub mod stock;
pub mod toolpath;

use std::sync::Arc;

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::MachineEvent;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_settings::config::Config;
use glam::Vec3;

use build::{BuildWorker, Outcome};
pub use camera::{Aabb, Camera, StdView};
use gcodekit5_visualizer::visualizer::StockZeroReference;
pub use nav_cube::NavAction;
pub use pipeline::install;
use pipeline::{Upload, UploadedMesh, VisualizerCallback};
pub use stock::{StockSetup, StockSimulation};
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
        .chain(sidebar::msgids())
        .chain(stock::msgids())
        .collect()
}

/// The Visualizer pane.
#[derive(Debug)]
pub struct VisualizerPane {
    camera: Camera,
    /// The program the toolpath is (being) built from.
    program: Option<Arc<[String]>>,
    /// The program of the toolpath shown, to tell a newly opened file from an edit.
    applied: Option<Arc<[String]>>,
    /// The toolpath shown, until and after its upload.
    mesh: Option<UploadedMesh>,
    /// Bounds of every move of the toolpath shown.
    bounds: Option<Aabb>,
    /// Bounds of its cutting moves.
    cut_bounds: Option<Aabb>,
    worker: BuildWorker,
    /// A build for `generation` was requested and has not arrived.
    building: bool,
    /// The last build failed, so nothing is drawn for the current program.
    build_failed: bool,
    /// Counts programs loaded, to tag each mesh.
    generation: u64,
    /// Program lines the machine acknowledged in the current or last stream.
    progress: u32,
    units: MeasurementSystem,
    /// The view's rectangle on the last frame.
    view_rect: egui::Rect,
    /// Whether the user hid the sidebar.
    sidebar_hidden: bool,
    /// The `enable_stock_removal_3d` setting.
    stock_enabled: bool,
    /// Whether "Show Stock Removal" is checked.
    show_stock: bool,
    /// The stock and tool simulated.
    stock_setup: StockSetup,
    /// The user's last Stock Z0 choice, used when a program records none.
    stock_zero_choice: StockZeroReference,
    stock: StockSimulation,
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
            applied: None,
            mesh: None,
            bounds: None,
            cut_bounds: None,
            worker: BuildWorker::new(),
            building: false,
            build_failed: false,
            generation: 0,
            progress: 0,
            units: MeasurementSystem::Metric,
            view_rect: egui::Rect::NOTHING,
            sidebar_hidden: false,
            stock_enabled: false,
            show_stock: false,
            stock_setup: StockSetup::default(),
            stock_zero_choice: StockZeroReference::default(),
            stock: StockSimulation::new(),
        }
    }

    /// The stock removal simulation.
    pub fn stock(&self) -> &StockSimulation {
        &self.stock
    }

    /// Whether "Show Stock Removal" is checked.
    pub fn stock_shown(&self) -> bool {
        self.show_stock
    }

    /// The stock and tool simulated.
    pub fn stock_setup(&self) -> StockSetup {
        self.stock_setup
    }

    /// Checks or unchecks "Show Stock Removal" (only while the setting is on), starting or
    /// stopping the simulation.
    ///
    /// # Arguments
    /// * `show` - Whether to show the stock.
    /// * `repaint` - Context to wake on progress.
    pub fn set_stock_shown(&mut self, show: bool, repaint: &egui::Context) {
        let show = show && self.stock_enabled;
        if show == self.show_stock {
            return;
        }
        self.show_stock = show;
        self.restart_stock(repaint);
    }

    /// Changes the stock and tool, restarting a shown simulation.
    ///
    /// # Arguments
    /// * `setup` - The new stock and tool.
    /// * `repaint` - Context to wake on progress.
    pub fn set_stock_setup(&mut self, setup: StockSetup, repaint: &egui::Context) {
        if setup == self.stock_setup {
            return;
        }
        if setup.zero != self.stock_setup.zero {
            self.stock_zero_choice = setup.zero;
        }
        self.stock_setup = setup;
        self.restart_stock(repaint);
    }

    /// Starts the simulation afresh when shown (GTK's `restart_stock_simulation`), else
    /// stops it.
    fn restart_stock(&mut self, repaint: &egui::Context) {
        if self.show_stock {
            self.stock
                .start(self.stock_setup, self.program.clone(), repaint);
        } else {
            self.stock.stop();
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

    /// The generation of the toolpath shown, once the current program's build has finished;
    /// it changes only when a different program is loaded.
    pub fn toolpath_generation(&self) -> Option<u64> {
        self.mesh.as_ref().map(|m| m.generation)
    }

    /// The bounds of every move of the toolpath shown.
    pub fn bounds(&self) -> Option<Aabb> {
        self.bounds
    }

    /// Whether the toolpath's vertices are still held on the CPU (not uploaded yet).
    pub fn toolpath_on_cpu(&self) -> bool {
        self.mesh.as_ref().is_some_and(|m| m.upload.pending())
    }

    /// Whether a toolpath is still being built.
    pub fn is_building(&self) -> bool {
        self.building
    }

    /// Whether the build of the current program failed (the OSD says so, and no toolpath of
    /// an earlier program is left on screen).
    pub fn build_failed(&self) -> bool {
        self.build_failed
    }

    /// Program lines acknowledged in the current or last stream.
    pub fn progress(&self) -> u32 {
        self.progress
    }

    /// The view's rectangle on the last frame.
    pub fn view_rect(&self) -> egui::Rect {
        self.view_rect
    }

    /// Whether the sidebar is hidden.
    pub fn sidebar_hidden(&self) -> bool {
        self.sidebar_hidden
    }

    /// Shows `program` (the program as the machine streams it), building its toolpath on the
    /// background worker when it is not the program already shown. `None` clears the view.
    /// The view is refitted only when the program replaces the one shown (a newly opened
    /// file); an edit keeps the user's view, as in GTK.
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
        // A program that records its stock Z0 sets it; otherwise the user's choice holds.
        self.stock_setup.zero = program
            .as_deref()
            .and_then(stock::recorded_zero)
            .unwrap_or(self.stock_zero_choice);
        self.restart_stock(repaint);
        match program {
            Some(lines) => {
                self.building = true;
                // A new build replaces the failed one, so its notice no longer applies.
                self.build_failed = false;
                self.worker
                    .request(self.generation, lines, self.applied.clone(), repaint);
            }
            None => {
                self.worker.cancel(self.generation);
                self.building = false;
                self.build_failed = false;
                self.clear_toolpath();
            }
        }
    }

    /// Drops the toolpath shown, so nothing of an earlier program stays on screen.
    fn clear_toolpath(&mut self) {
        self.applied = None;
        self.mesh = None;
        self.bounds = None;
        self.cut_bounds = None;
    }

    /// Takes the finished build, if any.
    fn poll_build(&mut self) {
        match self.worker.take() {
            Some(Outcome::Done(built)) if built.generation == self.generation => {
                self.building = false;
                self.build_failed = false;
                self.applied = self.program.clone();
                self.set_mesh(built.mesh, built.replaced);
            }
            Some(Outcome::Failed { generation }) if generation == self.generation => {
                // Already logged by the worker. The machine already holds this program, so
                // the previous program's toolpath must not stay up as if it were this one.
                self.building = false;
                self.build_failed = true;
                self.clear_toolpath();
            }
            Some(Outcome::Done(_) | Outcome::Failed { .. }) | None => {}
        }
    }

    /// Shows `mesh`; for a newly loaded program, fits it into an isometric view, as GTK does.
    fn set_mesh(&mut self, mesh: ToolpathMesh, refit: bool) {
        let ToolpathMesh {
            vertices,
            bounds,
            cut_bounds,
        } = mesh;
        if refit && let Some(bounds) = bounds {
            self.camera.view(StdView::IsoSe);
            self.camera.fit(bounds);
        }
        self.bounds = bounds;
        self.cut_bounds = cut_bounds;
        self.mesh = Some(UploadedMesh {
            generation: self.generation,
            upload: Arc::new(Upload::new(vertices)),
        });
    }

    /// Fits the toolpath into the view, or resets the view when there is none.
    fn fit(&mut self) {
        match self.bounds {
            Some(bounds) => self.camera.fit(bounds),
            None => self.camera.reset(),
        }
    }

    /// Resets the camera, then fits the toolpath (Reset View).
    fn reset_view(&mut self) {
        self.camera.reset();
        self.fit();
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
        self.stock.poll();
        let bounds = self.cut_bounds.or(self.bounds);
        if !self.sidebar_hidden {
            let units = self.units;
            let mut show_stock = self.show_stock;
            let mut setup = self.stock_setup;
            let notice = self.stock.notice().map(|n| n.text(ctx.tr));
            let progress = self.stock.progress();
            let enabled = self.stock_enabled;
            let clicked = egui::Panel::left("gcodekit5-visualizer-sidebar")
                .resizable(true)
                .default_size(sidebar::WIDTH)
                .min_size(sidebar::MIN_WIDTH)
                .show(ui, |ui| {
                    // Fill the panel, so it keeps its width rather than shrinking to fit.
                    egui::ScrollArea::vertical()
                        .id_salt("visualizer-sidebar")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let simulation = enabled.then_some(sidebar::Simulation {
                                show: &mut show_stock,
                                setup: &mut setup,
                                progress,
                                notice,
                            });
                            sidebar::show(ui, bounds.as_ref(), units, simulation, ctx.tr)
                        })
                        .inner
                })
                .inner;
            self.set_stock_setup(setup, ui.ctx());
            self.set_stock_shown(show_stock, ui.ctx());
            match clicked {
                Some(sidebar::SidebarAction::Fit) => self.fit(),
                Some(sidebar::SidebarAction::ResetView) => self.reset_view(),
                Some(sidebar::SidebarAction::Hide) => self.sidebar_hidden = true,
                None => {}
            }
        }
        let rect = ui.available_rect_before_wrap();
        self.view_rect = rect;
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
            view: self.camera.view_matrix(),
            stock: self.stock.mesh().filter(|_| self.show_stock).cloned(),
            rect,
        };
        ui.painter()
            .add(egui_wgpu::Callback::new_paint_callback(rect, callback));

        if let Some(action) = nav_cube::show(ui, rect, ctx.tr) {
            self.apply(action);
        }
        if self.sidebar_hidden && sidebar::show_button(ui, rect, ctx.tr) {
            self.sidebar_hidden = false;
        }
        let loaded = self.program.is_some();
        let too_large = self.mesh.as_ref().is_some_and(|m| m.upload.too_large());
        osd::show(
            ui,
            rect,
            osd::Overlay {
                loaded,
                too_large,
                failed: self.build_failed,
                bounds: bounds.as_ref(),
                tool,
            },
            self.units,
            ctx.tr,
        );
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
                self.reset_view();
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
            | Command::OpenFirmwareSettings
            | Command::OpenDeviceInfo
            | Command::OpenHelp
            | Command::About => return false,
        };
        self.apply(action);
        true
    }

    fn configure(&mut self, config: &Config) {
        self.units = config.ui.measurement_system;
        self.stock_enabled = config.ui.enable_stock_removal_3d;
        if !self.stock_enabled && self.show_stock {
            self.show_stock = false;
            self.stock.stop();
        }
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
                | MachineEvent::StreamHalted { .. }
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
