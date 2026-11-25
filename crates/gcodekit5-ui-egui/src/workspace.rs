//! Docking workspace: the panes of the main window and their arrangement.
//!
//! The arrangement is a [`craft_ui::docking::Workspace`] keyed by [`PaneId`]; it serializes
//! with serde so it can be saved and restored. Each pane implements [`Pane`]. Until the real
//! panes land (milestones M2 to M5) every pane is a placeholder showing its title.
//!
//! Panes ask for commands through [`PaneContext::request`]; the app runs them after drawing.
//!
//! Dependencies: `egui`, `craft-ui` (docking, with the `serde` feature), `gcodekit5-app`
//! (translations).

use craft_ui::docking::{self, DockArea, DockContent, DockStyle, Layout, Node, Permissions};
use craft_ui::layout::{SplitAxis, SplitSize};
use gcodekit5_app::i18n::Translator;
use serde::{Deserialize, Serialize};

use crate::commands::Command;

/// Identifies one pane of the main window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PaneId {
    /// Jogging, homing and machine state.
    MachineControl,
    /// The device console.
    Console,
    /// The G-code editor.
    Editor,
    /// The toolpath visualizer.
    Visualizer,
    /// The design canvas.
    Designer,
    /// The designer's layer list.
    DesignerLayers,
    /// The designer's property inspector.
    DesignerProperties,
    /// The CAM tool generators.
    CamTools,
}

impl PaneId {
    /// Every pane, in menu order.
    pub const ALL: [PaneId; 8] = [
        PaneId::MachineControl,
        PaneId::Console,
        PaneId::Editor,
        PaneId::Visualizer,
        PaneId::Designer,
        PaneId::DesignerLayers,
        PaneId::DesignerProperties,
        PaneId::CamTools,
    ];

    /// The English msgid of the pane's title, as listed in `po/gcodekit5.pot`.
    pub fn title_msgid(self) -> &'static str {
        match self {
            PaneId::MachineControl => n_!("Machine Control"),
            PaneId::Console => n_!("Device Console"),
            PaneId::Editor => n_!("G-Code Editor"),
            PaneId::Visualizer => n_!("Visualizer"),
            PaneId::Designer => n_!("Designer"),
            PaneId::DesignerLayers => n_!("Layers"),
            PaneId::DesignerProperties => n_!("Properties"),
            PaneId::CamTools => n_!("CAM Tools"),
        }
    }
}

/// What a pane may use while it draws.
#[derive(Debug)]
pub struct PaneContext<'a> {
    /// Translator for the current UI language.
    pub tr: &'a Translator,
    /// Commands the panes asked for this frame; the app runs them after the panes are drawn.
    commands: &'a mut Vec<Command>,
}

impl<'a> PaneContext<'a> {
    /// Creates a context for drawing panes.
    ///
    /// # Arguments
    /// * `tr` - Translator for the current UI language.
    /// * `commands` - The queue that [`PaneContext::request`] adds to.
    pub fn new(tr: &'a Translator, commands: &'a mut Vec<Command>) -> Self {
        Self { tr, commands }
    }

    /// Asks the app to run `command` once this frame's panes are drawn.
    ///
    /// # Arguments
    /// * `command` - The command to run.
    pub fn request(&mut self, command: Command) {
        self.commands.push(command);
    }
}

/// One pane of the docking workspace.
pub trait Pane {
    /// The pane's identity in the layout.
    fn id(&self) -> PaneId;

    /// The pane's tab title, translated.
    ///
    /// # Arguments
    /// * `tr` - Translator for the current UI language.
    fn title(&self, tr: &Translator) -> String;

    /// Draws the pane's body.
    ///
    /// # Arguments
    /// * `ui` - The pane body's `Ui`.
    /// * `ctx` - Shared state the pane may use.
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext);
}

/// A pane that shows only its title, until the real pane replaces it.
#[derive(Debug, Clone, Copy)]
struct PlaceholderPane {
    id: PaneId,
}

impl Pane for PlaceholderPane {
    fn id(&self) -> PaneId {
        self.id
    }

    fn title(&self, tr: &Translator) -> String {
        tr.tr(self.id.title_msgid()).into_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext) {
        ui.centered_and_justified(|ui| {
            // Not selectable: a selectable label fills the body and takes every drag in it.
            ui.add(
                egui::Label::new(egui::RichText::new(self.title(ctx.tr)).weak()).selectable(false),
            );
        });
    }
}

