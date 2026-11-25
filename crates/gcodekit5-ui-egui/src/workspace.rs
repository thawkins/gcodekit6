//! Docking workspace: the panes of the main window and their arrangement.
//!
//! The arrangement is a [`craft_ui::docking::Workspace`] keyed by [`PaneId`]; it serializes
//! with serde so it can be saved and restored. Each pane implements [`Pane`]. Until the real
//! panes land (milestones M2 to M5) every pane is a placeholder showing its title.
//!
//! Panes ask for commands through [`PaneContext::request`]; the app runs them after drawing.
//! A command that belongs to a pane (the machine commands belong to Machine Control) is handed
//! to it through [`Workspace::route`].
//!
//! The workspace also tracks which pane the user last pressed the pointer in
//! ([`Workspace::focused_pane`]), so the app can tell the keymap which pane has the keyboard.
//!
//! Dependencies: `egui`, `craft-ui` (docking, with the `serde` feature), `gcodekit5-app`
//! (translations).

use craft_ui::docking::{self, DockArea, DockContent, DockStyle, Layout, Node, Permissions};
use craft_ui::layout::{SplitAxis, SplitSize};
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::MachineEvent;
use gcodekit5_settings::config::Config;
use serde::{Deserialize, Serialize};

use crate::commands::{Command, MACHINE_CONTROL_SCOPE, VISUALIZER_SCOPE};
use crate::machine::Machine;

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

    /// The keymap scope of the bindings that work while this pane has the keyboard, if it has
    /// one.
    pub fn keymap_scope(self) -> Option<&'static str> {
        match self {
            PaneId::MachineControl => Some(MACHINE_CONTROL_SCOPE),
            PaneId::Visualizer => Some(VISUALIZER_SCOPE),
            PaneId::Console
            | PaneId::Editor
            | PaneId::Designer
            | PaneId::DesignerLayers
            | PaneId::DesignerProperties
            | PaneId::CamTools => None,
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
    /// The machine connection, with this frame's events and the derived status.
    machine: Option<&'a Machine>,
}

impl<'a> PaneContext<'a> {
    /// Creates a context for drawing panes.
    ///
    /// # Arguments
    /// * `tr` - Translator for the current UI language.
    /// * `commands` - The queue that [`PaneContext::request`] adds to.
    pub fn new(tr: &'a Translator, commands: &'a mut Vec<Command>) -> Self {
        Self {
            tr,
            commands,
            machine: None,
        }
    }

    /// The same context with the machine connection's frame events and status.
    ///
    /// # Arguments
    /// * `machine` - The app's machine connection.
    pub fn with_machine(mut self, machine: &'a Machine) -> Self {
        self.machine = Some(machine);
        self
    }

    /// The machine connection, when the app provided it.
    pub fn machine(&self) -> Option<&'a Machine> {
        self.machine
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

    /// Runs `command` if it belongs to this pane.
    ///
    /// # Arguments
    /// * `command` - A command from a key, a menu or a pane.
    /// * `ctx` - Shared state the pane may use.
    ///
    /// # Returns
    /// Whether the pane ran it.
    fn command(&mut self, command: Command, ctx: &mut PaneContext) -> bool {
        let _ = (command, ctx);
        false
    }

    /// Ends any press-and-hold input in progress (a hold-to-jog): the window lost focus, the
    /// pane was not drawn this frame, or the app is exiting.
    fn cancel_hold(&mut self) {}

    /// Takes the current settings (at start and after Preferences are applied).
    ///
    /// # Arguments
    /// * `config` - The current settings.
    fn configure(&mut self, config: &Config) {
        let _ = config;
    }

    /// Takes machine events (see [`Machine::take_pending`]), whether or not the pane is
    /// drawn, for a pane that keeps a log.
    ///
    /// # Arguments
    /// * `events` - Events since the last call, oldest first.
    /// * `tr` - Translator for the current UI language.
    fn machine_events(&mut self, events: &[MachineEvent], tr: &Translator) {
        let _ = (events, tr);
    }
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
    /// The pane the user last pressed the pointer in; `None` after a press outside every pane.
    #[serde(skip)]
    focused: Option<PaneId>,
}

