//! The Designer pane: the design canvas, with a tool strip (until the toolbox lands) and
//! grid and snap switches.
//!
//! The design is a `gcodekit5_designer::DesignerState`: shapes, selection, the undo/redo
//! stack, the viewport, and the grid and snap settings. The pane owns it; the app shares the
//! pane through [`SharedDesigner`], so the layers, properties and file commands can reach the
//! same design. Undo, Redo and the View zoom commands reach the pane through
//! [`Pane::command`] when the Designer is the pane the user works in. The app copies
//! [`DesignerPane::is_modified`] into `AppState::dirty.designer` for the quit prompt.
//!
//! Ported from the GTK `ui/gtk/designer_canvas/*`. Not ported here: the toolbox, layers and
//! properties panels, the Text tool (it needs the text dialog), shape gallery placement,
//! the array dialog, the toolpath preview, file operations, scrollbars, the device's work
//! area (the default 250 x 250 mm is shown), and the context menu's other entries.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model), `gcodekit5-app` (translations),
//! `gcodekit5-settings` (grid line widths).

pub mod canvas;

use std::cell::RefCell;
use std::rc::Rc;

use gcodekit5_app::i18n::Translator;
use gcodekit5_designer::DesignerState;
use gcodekit5_designer::model::Point;
use gcodekit5_settings::config::Config;

use canvas::rendering::{self, LineWidths};
use canvas::{CanvasInput, CanvasView, DesignerTool, editing};

use crate::commands::Command;
use crate::workspace::{Pane, PaneContext, PaneId};

/// The msgids the pane shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = DesignerTool::ALL
        .into_iter()
        .map(DesignerTool::label_msgid)
        .collect();
    ids.extend([n_!("Show Grid"), n_!("Snap to grid")]);
    ids.extend(canvas::input::msgids());
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
        }
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
        }
    }

    /// Redoes the last undone edit; ignored during a canvas drag ([`DesignerPane::dragging`]).
    pub fn redo(&mut self) {
        if self.dragging() {
            tracing::debug!("redo ignored during a canvas drag");
        } else {
            self.state.redo();
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

    /// The tool strip and the grid and snap switches.
    fn show_tools(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        ui.horizontal_wrapped(|ui| {
            for tool in DesignerTool::ALL {
                if ui
                    .selectable_label(self.tool == tool, tr.tr(tool.label_msgid()))
                    .clicked()
                {
                    self.set_tool(tool);
                }
            }
            ui.separator();
            ui.checkbox(&mut self.state.show_grid, tr.tr("Show Grid"));
            ui.checkbox(&mut self.state.snap_enabled, tr.tr("Snap to grid"));
        });
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
        self.input
            .handle(ui, &response, &view, &mut self.state, self.tool, tr);
        self.view = CanvasView::new(rect, self.state.canvas.viewport());
        let palette = crate::theme::palette(ui.ctx().theme());
        rendering::paint(
            &ui.painter_at(rect),
            &self.view,
            &self.state,
            &self.input,
            palette,
            self.widths,
        );
    }
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
        egui::Panel::top("gcodekit5-designer-tools")
            .show_separator_line(false)
            .show(ui, |ui| self.show_tools(ui, tr));
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
