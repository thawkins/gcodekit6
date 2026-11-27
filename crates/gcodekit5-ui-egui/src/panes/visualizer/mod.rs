//! The Visualizer pane: the program's toolpath in 3D, drawn with wgpu.
//!
//! The toolpath is built from the program the machine streams ([`Machine::program`]) when
//! that program changes (by `Arc` identity), on one background worker that builds only the
//! latest program ([`build`]), and uploaded to the GPU once ([`pipeline`]); a frame only
//! writes the camera and the stream progress. A newly opened program is fitted into an
//! isometric view; an edit of the shown program keeps the user's view. Rapids and
//! moves with the spindle/laser off are grey, cuts are coloured by power, and the lines the
//! machine has acknowledged are drawn in the accent colour. The tool marker follows the work
//! position the machine reports.
//!
//! Mouse: left drag orbits, Shift + left drag pans, the wheel zooms. The 2D
//! view mode ([`VisualizerPane::set_2d`]) locks the camera to the orthographic top view: left
//! drag pans, the nav cube's view grid and zoom row are hidden, and only the Top view is
//! taken from the View commands; a program loaded in 2D is fitted from the top. The nav cube
//! ([`nav_cube`]) and the View commands switch to standard views, zoom and fit; the OSD
//! ([`osd`]) shows the bounds and the tool position. The left sidebar ([`sidebar`]) has
//! a toolbar (Fit to Content, Reset View, Fit to Device Working Area, Hide), its
//! Toolpath and Guides toggles ([`Display`]: rapids, cuts, the tool marker, the grid and its
//! spacing, the machine bounds) and Inspector.
//!
//! Whether the sidebar is hidden is kept between runs in the settings' `panel_visibility`
//! under the `visualizer_sidebar` key ([`Pane::panel_visibility`]).
//!
//! Right-click on the view opens a menu ([`menu`]): the fits, Copy cursor coordinates
//! (the work-plane point under the pointer) and the Toggle switches of the sidebar's toggles.
//!
//! The run preview bar ([`preview`]) at the bottom left plays the toolpath with a preview
//! marker of its own (cyan), at four speeds; File > Run (`Cmd+R`, F5) shows the
//! page and starts it from the beginning. It only animates the view: the pane
//! never asks the machine for anything while it plays. A new program stops it.
//!
//! The machine bounds and Fit to Device Working Area use the active device profile's X, Y
//! and Z limits ([`work_area_of`], handed in through [`Pane::set_device`]); with no profile,
//! Fit to Device does nothing and no bounds are drawn.
//!
//! With the `enable_stock_removal_3d` setting on, the sidebar's Simulation section shows
//! "Show Stock Removal": the stock is cut by the program on a worker ([`stock`]) and
//! drawn lit and depth-tested with the toolpath. The run restarts when the program or the
//! stock changes (field edits after a short pause), and stops when the toggle or the setting
//! is turned off. Each program the Designer generates brings the Designer's stock
//! size and tool diameter with it ([`StockSeed`], through [`PaneContext::stock_seed`]); edits
//! in the sidebar hold until the next one.
//!
//! Not shown: scrollbars and a zoom/centre/cursor status line.
//!
//! [`Machine::program`]: crate::machine::Machine::program
//!
//! Dependencies: `egui`, `egui-wgpu`, `glam`, `gcodekit5-visualizer` (parser, camera),
//! `gcodekit5-app` (machine events, translations), `gcodekit5-core` (units).

pub mod build;
pub mod camera;
pub mod menu;
pub mod nav_cube;
pub mod osd;
pub mod pipeline;
pub mod preview;
pub mod sidebar;
pub mod stock;
pub mod toolpath;

use std::sync::Arc;

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::MachineEvent;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_devicedb::DeviceProfile;
use gcodekit5_settings::config::Config;
use glam::Vec3;