/// The panes whose bodies a frame drew; the others have their holds cancelled
/// ([`Pane::cancel_hold`]).
type Drawn = Vec<PaneId>;

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
            focused: None,
        }
    }

    /// Puts `pane` in place of the pane with the same id.
    ///
    /// # Arguments
    /// * `pane` - The pane to show.
    pub fn set_pane(&mut self, pane: Box<dyn Pane>) {
        let id = pane.id();
        match self.panes.iter_mut().find(|p| p.id() == id) {
            Some(slot) => *slot = pane,
            None => self.panes.push(pane),
        }
    }

    /// Offers `command` to each pane until one runs it.
    ///
    /// # Arguments
    /// * `command` - The command to run.
    /// * `ctx` - Shared state passed to the panes.
    ///
    /// # Returns
    /// Whether a pane ran it.
    pub fn route(&mut self, command: Command, ctx: &mut PaneContext) -> bool {
        self.panes.iter_mut().any(|pane| pane.command(command, ctx))
    }

    /// Ends every pane's press-and-hold input ([`Pane::cancel_hold`]).
    pub fn cancel_holds(&mut self) {
        for pane in &mut self.panes {
            pane.cancel_hold();
        }
    }

    /// Hands the current settings to every pane ([`Pane::configure`]).
    ///
    /// # Arguments
    /// * `config` - The current settings.
    pub fn configure(&mut self, config: &Config) {
        for pane in &mut self.panes {
            pane.configure(config);
        }
    }

    /// Hands machine `events` to every pane ([`Pane::machine_events`]).
    ///
    /// # Arguments
    /// * `events` - Events since the last call, oldest first.
    /// * `tr` - Translator for the current UI language.
    pub fn machine_events(&mut self, events: &[MachineEvent], tr: &Translator) {
        for pane in &mut self.panes {
            pane.machine_events(events, tr);
        }
    }

    /// Applies a layout change (move, activate, float...) with the workspace's permissions.
    ///
    /// # Arguments
    /// * `action` - The change.
    ///
    /// # Errors
    /// The docking model's refusal.
    pub fn apply(&mut self, action: docking::Action<PaneId>) -> Result<(), docking::DockError> {
        self.dock.apply(action, permissions).map(drop)
    }

    /// The pane the user last pressed the pointer in, or `None` when the last press was
    /// outside every pane.
    pub fn focused_pane(&self) -> Option<PaneId> {
        self.focused
    }

    /// Draws the workspace into the rest of `ui` and applies the user's layout changes.
    ///
    /// # Arguments
    /// * `ui` - The `Ui` whose remaining room the workspace fills.
    /// * `ctx` - Shared state passed to every pane.
    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext) {
        let style = DockStyle::from_ui(ui);
        let press = ui.input(|i| {
            i.pointer
                .any_pressed()
                .then(|| i.pointer.press_origin())
                .flatten()
        });
        let mut content = Content {
            panes: &mut self.panes,
            ctx,
            press,
            pressed_in: None,
            drawn: Drawn::new(),
        };
        let output = DockArea::new(egui::Id::unique("gcodekit5-workspace"))
            .locked(self.dock.locked)
            .show_content(ui, &self.dock.layout, &style, &mut content);
        if press.is_some() {
            self.focused = content.pressed_in;
        }
        let drawn = content.drawn;
        for pane in self.panes.iter_mut().filter(|p| !drawn.contains(&p.id())) {
            pane.cancel_hold();
        }
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
    /// Where the pointer was pressed this frame, if it was.
    press: Option<egui::Pos2>,
    /// The pane whose body holds `press`.
    pressed_in: Option<PaneId>,
    /// The panes whose bodies were drawn this frame.
    drawn: Drawn,
}

impl DockContent<PaneId> for Content<'_, '_, '_> {
    fn body(&mut self, ui: &mut egui::Ui, panel: &PaneId) {
        self.drawn.push(*panel);
        if self.press.is_some_and(|pos| ui.max_rect().contains(pos)) {
            self.pressed_in = Some(*panel);
        }
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
