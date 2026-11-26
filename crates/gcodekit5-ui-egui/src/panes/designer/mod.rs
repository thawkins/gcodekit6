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
//! [`shape_gallery`]; the shape it arms is placed by the next canvas click. New, Open, Save
//! and Import are [`file_ops`]; the shapes a CAM page generates are added by
//! [`DesignerPane::add_generated_paths`]; G-code and SVG output (Export, the toolbox's
//! Generate G-Code and Frame) are [`export`], the outcome shown as the pane's notice. The
//! canvas outlines the active device profile's work area ([`DesignerPane::work_area`]). Not
//! ported here: scrollbars.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model), `gcodekit5-app` (translations),
//! `gcodekit5-settings` (grid line widths, display units), `gcodekit5-core` (units).

pub mod canvas;
pub mod export;
pub mod file_ops;
mod generated;
pub mod layers;
pub mod properties;
pub mod shape_gallery;
mod shared;
pub mod svg_export;
pub mod toolbox;

pub use export::ToEditor;
pub use shared::SharedDesigner;

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_designer::DesignerState;
use gcodekit5_designer::gcode_gen::MachineLimits;
use gcodekit5_designer::model::{Point, Shape};
use gcodekit5_devicedb::DeviceProfile;
use gcodekit5_settings::config::Config;

use canvas::array_dialog::{self, ArrayDialog};
use canvas::overlay::{self, ViewAction};
use canvas::rendering::{self, LineWidths};
use canvas::text_tool::{self, TextTool};
use canvas::toolpath_preview::ToolpathPreview;
use canvas::{CanvasInput, CanvasView, DesignerTool, ShapeFlags, editing};
use shape_gallery::ShapeGallery;

use crate::commands::Command;
use crate::workspace::{Pane, PaneContext, PaneId};

/// How far Duplicate moves the copies right and up, in mm (GTK's Ctrl+D offset).
pub const DUPLICATE_OFFSET_MM: f64 = 10.0;

