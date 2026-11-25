//! The Designer pane: the toolbox (a side panel) and the design canvas.
//!
//! The design is a `gcodekit5_designer::DesignerState`: shapes, selection, the undo/redo
//! stack, the viewport, and the grid and snap settings. The pane owns it; the app shares the
//! pane through [`SharedDesigner`], so the layers, properties and file commands can reach the
//! same design. Undo, Redo and the View zoom commands reach the pane through
//! [`Pane::command`] when the Designer is the pane the user works in. The app copies
//! [`DesignerPane::is_modified`] into `AppState::dirty.designer` for the quit prompt.
//!
//! Ported from the GTK `ui/gtk/designer_canvas/*` and `designer_toolbox.rs` ([`toolbox`]);
//! [`layers`] is the Layers pane and [`properties`] the Properties pane. The Text tool opens
//! the Add Text dialog ([`canvas::text_tool`]). The toolbox's "Shape Gallery…" opens
//! [`shape_gallery`]; the shape it arms is placed by the next canvas click. Not ported here:
//! scrollbars, the device's work area (the default 250 x 250 mm is shown), and the context
//! menu's other entries.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model), `gcodekit5-app` (translations),
//! `gcodekit5-settings` (grid line widths, display units), `gcodekit5-core` (units).

pub mod canvas;
pub mod layers;
pub mod properties;
pub mod shape_gallery;
pub mod toolbox;

use std::cell::RefCell;
use std::rc::Rc;

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_designer::DesignerState;
use gcodekit5_designer::gcode_gen::MachineLimits;
use gcodekit5_designer::model::{Point, Shape};
use gcodekit5_settings::config::Config;

use canvas::rendering::{self, LineWidths};
use canvas::text_tool::{self, TextTool};
use canvas::{CanvasInput, CanvasView, DesignerTool, ShapeFlags, editing};
use shape_gallery::ShapeGallery;

use crate::commands::Command;
use crate::workspace::{Pane, PaneContext, PaneId};

/// The msgids the pane shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = toolbox::msgids();
    ids.extend(canvas::input::msgids());
    ids.extend(layers::msgids());
    ids.extend(properties::msgids());
    ids.extend(text_tool::msgids());
    ids.extend(shape_gallery::msgids());
    ids
}

/// The Designer pane.
pub struct DesignerPane {
    state: DesignerState,
    tool: DesignerTool,
    input: CanvasInput,
    /// The view of the last frame.
    view: CanvasView,
    /// Whether the view was fitted to the work area (on the first frame with a size).
    fitted: bool,
    widths: LineWidths,
    /// The shapes the layers panel has hidden or locked.
    flags: ShapeFlags,
    /// The display units of the toolbox's settings and the properties.
    units: MeasurementSystem,
    /// The Text tool's Add Text dialog.
    text_tool: TextTool,
    /// The properties panel's live offset or chamfer preview.
    preview: Vec<Shape>,
    /// The Fast Shape Gallery and the shape it armed.
    gallery: ShapeGallery,
}

impl std::fmt::Debug for DesignerPane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesignerPane")
            .field("shapes", &self.state.canvas.shape_count())
            .field("tool", &self.tool)
            .field("view", &self.view)
            .finish_non_exhaustive()
    }
}

impl Default for DesignerPane {
    fn default() -> Self {
        Self::new()
    }
}

impl DesignerPane {
    /// An empty design with the Select tool.
    pub fn new() -> Self {
        Self {
            state: DesignerState::new(),
            tool: DesignerTool::Select,
            input: CanvasInput::default(),
            view: CanvasView::default(),
            fitted: false,
            widths: LineWidths::default(),
            flags: ShapeFlags::default(),
            units: MeasurementSystem::default(),
            text_tool: TextTool::default(),
            preview: Vec::new(),
            gallery: ShapeGallery::default(),
        }
    }

    /// Whether the Fast Shape Gallery is open.
    pub fn gallery_open(&self) -> bool {
        self.gallery.is_open()
    }