/// One placeholder pane per [`PaneId`].
fn placeholder_panes() -> Vec<Box<dyn Pane>> {
    PaneId::ALL
        .into_iter()
        .map(|id| Box::new(PlaceholderPane { id }) as Box<dyn Pane>)
        .collect()
}

/// The main window's panes and their docking arrangement.
///
/// Only the arrangement is serialized and compared; the panes are rebuilt on load.
#[derive(Serialize, Deserialize)]
pub struct Workspace {
    dock: docking::Workspace<PaneId>,
    #[serde(skip, default = "placeholder_panes")]
    panes: Vec<Box<dyn Pane>>,
}

impl std::fmt::Debug for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workspace")
            .field("dock", &self.dock)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Workspace {
    fn eq(&self, other: &Self) -> bool {
        self.dock == other.dock
    }
}

/// A tab group holding `panels`, the first one shown.
fn tabs(panels: Vec<PaneId>) -> Node<PaneId> {
    Node::Tabs { panels, active: 0 }
}

/// A split of `first` and `second` along `axis`, `first` taking `ratio` of the room.
fn split(axis: SplitAxis, ratio: f32, first: Node<PaneId>, second: Node<PaneId>) -> Node<PaneId> {
    Node::Split {
        axis,
        size: SplitSize::Ratio(ratio),
        first: Box::new(first),
        second: Box::new(second),
    }
}

/// Every pane may move and float but not close: there is no menu to reopen one yet.
fn permissions(_pane: &PaneId) -> Permissions {
    Permissions {
        closable: false,
        ..Permissions::default()
    }
}

impl Workspace {
    /// The arrangement a first run starts with: machine control on the left, the visualizer,
    /// editor, designer and CAM tools in the middle above the console, and the designer's
    /// layers and properties on the right.
    pub fn default_layout() -> Workspace {
        let centre = split(
            SplitAxis::Vertical,
            0.75,
            tabs(vec![
                PaneId::Visualizer,
                PaneId::Editor,
                PaneId::Designer,
                PaneId::CamTools,
            ]),
            tabs(vec![PaneId::Console]),
        );
        let inspector = split(
            SplitAxis::Vertical,
            0.5,
            tabs(vec![PaneId::DesignerLayers]),
            tabs(vec![PaneId::DesignerProperties]),
        );
        let root = split(
            SplitAxis::Horizontal,
            0.22,
            tabs(vec![PaneId::MachineControl]),
            split(SplitAxis::Horizontal, 0.75, centre, inspector),
        );
        Workspace {
            dock: docking::Workspace::new(Layout {
                root: Some(root),
                floating: Vec::new(),
            }),
            panes: placeholder_panes(),
        }
    }

    /// Draws the workspace into the rest of `ui` and applies the user's layout changes.
    ///
    /// # Arguments
    /// * `ui` - The `Ui` whose remaining room the workspace fills.
    /// * `ctx` - Shared state passed to every pane.
    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext) {
        let style = DockStyle::from_ui(ui);
        let mut content = Content {
            panes: &mut self.panes,
            ctx,
        };
        let output = DockArea::new(egui::Id::unique("gcodekit5-workspace"))
            .locked(self.dock.locked)
            .show_content(ui, &self.dock.layout, &style, &mut content);
        if let Some(error) = output.error {
            tracing::warn!(?error, "workspace layout not drawn");
        }
        if !output.actions.is_empty()
            && let Err(error) = self.dock.apply_all(output.actions, permissions)
        {
            tracing::warn!(?error, "workspace layout change rejected");
        }
    }
}

/// Adapts the panes to the docking renderer for one frame.
struct Content<'p, 'c, 't> {
    panes: &'p mut [Box<dyn Pane>],
    ctx: &'c mut PaneContext<'t>,
}

impl DockContent<PaneId> for Content<'_, '_, '_> {
    fn body(&mut self, ui: &mut egui::Ui, panel: &PaneId) {
        if let Some(pane) = self.panes.iter_mut().find(|p| p.id() == *panel) {
            pane.ui(ui, self.ctx);
        }
    }

    fn logical_label(&self, panel: &PaneId) -> String {
        self.panes.iter().find(|p| p.id() == *panel).map_or_else(
            || self.ctx.tr.tr(panel.title_msgid()).into_owned(),
            |p| p.title(self.ctx.tr),
        )
    }

    fn permissions(&self, panel: &PaneId) -> Permissions {
        permissions(panel)
    }
}