/// The msgids the pane shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = toolbox::msgids();
    ids.extend(canvas::input::msgids());
    ids.extend(canvas::overlay::msgids());
    ids.extend(layers::msgids());
    ids.extend(properties::msgids());
    ids.extend(text_tool::msgids());
    ids.extend(shape_gallery::msgids());
    ids.extend(array_dialog::msgids());
    ids.extend(file_ops::msgids());
    ids.extend(export::msgids());
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
    /// The context menu's Array dialog.
    array: ArrayDialog,
    /// The toolpath preview ("Show Toolpaths").
    toolpaths: ToolpathPreview,
    /// The outcome of the last file operation, shown above the canvas.
    notice: Option<String>,
    /// G-code for the editor (Generate G-Code, Frame), until the app takes it.
    editor_gcode: Option<String>,
    /// The machine's working area for the out-of-limits check: the device profile's, else
    /// the Preferences limits ([`DesignerPane::machine_limits`]).
    limits: Option<MachineLimits>,
    /// The Preferences limits, used while no device profile is active.
    config_limits: Option<MachineLimits>,
    /// The active device profile's X/Y area (mm), if any.
    device: Option<editing::Bounds>,
    /// Whether Import reads STL files (Settings' "STL File Import").
    stl_import: bool,
    /// The toolbox asked for G-code for the editor; the app runs it.
    editor_request: Option<ToEditor>,
    /// Counts changes to the design ([`DesignerPane::revision`]).
    revision: u64,
    /// The last world point under the pointer, for the status line.
    cursor: (f64, f64),
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
            array: ArrayDialog::default(),
            toolpaths: ToolpathPreview::default(),
            notice: None,
            editor_gcode: None,
            limits: None,
            config_limits: None,
            device: None,
            stl_import: false,
            editor_request: None,
            revision: 0,
            cursor: (0.0, 0.0),
        }
    }

    /// The toolpaths the preview shows (the last finished build).
    pub fn preview_toolpaths(&self) -> &[gcodekit5_designer::Toolpath] {
        self.toolpaths.toolpaths()
    }

    /// Whether the preview matches the design as it is now (true while it is off).
    pub fn toolpath_preview_ready(&self) -> bool {
        self.toolpaths.is_current(&self.state, self.revision)
    }

    /// Whether the Array dialog is open.
    pub fn array_dialog_open(&self) -> bool {
        self.array.is_open()
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

    /// The design, for edits made outside the canvas; counts as a change to the design
    /// ([`DesignerPane::revision`]).
    pub fn state_mut(&mut self) -> &mut DesignerState {
        self.touch();
        &mut self.state
    }

    /// The design's revision: a counter that grows with every change that can reach the
    /// toolpaths (an edit through [`DesignerPane::state_mut`], undo and redo, hiding, a file
    /// operation, and each frame in which the user finished an action in the pane: a button
    /// release or a key). Spurious increments only cost a preview rebuild.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Records a change to the design.
    pub(crate) fn touch(&mut self) {
        self.revision += 1;
    }

    /// Toolpath preview builds running: 0 or 1.
    #[doc(hidden)]
    pub fn toolpath_builds_running(&self) -> usize {
        self.toolpaths.running()
    }

    /// Test hook: while `on`, every toolpath preview build panics on its worker thread.
    ///
    /// # Arguments
    /// * `on` - Whether builds panic.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_toolpath_build_panics(&mut self, on: bool) {
        self.toolpaths.set_build_panics(on);
    }

    /// Test hook: each toolpath preview build waits `delay` before its first shape.
    ///
    /// # Arguments
    /// * `delay` - The wait, or `None` for none.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_toolpath_build_delay(&mut self, delay: Option<std::time::Duration>) {
        self.toolpaths.set_build_delay(delay);
    }

    /// Toolpath preview builds started so far.
    #[doc(hidden)]
    pub fn toolpath_builds_started(&self) -> u64 {
        self.toolpaths.started()
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
        self.touch();
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

    /// The work area outlined on the canvas and fitted by Fit to Device: the active device
    /// profile's X/Y limits (GTK's `compute_device_bbox`), else GTK's 250 x 250 mm.
    pub fn work_area(&self) -> editing::Bounds {
        self.device.unwrap_or_else(editing::default_work_area)
    }

    /// The XY limits exported G-code is checked against: the active device profile's, else
    /// Preferences > Machine's X/Y limits from 0 (GTK's `designer_machine_limits`), if set.
    pub fn machine_limits(&self) -> Option<&MachineLimits> {
        self.limits.as_ref()
    }

    /// Recomputes [`DesignerPane::machine_limits`] from the device and the settings.
    fn update_limits(&mut self) {
        self.limits = match self.device {
            Some((x_min, y_min, x_max, y_max)) => Some(MachineLimits {
                x_min,
                x_max,
                y_min,
                y_max,
            }),
            None => self.config_limits.clone(),
        };
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
            self.touch();
            self.deselect_unpickable();
        }
    }

    /// Redoes the last undone edit; ignored during a canvas drag ([`DesignerPane::dragging`]).
    pub fn redo(&mut self) {
        if self.dragging() {
            tracing::debug!("redo ignored during a canvas drag");
        } else {
            self.state.redo();
            self.touch();
            self.deselect_unpickable();
        }
    }

    /// Whether a canvas drag is in progress. A move or resize records its own undo step when
    /// it ends, from the shapes it started with, so undo, redo and delete wait for it.
    pub fn dragging(&self) -> bool {
        self.input.dragging()
    }

    /// Rotates the selection by `degrees` (counter-clockwise positive), as one undo step.
    ///
    /// # Arguments
    /// * `degrees` - The angle to add.
    pub fn rotate_selected_by(&mut self, degrees: f64) {
        editing::rotate_selected_by(&mut self.state, degrees);
    }

    /// Runs a shape operation from a key (Align, Group, Ungroup) and records the change, so
    /// the toolpath preview follows it ([`DesignerPane::revision`]). Ignored during a canvas
    /// drag, which still holds the shapes it started from.
    ///
    /// # Arguments
    /// * `op` - The model's operation on the selection.
    fn edit(&mut self, op: fn(&mut DesignerState)) {
        if self.dragging() {
            tracing::debug!("shape edit ignored during a canvas drag");
            return;
        }
        op(&mut self.state);
        self.touch();
    }

    /// Cut, Copy or Paste of shapes through the design's own clipboard, as GTK's Edit
    /// actions on the Designer page: Paste centres the copies on the pointer's last point
    /// on the canvas, with new ids, as one undo step. Ignored during a canvas drag.
    ///
    /// # Arguments
    /// * `command` - `Cut`, `Copy` or `Paste`; any other command does nothing.
    fn clipboard(&mut self, command: Command) {
        if self.dragging() {
            tracing::debug!(?command, "clipboard ignored during a canvas drag");
            return;
        }
        match command {
            Command::Cut => editing::cut_selected(&mut self.state),
            Command::Copy => self.state.copy_selected(),
            Command::Paste => {
                let (x, y) = self.input.hover.unwrap_or(self.cursor);
                self.state.paste_at_location(x, y);
            }
            _ => return,
        }
        self.touch();
    }

    /// The canvas: input, then painting with the view the input left.
    ///
    /// # Returns
    /// The command of the empty state's button clicked this frame, if any.
    fn show_canvas(&mut self, ui: &mut egui::Ui, tr: &Translator) -> Option<Command> {
        let rect = ui.available_rect_before_wrap();
        ui.advance_cursor_after_rect(rect);
        let response = ui.interact(rect, canvas::canvas_id(), egui::Sense::click_and_drag());
        self.state
            .canvas
            .set_canvas_size(f64::from(rect.width()), f64::from(rect.height()));
        if !self.fitted && rect.width() > 0.0 && rect.height() > 0.0 {
            let area = self.work_area();
            editing::fit_work_area(&mut self.state, area);
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
        if self.input.take_array_click() {
            self.array.open();
        }
        self.array.show(ui.ctx(), &mut self.state, self.units, tr);
        self.view = CanvasView::new(rect, self.state.canvas.viewport());
        let dragging = self.dragging();
        self.toolpaths
            .update(ui.ctx(), &self.state, &self.flags, self.revision, dragging);
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
            self.work_area(),
        );
        self.toolpaths
            .paint(&painter, &self.view, &self.state, palette);
        if self.gallery.pending().is_some() {
            let ghost = self.input.hover.and_then(|at| self.gallery.ghost(at));
            rendering::paint_ghost(&painter, &self.view, ghost.as_ref(), palette.accent);
            hint(ui, rect, &tr.tr("Click to position shape"));
        }
        self.overlays(ui, rect, tr);
        let empty = if self.state.canvas.shape_store.is_empty() && self.gallery.pending().is_none()
        {
            let laser =
                self.state.machine_mode() != gcodekit5_designer::designer_state::MachineMode::Cnc3D;
            overlay::empty_state(ui, rect, tr, self.stl_import, laser)
        } else {
            None
        };
        self.gallery.show(ui.ctx(), tr);
        empty
    }

    /// The floating toolbar (applied at once) and the status line over the canvas `rect`.
    fn overlays(&mut self, ui: &mut egui::Ui, rect: egui::Rect, tr: &Translator) {
        if let Some(action) = overlay::toolbar(ui, rect, tr) {
            self.apply_view(action);
        }
        if let Some(at) = self.input.hover {
            self.cursor = at;
        }
        let center = self.view.screen_to_world(rect.center());
        let status = overlay::Status {
            zoom: self.state.canvas.zoom(),
            center: (f64::from(center.x), f64::from(center.y)),
            cursor: self.cursor,
            snap: self.state.snap_enabled,
            constraint: ui.input(|i| i.modifiers.shift),
            units: self.units,
        };
        overlay::status_line(ui, rect, &status, tr);
    }

    /// Applies a view change from the canvas toolbar or the View commands.
    ///
    /// # Arguments
    /// * `action` - The change.
    pub fn apply_view(&mut self, action: ViewAction) {
        let area = self.work_area();
        match action {
            ViewAction::ZoomIn => self.state.zoom_in(),
            ViewAction::ZoomOut => self.state.zoom_out(),
            ViewAction::FitContent => editing::zoom_fit(&mut self.state, area),
            ViewAction::FitWorkArea => editing::fit_work_area(&mut self.state, area),
            // GTK's reset: 100% with the origin at the canvas's bottom-left corner.
            ViewAction::FitViewport => {
                self.state.canvas.set_zoom(1.0);
                self.state.canvas.set_pan(0.0, 0.0);
            }
        }
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
        rect.min + egui::vec2(0.0, overlay::TOOLBAR_HEIGHT + 4.0),
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
            .default_size(250.0)
            .show(ui, |ui| {
                let preview = self.toolpaths.progress();
                toolbox::show(ui, &mut self.state, self.tool, self.units, preview, tr)
            })
            .inner;
        if let Some(tool) = out.tool {
            self.set_tool(tool);
        }
        if out.gallery {
            self.gallery.open();
        }
        if out.generate {
            self.request_to_editor(ToEditor::Generate);
        }
        if out.frame {
            self.request_to_editor(ToEditor::Frame);
        }
        // A finished user action (a release, a key) may have changed the design.
        let acted = ui.input(|i| {
            i.pointer.any_released()
                || i.events.iter().any(|e| {
                    matches!(
                        e,
                        egui::Event::Key { pressed: true, .. }
                            | egui::Event::Text(_)
                            | egui::Event::Paste(_)
                    )
                })
        });
        if acted {
            self.touch();
        }
        // After the click's touch, so the design as it is now counts as previewed.
        if out.cancel_preview {
            self.toolpaths.cancel(self.revision);
        }
        self.show_notice(ui, tr);
        if let Some(command) = self.show_canvas(ui, tr) {
            ctx.request(command);
        }
    }

    fn command(&mut self, command: Command, _ctx: &mut PaneContext) -> bool {
        match command {
            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::ZoomIn => self.apply_view(ViewAction::ZoomIn),
            Command::ZoomOut => self.apply_view(ViewAction::ZoomOut),
            Command::ZoomFit => self.apply_view(ViewAction::FitContent),
            Command::ResetView => self.apply_view(ViewAction::FitViewport),
            Command::Cut | Command::Copy | Command::Paste => self.clipboard(command),
            Command::AlignLeft => self.edit(DesignerState::align_selected_horizontal_left),
            Command::AlignRight => self.edit(DesignerState::align_selected_horizontal_right),
            Command::AlignTop => self.edit(DesignerState::align_selected_vertical_top),
            Command::AlignBottom => self.edit(DesignerState::align_selected_vertical_bottom),
            Command::AlignCenterX => self.edit(DesignerState::align_selected_horizontal_center),
            Command::AlignCenterY => self.edit(DesignerState::align_selected_vertical_center),
            Command::Group => self.edit(DesignerState::group_selected),
            Command::Ungroup => self.edit(DesignerState::ungroup_selected),
            Command::Duplicate => self.edit(|state| state.duplicate_selected(DUPLICATE_OFFSET_MM)),
            Command::FitDevice => self.apply_view(ViewAction::FitWorkArea),
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
            | Command::OpenFirmwareSettings
            | Command::OpenDeviceInfo
            | Command::OpenHelp
            | Command::About => return false,
        }
        true
    }

    fn set_device(&mut self, profile: Option<&DeviceProfile>) {
        self.device = profile.map(|p| {
            let (x, y) = (&p.x_axis, &p.y_axis);
            (
                x.min.min(x.max),
                y.min.min(y.max),
                x.min.max(x.max),
                y.min.max(y.max),
            )
        });
        self.update_limits();
    }

    fn configure(&mut self, config: &Config) {
        self.units = config.ui.measurement_system;
        self.stl_import = config.ui.enable_stl_import;
        // GTK's fallback when no device profile is active: the Preferences limits.
        let machine = &config.machine;
        self.config_limits =
            (machine.x_limit > 0.0 && machine.y_limit > 0.0).then_some(MachineLimits {
                x_min: 0.0,
                x_max: machine.x_limit,
                y_min: 0.0,
                y_max: machine.y_limit,
            });
        self.update_limits();
        self.widths = LineWidths {
            grid_major: config.ui.grid_major_line_width as f32,
            grid_minor: config.ui.grid_minor_line_width as f32,
        };
    }
}