    /// The gallery shape waiting for a canvas click, if any.
    pub fn pending_shape(&self) -> Option<&Shape> {
        self.gallery.pending()
    }

    /// The display units (the Settings window's measurement system).
    pub fn units(&self) -> MeasurementSystem {
        self.units
    }

    /// Sets the display units (normally from the settings, through `configure`).
    ///
    /// # Arguments
    /// * `units` - Metric or imperial.
    pub fn set_units(&mut self, units: MeasurementSystem) {
        self.units = units;
    }

    /// Whether the Text tool's Add Text dialog is open.
    pub fn text_dialog_open(&self) -> bool {
        self.text_tool.is_open()
    }

    /// The live effect preview the canvas paints (the properties panel's offset or chamfer
    /// being typed).
    pub fn effect_preview(&self) -> &[Shape] {
        &self.preview
    }

    /// Replaces the live effect preview; an empty list clears it.
    ///
    /// # Arguments
    /// * `shapes` - The previewed shapes.
    pub fn set_effect_preview(&mut self, shapes: Vec<Shape>) {
        self.preview = shapes;
    }

    /// The design.
    pub fn state(&self) -> &DesignerState {
        &self.state
    }

    /// The design, for edits made outside the canvas.
    pub fn state_mut(&mut self) -> &mut DesignerState {
        &mut self.state
    }

    /// The current tool.
    pub fn tool(&self) -> DesignerTool {
        self.tool
    }

    /// Switches to `tool`, dropping a polyline or drag in progress.
    ///
    /// # Arguments
    /// * `tool` - The tool the canvas's primary button uses from now on.
    pub fn set_tool(&mut self, tool: DesignerTool) {
        if tool != self.tool {
            self.input.reset();
            self.tool = tool;
        }
    }

    /// The hidden and locked shapes.
    pub fn flags(&self) -> &ShapeFlags {
        &self.flags
    }

    /// Hides or shows shape `id`. A hidden shape is not painted or picked, and is deselected.
    ///
    /// # Arguments
    /// * `id` - The shape.
    /// * `hidden` - Whether it is hidden from now on.
    pub fn set_hidden(&mut self, id: u64, hidden: bool) {
        self.flags.set_hidden(id, hidden);
        if hidden {
            self.deselect(id);
        }
    }

    /// Locks or unlocks shape `id`. A locked shape cannot be picked, and is deselected.
    ///
    /// # Arguments
    /// * `id` - The shape.
    /// * `locked` - Whether it is locked from now on.
    pub fn set_locked(&mut self, id: u64, locked: bool) {
        self.flags.set_locked(id, locked);
        if locked {
            self.deselect(id);
        }
    }

    /// Drops every hidden or locked shape from the selection. Undo and redo restore a shape's
    /// whole state, `selected` included, so this keeps the selection free of them.
    fn deselect_unpickable(&mut self) {
        let ids: Vec<u64> = self
            .state
            .canvas
            .shapes()
            .filter(|o| o.selected && !self.flags.is_pickable(o.id))
            .map(|o| o.id)
            .collect();
        for id in ids {
            self.deselect(id);
        }
    }

    /// Drops shape `id` from the selection.
    fn deselect(&mut self, id: u64) {
        let canvas = &mut self.state.canvas;
        if let Some(obj) = canvas.get_shape_mut(id) {
            obj.selected = false;
        }
        if canvas.selection_manager.selected_id() == Some(id) {
            let next = canvas.shapes().find(|o| o.selected).map(|o| o.id);
            canvas.selection_manager.set_selected_id(next);
        }
    }

    /// The canvas's view on the last frame: maps screen points to world millimetres.
    pub fn view(&self) -> CanvasView {
        self.view
    }

    /// The points of the polyline being drawn.
    pub fn polyline_points(&self) -> &[Point] {
        self.input.polyline()
    }

    /// Whether the design changed since it was created (or last saved).
    pub fn is_modified(&self) -> bool {
        self.state.is_modified
    }