use build::{BuildWorker, Outcome};
pub use camera::{Aabb, Camera, StdView};
use gcodekit5_visualizer::visualizer::StockZeroReference;
pub use nav_cube::NavAction;
pub use pipeline::install;
use pipeline::{Upload, UploadedMesh, VisualizerCallback};
pub use preview::{PreviewSpeed, PreviewState, RunPreview};
pub use sidebar::Display;
pub use stock::{StockSeed, StockSetup, StockSimulation};
pub use toolpath::ToolpathMesh;

use crate::commands::Command;
use crate::workspace::{Pane, PaneContext, PaneId};

/// The settings' `panel_visibility` key of the sidebar, kept between runs.
pub const SIDEBAR_KEY: &str = "visualizer_sidebar";
/// Millimetres the Zoom In / Zoom Out buttons and commands move the eye.
pub const ZOOM_STEP: f32 = 10.0;
/// How long a stock field must stay unchanged before the simulation restarts with it.
pub const STOCK_EDIT_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(250);
/// Millimetres of zoom per point of wheel scroll (5 mm per wheel notch).
const ZOOM_PER_SCROLL_POINT: f32 = 0.1;

/// The work area of `profile`: its X, Y and Z limits in mm (the machine bounds).
///
/// # Arguments
/// * `profile` - The device profile.
pub fn work_area_of(profile: &DeviceProfile) -> Aabb {
    let axis = |a: &gcodekit5_devicedb::AxisLimits| (a.min as f32, a.max as f32);
    let [(x0, x1), (y0, y1), (z0, z1)] = [
        axis(&profile.x_axis),
        axis(&profile.y_axis),
        axis(&profile.z_axis),
    ];
    let a = Vec3::new(x0, y0, z0);
    let b = Vec3::new(x1, y1, z1);
    Aabb::new(a.min(b), a.max(b))
}