    /// Undoes the last edit; ignored during a canvas drag ([`DesignerPane::dragging`]).
    pub fn undo(&mut self) {
        if self.dragging() {
            tracing::debug!("undo ignored during a canvas drag");
        } else {
            self.state.undo();
            self.deselect_unpickable();
        }
    }

    /// Redoes the last undone edit; ignored during a canvas drag ([`DesignerPane::dragging`]).
    pub fn redo(&mut self) {
        if self.dragging() {
            tracing::debug!("redo ignored during a canvas drag");
        } else {
            self.state.redo();
            self.deselect_unpickable();
        }
    }

    /// Whether a canvas drag is in progress. A move or resize records its own undo step when
    /// it ends, from the shapes it started with, so undo, redo and delete wait for it.
    pub fn dragging(&self) -> bool {
        self.input.dragging()
    }

    /// Generates G-code for the design, leaving out hidden shapes.
    ///
    /// Hidden shapes ([`ShapeFlags`]) are view state the model does not know, so this builds
    /// the code from a copy of the design without them (the design is untouched). Every
    /// export must go through here, so a hidden shape is never cut. Locked shapes are cut.
    ///
    /// # Arguments
    /// * `machine_limits` - The machine's limits, if known (as the model's `generate_gcode`).
    pub fn generate_gcode(&self, machine_limits: Option<MachineLimits>) -> String {
        let mut copy = self.state.clone();
        for id in self.state.canvas.shape_store.draw_order_iter() {
            if self.flags.is_hidden(id) {
                copy.canvas.remove_shape(id);
            }
        }
        copy.generate_gcode(machine_limits)
    }

    /// Rotates the selection by `degrees` (counter-clockwise positive), as one undo step.
    ///
    /// # Arguments
    /// * `degrees` - The angle to add.
    pub fn rotate_selected_by(&mut self, degrees: f64) {
        editing::rotate_selected_by(&mut self.state, degrees);
    }

    /// The canvas: input, then painting with the view the input left.
    fn show_canvas(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        self.state
            .canvas
            .set_canvas_size(f64::from(rect.width()), f64::from(rect.height()));
        if !self.fitted && rect.width() > 0.0 && rect.height() > 0.0 {
            editing::fit_work_area(&mut self.state);
            self.fitted = true;
        }
        let view = CanvasView::new(rect, self.state.canvas.viewport());
        if self.gallery.pending().is_some() {
            self.place_gallery_shape(ui, &response, &view);
        } else {
            self.input.handle(
                ui,
                &response,
                &view,
                &mut self.state,
                &self.flags,
                self.tool,
                tr,
            );
        }
        if let Some(at) = self.input.take_text_click() {
            self.text_tool.open(at);
        }
        self.text_tool.show(ui.ctx(), &mut self.state, tr);
        self.view = CanvasView::new(rect, self.state.canvas.viewport());
        let palette = crate::theme::palette(ui.ctx().theme());
        let painter = ui.painter_at(rect);
        rendering::paint(
            &painter,
            &self.view,
            &self.state,
            &self.flags,
            &self.input,
            palette,
            self.widths,
            &self.preview,
        );
        if self.gallery.pending().is_some() {
            let ghost = self.input.hover.and_then(|at| self.gallery.ghost(at));
            rendering::paint_ghost(&painter, &self.view, ghost.as_ref(), palette.accent);
            hint(ui, rect, &tr.tr("Click to position shape"));
        }
        self.gallery.show(ui.ctx(), tr);
    }

    /// The canvas while a gallery shape is armed: the pointer moves its ghost, a primary
    /// click places it (snapped unless the Select tool is active, as GTK), Escape drops it.
    /// Other input waits, so the click selects nothing.
    fn place_gallery_shape(&mut self, ui: &egui::Ui, response: &egui::Response, view: &CanvasView) {
        let world = |p: egui::Pos2| {
            let w = view.screen_to_world(p);
            (f64::from(w.x), f64::from(w.y))
        };
        self.input.hover = response.hover_pos().map(world);
        if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            let (x, y) = world(p);
            let at = if self.tool == DesignerTool::Select {
                (x, y)
            } else {
                editing::snap(&self.state, x, y)
            };
            self.gallery.place(&mut self.state, at);
            response.request_focus();
        }
        let escape =
            !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::Escape));
        if escape {
            self.gallery.cancel();
        }
    }
}