/// The msgids the pane shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    nav_cube::msgids()
        .into_iter()
        .chain(menu::msgids())
        .chain(osd::msgids())
        .chain(preview::msgids())
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
    /// Its S-value statistics.
    intensity: Option<toolpath::Intensity>,
    /// A newly loaded toolpath is to be fitted once this frame's view size is known.
    refit_pending: bool,
    worker: BuildWorker,
    /// A build for `generation` was requested and has not arrived.
    building: bool,
    /// The last build failed, so nothing is drawn for the current program.
    build_failed: bool,
    /// Counts builds requested (programs loaded, and rebuilds), to tag each mesh.
    generation: u64,
    /// The Preferences "Arc Segment Length" (mm, clamped) the toolpath is built with.
    arc_segment_length: f64,
    /// The arc segment length changed: the next frame rebuilds the program shown.
    rebuild_pending: bool,
    /// Program lines the machine acknowledged in the current or last stream.
    progress: u32,
    units: MeasurementSystem,
    /// The view's rectangle on the last frame.
    view_rect: egui::Rect,
    /// Whether the user hid the sidebar.
    sidebar_hidden: bool,
    /// The sidebar's Toolpath and Guides toggles.
    display: Display,
    /// The 2D view mode: the camera stays in the top view.
    view_2d: bool,
    /// The active device profile's work area, if a profile is active.
    work_area: Option<Aabb>,
    /// The run preview.
    preview: RunPreview,
    /// File > Run waits for the program's build to start the preview.
    preview_on_build: bool,
    /// File > Run was chosen: the next frame loads the machine's program, then starts.
    run_requested: bool,
    /// When the run preview last moved on, while it plays.
    preview_clock: Option<std::time::Instant>,
    /// The work-plane point under the pointer when the right-click menu opened.
    menu_cursor: Option<glam::Vec2>,
    /// Why File > Run started no preview: the warning title and text.
    preview_notice: Option<(&'static str, &'static str)>,
    /// The `enable_stock_removal_3d` setting.
    stock_enabled: bool,
    /// Whether "Show Stock Removal" is checked.
    show_stock: bool,
    /// The stock and tool simulated.
    stock_setup: StockSetup,
    /// The user's last Stock Z0 choice, used when a program records none.
    stock_zero_choice: StockZeroReference,
    stock: StockSimulation,
    /// When an edited stock restarts the simulation (debounced field edits).
    stock_restart_at: Option<std::time::Instant>,
    /// The serial of the last Designer stock applied.
    stock_seed_serial: Option<u64>,
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
            intensity: None,
            refit_pending: false,
            worker: BuildWorker::new(),
            building: false,
            build_failed: false,
            generation: 0,
            arc_segment_length: toolpath::DEFAULT_ARC_SEGMENT_LENGTH,
            rebuild_pending: false,
            progress: 0,
            units: MeasurementSystem::Metric,
            view_rect: egui::Rect::NOTHING,
            sidebar_hidden: false,
            display: Display::default(),
            view_2d: false,
            work_area: None,
            preview: RunPreview::default(),
            preview_on_build: false,
            run_requested: false,
            preview_clock: None,
            preview_notice: None,
            menu_cursor: None,
            stock_enabled: false,
            show_stock: false,
            stock_setup: StockSetup::default(),
            stock_zero_choice: StockZeroReference::default(),
            stock: StockSimulation::new(),
            stock_restart_at: None,
            stock_seed_serial: None,
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

    /// Changes the stock and tool from a field edit: the simulation restarts once the values
    /// stay unchanged for [`STOCK_EDIT_DEBOUNCE`], so dragging a field runs it once.
    ///
    /// # Arguments
    /// * `setup` - The new stock and tool.
    /// * `repaint` - Context to wake when the pause is over.
    pub fn edit_stock_setup(&mut self, setup: StockSetup, repaint: &egui::Context) {
        if setup == self.stock_setup {
            return;
        }
        if setup.zero != self.stock_setup.zero {
            self.stock_zero_choice = setup.zero;
        }
        self.stock_setup = setup;
        if self.show_stock {
            self.stock_restart_at = Some(std::time::Instant::now() + STOCK_EDIT_DEBOUNCE);
            repaint.request_repaint_after(STOCK_EDIT_DEBOUNCE);
        }
    }

    /// Whether an edited stock waits to restart the simulation.
    pub fn stock_restart_pending(&self) -> bool {
        self.stock_restart_at.is_some()
    }

    /// Restarts the simulation once a field edit's pause is over.
    fn restart_stock_if_due(&mut self, repaint: &egui::Context) {
        match self.stock_restart_at {
            Some(at) if std::time::Instant::now() >= at => self.restart_stock(repaint),
            Some(at) => {
                repaint
                    .request_repaint_after(at.saturating_duration_since(std::time::Instant::now()));
            }
            None => {}
        }
    }

    /// Takes the Designer's stock and tool when it generated a program not applied yet.
    ///
    /// # Returns
    /// Whether the stock changed.
    fn apply_stock_seed(&mut self, seed: Option<StockSeed>) -> bool {
        let Some(seed) = seed.filter(|s| Some(s.serial) != self.stock_seed_serial) else {
            return false;
        };
        self.stock_seed_serial = Some(seed.serial);
        let seeded = seed.apply(self.stock_setup);
        let changed = seeded != self.stock_setup;
        self.stock_setup = seeded;
        changed
    }

    /// Starts the simulation afresh when shown, else
    /// stops it.
    fn restart_stock(&mut self, repaint: &egui::Context) {
        self.stock_restart_at = None;
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
    /// it changes only when a different program is loaded or the program is rebuilt (the
    /// arc segment length changed).
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

    /// The sidebar's Toolpath and Guides toggles.
    pub fn display(&self) -> Display {
        self.display
    }

    /// The run preview.
    pub fn preview(&self) -> &RunPreview {
        &self.preview
    }

    /// The run preview, to drive it.
    pub fn preview_mut(&mut self) -> &mut RunPreview {
        &mut self.preview
    }

    /// Why File > Run started no preview (the warning title and text), until the next
    /// program or Run.
    pub fn preview_notice(&self) -> Option<(&'static str, &'static str)> {
        self.preview_notice
    }

    /// File > Run: starts the run preview from the beginning, once the program's toolpath is
    /// built. With no program or no motion, says why
    /// instead.
    pub fn run_preview(&mut self) {
        self.preview_notice = None;
        if self.program.is_none() {
            self.preview_notice = Some((
                n_!("No G-code to run"),
                n_!("Generate or open G-code before using Run."),
            ));
        } else if self.building {
            self.preview_on_build = true;
        } else if !self.preview.restart() {
            self.preview_notice = Some((
                n_!("No motion commands found"),
                n_!("The loaded G-code has no G0/G1/G2/G3 movement to preview."),
            ));
        }
    }

    /// Whether the 2D view mode is on.
    pub fn is_2d(&self) -> bool {
        self.view_2d
    }

    /// Switches to the 2D view mode (the top view, locked) or back to the 3D view (the
    /// isometric view).
    ///
    /// # Arguments
    /// * `on` - `true` for 2D, `false` for 3D.
    pub fn set_2d(&mut self, on: bool) {
        self.view_2d = on;
        self.camera
            .view(if on { StdView::Top } else { StdView::IsoSe });
    }

    /// The active device profile's work area, if a profile is active.
    pub fn work_area(&self) -> Option<Aabb> {
        self.work_area
    }

    /// Fits the active device's work area into the view (Fit to Device Working Area); with
    /// no active profile it does nothing.
    pub fn fit_device(&mut self) {
        if let Some(area) = self.work_area {
            self.camera.fit(area);
        }
    }

    /// Sets the sidebar's Toolpath and Guides toggles.
    ///
    /// # Arguments
    /// * `display` - What the view draws.
    pub fn set_display(&mut self, display: Display) {
        self.display = display;
    }

    /// Shows `program` (the program as the machine streams it), building its toolpath on the
    /// background worker when it is not the program already shown. `None` clears the view.
    /// The view is refitted only when the program replaces the one shown (a newly opened
    /// file); an edit keeps the user's view.
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
        // A new program ends the preview of the old one.
        self.preview.stop();
        self.preview_on_build = false;
        self.preview_notice = None;
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
        self.intensity = None;
        self.refit_pending = false;
        self.preview.set_points(Vec::new());
    }

    /// Rebuilds the program shown (with the current arc segment length), keeping the view.
    fn rebuild(&mut self, repaint: &egui::Context) {
        let Some(lines) = self.program.clone() else {
            return;
        };
        self.generation = self.generation.wrapping_add(1);
        self.building = true;
        self.build_failed = false;
        self.worker
            .request(self.generation, lines, self.applied.clone(), repaint);
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

    /// Shows `mesh`; for a newly loaded program, fits it into an isometric view.
    fn set_mesh(&mut self, mesh: ToolpathMesh, refit: bool) {
        let ToolpathMesh {
            vertices,
            bounds,
            cut_bounds,
            preview,
            intensity,
        } = mesh;
        self.intensity = intensity;
        self.preview.set_points(preview);
        if std::mem::take(&mut self.preview_on_build) {
            self.run_preview();
        }
        // The fit depends on the view's aspect, which this frame sets after the build is
        // polled, so a build that lands on the first frame is fitted in `ui`, not here.
        self.refit_pending = refit && bounds.is_some();
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

    /// Resets the camera (to the top view in 2D), then fits the toolpath (Reset View).
    fn reset_view(&mut self) {
        self.camera.reset();
        if self.view_2d {
            self.camera.view(StdView::Top);
        }
        self.fit();
    }

    /// Runs a nav cube action.
    ///
    /// # Arguments
    /// * `action` - What the user clicked.
    pub fn apply(&mut self, action: NavAction) {
        match action {
            // 2D stays in the top view.
            NavAction::View(view) if self.view_2d && view != StdView::Top => {}
            NavAction::View(view) => self.camera.view(view),
            NavAction::ZoomIn => self.camera.zoom(ZOOM_STEP),
            NavAction::ZoomOut => self.camera.zoom(-ZOOM_STEP),
            NavAction::Fit => self.fit(),
            NavAction::FitDevice => self.fit_device(),
        }
    }

    /// Moves the run preview on by the time since the last frame while it plays, and wakes
    /// the next frame in time for its next step.
    fn play_preview(&mut self, repaint: &egui::Context) {
        if self.preview.state() != PreviewState::Playing {
            self.preview_clock = None;
            return;
        }
        let now = std::time::Instant::now();
        if let Some(last) = self.preview_clock {
            self.preview.advance(now.saturating_duration_since(last));
        }
        self.preview_clock = Some(now);
        repaint.request_repaint_after(preview::TICK);
    }

    /// The right-click menu over the view `response` ([`menu`]).
    fn context_menu(&mut self, response: &egui::Response, rect: egui::Rect, tr: &Translator) {
        if response.secondary_clicked() {
            self.menu_cursor = response
                .interact_pointer_pos()
                .and_then(|pos| menu::cursor_on_plane(&self.camera, rect, pos));
        }
        let (display, cursor, units) = (&mut self.display, self.menu_cursor, self.units);
        let mut chosen = None;
        response.context_menu(|ui| chosen = menu::show(ui, display, cursor, units, tr));
        match chosen {
            Some(menu::MenuAction::Fit) => self.fit(),
            Some(menu::MenuAction::ResetView) => self.reset_view(),
            Some(menu::MenuAction::FitDevice) => self.fit_device(),
            None => {}
        }
    }

    /// Orbits, pans and zooms with the mouse over `response`.
    fn handle_input(&mut self, ui: &egui::Ui, response: &egui::Response) {
        if response.dragged_by(egui::PointerButton::Primary) {
            let delta = response.drag_delta();
            if self.view_2d || ui.input(|i| i.modifiers.shift) {
                self.camera.pan(delta.x, delta.y);
            } else {
                self.camera.orbit(delta.x, delta.y);
            }
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                // Zoom in as the wheel turns toward the user.
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
        let seeded = self.apply_stock_seed(ctx.stock_seed());
        let shown_program = self.program.clone();
        if let Some(machine) = ctx.machine() {
            self.load_program(machine.program().cloned(), ui.ctx());
            let status = machine.status();
            tool = status.work_position().filter(|_| status.connected);
        }
        let reloaded = match (&shown_program, &self.program) {
            (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
            (None, None) => false,
            (Some(_), None) | (None, Some(_)) => true,
        };
        if seeded && !reloaded {
            self.restart_stock(ui.ctx());
        }
        self.restart_stock_if_due(ui.ctx());
        if std::mem::take(&mut self.rebuild_pending) {
            self.rebuild(ui.ctx());
        }
        self.poll_build();
        if std::mem::take(&mut self.run_requested) {
            self.run_preview();
        }
        self.stock.poll();
        let bounds = self.cut_bounds.or(self.bounds);
        if !self.sidebar_hidden {
            let units = self.units;
            let mut show_stock = self.show_stock;
            let mut setup = self.stock_setup;
            let notice = self.stock.notice().map(|n| n.text(ctx.tr));
            let progress = self.stock.progress();
            let enabled = self.stock_enabled;
            let display = &mut self.display;
            let view_2d = self.view_2d;
            let intensity = self.intensity;
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
                            let inspector = sidebar::Inspector {
                                bounds: bounds.as_ref(),
                                intensity,
                            };
                            sidebar::show(
                                ui, inspector, view_2d, display, units, simulation, ctx.tr,
                            )
                        })
                        .inner
                })
                .inner;
            self.edit_stock_setup(setup, ui.ctx());
            self.set_stock_shown(show_stock, ui.ctx());
            match clicked {
                Some(sidebar::SidebarAction::Fit) => self.fit(),
                Some(sidebar::SidebarAction::ResetView) => self.reset_view(),
                Some(sidebar::SidebarAction::FitDevice) => self.fit_device(),
                Some(sidebar::SidebarAction::ViewMode(on)) => self.set_2d(on),
                Some(sidebar::SidebarAction::Hide) => self.sidebar_hidden = true,
                None => {}
            }
        }
        let rect = ui.available_rect_before_wrap();
        self.view_rect = rect;
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        self.camera
            .set_aspect(rect.width() / rect.height().max(1.0));
        if std::mem::take(&mut self.refit_pending)
            && let Some(bounds) = self.bounds
        {
            self.camera.view(if self.view_2d {
                StdView::Top
            } else {
                StdView::IsoSe
            });
            self.camera.fit(bounds);
        }
        self.handle_input(ui, &response);
        self.context_menu(&response, rect, ctx.tr);

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
        self.play_preview(ui.ctx());
        let marker = tool.filter(|_| self.display.marker);
        let callback = VisualizerCallback {
            view_proj: self.camera.view_proj(rect.width() / rect.height().max(1.0)),
            progress: self.progress,
            done_color: srgb_floats(crate::theme::palette(mode).accent),
            mesh: self.mesh.clone(),
            tool: marker.map(Vec3::from_array),
            preview: self.preview.position().filter(|_| self.display.marker),
            view: self.camera.view_matrix(),
            stock: self.stock.mesh().filter(|_| self.show_stock).cloned(),
            rect,
            show_travel: self.display.rapids,
            show_cuts: self.display.cuts,
            grid: self.display.grid.then_some(self.display.grid_spacing),
            machine_bounds: self.work_area.filter(|_| self.display.bounds),
        };
        ui.painter()
            .add(egui_wgpu::Callback::new_paint_callback(rect, callback));

        if let Some(action) = nav_cube::show(ui, rect, !self.view_2d, ctx.tr) {
            self.apply(action);
        }
        if self.sidebar_hidden && sidebar::show_button(ui, rect, ctx.tr) {
            self.sidebar_hidden = false;
        }
        let bar = preview::show_bar(ui, rect, &mut self.preview, ctx.tr);
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
                bottom: bar.top(),
                notice: self.preview_notice,
            },
            self.units,
            ctx.tr,
        );
    }

    fn command(&mut self, command: Command, ctx: &mut PaneContext) -> bool {
        let action = match command {
            Command::FileRun => {
                // With the app's machine, the program to preview is the one the machine
                // holds now (the app syncs the editor first); the next frame loads it before
                // starting, so a Run right after an edit previews the edited program.
                if ctx.machine().is_some() {
                    self.run_requested = true;
                } else {
                    self.run_preview();
                }
                return true;
            }
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
            | Command::About
            | Command::AlignLeft
            | Command::AlignRight
            | Command::AlignTop
            | Command::AlignBottom
            | Command::AlignCenterX
            | Command::AlignCenterY
            | Command::Group
            | Command::Ungroup
            | Command::Duplicate
            | Command::FitDevice => return false,
        };
        self.apply(action);
        true
    }

    fn set_device(&mut self, profile: Option<&DeviceProfile>) {
        self.work_area = profile.map(work_area_of);
    }

    fn panel_visibility(&self) -> Option<(&'static str, bool)> {
        Some((SIDEBAR_KEY, !self.sidebar_hidden))
    }

    fn configure(&mut self, config: &Config) {
        self.sidebar_hidden = !config
            .ui
            .panel_visibility
            .get(SIDEBAR_KEY)
            .copied()
            .unwrap_or(true);
        self.units = config.ui.measurement_system;
        self.stock_enabled = config.ui.enable_stock_removal_3d;
        let arc_segment_length =
            toolpath::clamp_arc_segment_length(config.file_processing.arc_segment_length);
        if arc_segment_length != self.arc_segment_length {
            self.arc_segment_length = arc_segment_length;
            self.worker.set_arc_segment_length(arc_segment_length);
            self.rebuild_pending = self.program.is_some();
        }
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
                | MachineEvent::ProtocolMismatch { .. }
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