/// A hint at the top centre of the canvas `rect` (GTK's "Click to position shape"); it does
/// not take the pointer.
fn hint(ui: &mut egui::Ui, rect: egui::Rect, text: &str) {
    let strip = egui::Rect::from_min_size(
        rect.min + egui::vec2(0.0, 12.0),
        egui::vec2(rect.width(), 40.0),
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(strip)
            .layout(egui::Layout::top_down(egui::Align::Center)),
        |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.add(egui::Label::new(text).selectable(false));
            });
        },
    );
}

impl Pane for DesignerPane {
    fn id(&self) -> PaneId {
        PaneId::Designer
    }

    fn title(&self, tr: &Translator) -> String {
        tr.tr(PaneId::Designer.title_msgid()).into_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext) {
        let tr = ctx.tr;
        let out = egui::Panel::left("gcodekit5-designer-toolbox")
            .default_size(190.0)
            .show(ui, |ui| {
                toolbox::show(ui, &mut self.state, self.tool, self.units, tr)
            })
            .inner;
        if let Some(tool) = out.tool {
            self.set_tool(tool);
        }
        if out.gallery {
            self.gallery.open();
        }
        self.show_canvas(ui, tr);
    }

    fn command(&mut self, command: Command, _ctx: &mut PaneContext) -> bool {
        match command {
            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::ZoomIn => self.state.zoom_in(),
            Command::ZoomOut => self.state.zoom_out(),
            Command::ZoomFit => editing::zoom_fit(&mut self.state),
            // GTK's reset: 100% with the origin at the canvas's bottom-left corner.
            Command::ResetView => {
                self.state.canvas.set_zoom(1.0);
                self.state.canvas.set_pan(0.0, 0.0);
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
            | Command::ViewTop
            | Command::ViewFront
            | Command::ViewIso
            | Command::OpenSettings
            | Command::OpenDeviceManager
            | Command::OpenToolsManager
            | Command::OpenMaterialsManager
            | Command::OpenHelp
            | Command::About => return false,
        }
        true
    }

    fn configure(&mut self, config: &Config) {
        self.units = config.ui.measurement_system;
        self.widths = LineWidths {
            grid_major: config.ui.grid_major_line_width as f32,
            grid_minor: config.ui.grid_minor_line_width as f32,
        };
    }
}

/// The designer pane shared between the workspace, which draws it, and the app, which routes
/// commands to it and reads its modified flag (and, later, the layers, properties and file
/// commands).
#[derive(Debug, Clone)]
pub struct SharedDesigner(pub Rc<RefCell<DesignerPane>>);

impl Pane for SharedDesigner {
    fn id(&self) -> PaneId {
        PaneId::Designer
    }

    fn title(&self, tr: &Translator) -> String {
        tr.tr(PaneId::Designer.title_msgid()).into_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext) {
        match self.0.try_borrow_mut() {
            Ok(mut pane) => pane.ui(ui, ctx),
            Err(error) => tracing::warn!(%error, "designer busy; not drawn this frame"),
        }
    }

    fn command(&mut self, command: Command, ctx: &mut PaneContext) -> bool {
        match self.0.try_borrow_mut() {
            Ok(mut pane) => pane.command(command, ctx),
            Err(error) => {
                tracing::warn!(%error, ?command, "designer busy; command dropped");
                false
            }
        }
    }

    fn configure(&mut self, config: &Config) {
        match self.0.try_borrow_mut() {
            Ok(mut pane) => pane.configure(config),
            Err(error) => tracing::warn!(%error, "designer busy; settings not applied"),
        }
    }
}
