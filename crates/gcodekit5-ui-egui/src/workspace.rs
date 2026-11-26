//! The main window's pages and the panes on them, arranged as the GTK front end does: a row
//! of eight page tabs ([`Page`], in GTK's order) above one page at a time, each page a fixed
//! layout of panes. The four manager pages (Device Manager, Device Config, CNC Tools,
//! Materials) hold no pane: the app draws them ([`Workspace::show`] leaves them to it).
//! Switching pages keeps every pane's state; nothing about the arrangement is saved, so a
//! docking layout an earlier build may have kept is never read.
//!
//! Each pane implements [`Pane`]; one that is not set yet is a placeholder showing its title.
//!
//! Panes ask for commands through [`PaneContext::request`]; the app runs them after drawing.
//! A command that belongs to a pane (the machine commands belong to Machine Control) is handed
//! to it through [`Workspace::route`]. A pane that needs a confirmation queues it
//! ([`Pane::take_confirm`]); the app asks it and hands the answer back
//! ([`Workspace::confirmed`]).
//!
//! The workspace also tracks which pane the user last pressed the pointer in
//! ([`Workspace::focused_pane`]), so the app can tell the keymap which pane has the keyboard,
//! and the pane the user works in ([`Workspace::active_pane`]), which a press on a menu does
//! not change, so the app can hand Undo or the zoom commands to the designer.
//!
//! Dependencies: `egui`, `craft-ui` (accessible tabs), `gcodekit5-app` (translations).

use craft_ui::tabs::Tab;
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::MachineEvent;
use gcodekit5_settings::config::Config;

use crate::commands::{Command, MACHINE_CONTROL_SCOPE, VISUALIZER_SCOPE};
use crate::dialogs::{ConfirmChoice, ConfirmRequest};
use crate::machine::Machine;
use gcodekit5_devicedb::ControllerType;

/// Identifies one pane of the main window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
    /// Every pane.
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

/// One page of the main window, as GTK's top row of tabs names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    /// Toolbox, canvas, and the inspector over the objects list.
    Designer,
    /// The toolpath visualizer.
    Visualizer,
    /// Machine controls, with the G-code editor above the console beside them.
    MachineControl,
    /// The CAM tool generators.
    CamTools,
    /// The device profiles.
    DeviceManager,
    /// The controller's firmware settings.
    DeviceConfig,
    /// The CNC tool library.
    CncTools,
    /// The material library.
    Materials,
}

impl Page {
    /// Every page, in GTK's tab order.
    pub const ALL: [Page; 8] = [
        Page::Designer,
        Page::Visualizer,
        Page::MachineControl,
        Page::CamTools,
        Page::DeviceManager,
        Page::DeviceConfig,
        Page::CncTools,
        Page::Materials,
    ];

    /// The English msgid of the page's tab, as listed in `po/gcodekit5.pot`.
    pub fn title_msgid(self) -> &'static str {
        match self {
            Page::Designer => n_!("Designer"),
            Page::Visualizer => n_!("Visualizer"),
            Page::MachineControl => n_!("Machine Control"),
            Page::CamTools => n_!("CAM Tools"),
            Page::DeviceManager => n_!("Device Manager"),
            Page::DeviceConfig => n_!("Device Config"),
            Page::CncTools => n_!("CNC Tools"),
            Page::Materials => n_!("Materials"),
        }
    }

    /// The panes the page shows; none for a manager page, which the app draws.
    pub fn panes(self) -> &'static [PaneId] {
        match self {
            Page::Designer => &[
                PaneId::Designer,
                PaneId::DesignerProperties,
                PaneId::DesignerLayers,
            ],
            Page::Visualizer => &[PaneId::Visualizer],
            Page::MachineControl => &[PaneId::MachineControl, PaneId::Editor, PaneId::Console],
            Page::CamTools => &[PaneId::CamTools],
            Page::DeviceManager | Page::DeviceConfig | Page::CncTools | Page::Materials => &[],
        }
    }

    /// The page that shows `pane`.
    ///
    /// # Arguments
    /// * `pane` - The pane.
    pub fn of_pane(pane: PaneId) -> Page {
        match pane {
            PaneId::MachineControl | PaneId::Editor | PaneId::Console => Page::MachineControl,
            PaneId::Visualizer => Page::Visualizer,
            PaneId::Designer | PaneId::DesignerLayers | PaneId::DesignerProperties => {
                Page::Designer
            }
            PaneId::CamTools => Page::CamTools,
        }
    }

    /// The page a command shows, for the commands that opened a manager window before the
    /// managers became pages.
    ///
    /// # Arguments
    /// * `command` - A command.
    pub fn for_command(command: Command) -> Option<Page> {
        match command {
            Command::OpenDeviceManager => Some(Page::DeviceManager),
            Command::OpenFirmwareSettings => Some(Page::DeviceConfig),
            Command::OpenToolsManager => Some(Page::CncTools),
            Command::OpenMaterialsManager => Some(Page::Materials),
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
            | Command::ZoomFit
            | Command::ZoomIn
            | Command::ZoomOut
            | Command::ResetView
            | Command::ViewTop
            | Command::ViewFront
            | Command::ViewIso
            | Command::OpenSettings
            | Command::OpenDeviceInfo
            | Command::OpenHelp
            | Command::About => None,
        }
    }
}

/// Every msgid the page tabs can show, for the translation guard test.
pub fn msgids() -> Vec<&'static str> {
    Page::ALL.into_iter().map(Page::title_msgid).collect()
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

/// One pane of a page.
pub trait Pane {
    /// The pane's identity in the layout.
    fn id(&self) -> PaneId;

    /// The pane's title, translated.
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

    /// Runs once every frame, whether or not the pane is drawn, for work that must follow
    /// the machine while the pane's page is hidden (a query that keeps its state current).
    ///
    /// # Arguments
    /// * `machine` - The app's machine connection.
    fn frame(&mut self, machine: &Machine) {
        let _ = machine;
    }

    /// Takes the active device profile's controller type (at start and when it changes).
    ///
    /// # Arguments
    /// * `controller` - The controller type, if a profile is active.
    fn set_controller(&mut self, controller: Option<&ControllerType>) {
        let _ = controller;
    }

    /// Takes the confirmation the pane wants the app to ask for, if any.
    fn take_confirm(&mut self) -> Option<ConfirmRequest> {
        None
    }

    /// Acts on the answer to a confirmation the pane asked for.
    ///
    /// # Arguments
    /// * `request` - The question.
    /// * `choice` - The answer.
    /// * `ctx` - Shared state the pane may use.
    ///
    /// # Returns
    /// Whether the question was the pane's.
    fn confirmed(
        &mut self,
        request: ConfirmRequest,
        choice: ConfirmChoice,
        ctx: &mut PaneContext,
    ) -> bool {
        let _ = (request, choice, ctx);
        false
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

/// The main window's pages and their panes.
pub struct Workspace {
    panes: Vec<Box<dyn Pane>>,
    /// The page shown.
    page: Page,
    /// The pane the user last pressed the pointer in; `None` after a press outside every pane.
    focused: Option<PaneId>,
    /// The pane the user last pressed the pointer in, kept through presses outside every
    /// pane (a menu, a tab).
    active: Option<PaneId>,
    /// The panes whose bodies the last frame drew.
    drawn: Drawn,
}

/// The panes whose bodies a frame drew; the others have their holds cancelled
/// ([`Pane::cancel_hold`]).
type Drawn = Vec<PaneId>;

impl std::fmt::Debug for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workspace")
            .field("page", &self.page)
            .finish_non_exhaustive()
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

/// Height of the page tab row in points.
const TAB_HEIGHT: f32 = 26.0;

/// Share of the page's width the Machine Control page's editor and console column takes,
/// and the Designer page's inspector column.
const SIDE_COLUMN: f32 = 0.3;

impl Workspace {
    /// The pages with placeholder panes, showing Machine Control (GTK's default start page).
    pub fn new() -> Workspace {
        Workspace {
            panes: placeholder_panes(),
            page: Page::MachineControl,
            focused: None,
            active: None,
            drawn: Drawn::new(),
        }
    }

    /// The page shown.
    pub fn page(&self) -> Page {
        self.page
    }

    /// Shows `page` from the next frame on.
    ///
    /// # Arguments
    /// * `page` - The page to show.
    pub fn set_page(&mut self, page: Page) {
        self.page = page;
    }

    /// Shows the page that holds `pane` from the next frame on.
    ///
    /// # Arguments
    /// * `pane` - The pane to bring to the front.
    pub fn show_pane(&mut self, pane: PaneId) {
        self.page = Page::of_pane(pane);
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

    /// Offers `command` to the pane `id` only.
    ///
    /// # Arguments
    /// * `id` - The pane to offer it to.
    /// * `command` - The command to run.
    /// * `ctx` - Shared state passed to the pane.
    ///
    /// # Returns
    /// Whether the pane ran it.
    pub fn route_to(&mut self, id: PaneId, command: Command, ctx: &mut PaneContext) -> bool {
        self.panes
            .iter_mut()
            .find(|pane| pane.id() == id)
            .is_some_and(|pane| pane.command(command, ctx))
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

    /// Runs every pane's per-frame work ([`Pane::frame`]), drawn or not.
    ///
    /// # Arguments
    /// * `machine` - The app's machine connection.
    pub fn frame(&mut self, machine: &Machine) {
        for pane in &mut self.panes {
            pane.frame(machine);
        }
    }

    /// Hands the active device profile's controller type to every pane
    /// ([`Pane::set_controller`]).
    ///
    /// # Arguments
    /// * `controller` - The controller type, if a profile is active.
    pub fn set_controller(&mut self, controller: Option<&ControllerType>) {
        for pane in &mut self.panes {
            pane.set_controller(controller);
        }
    }

    /// Takes the confirmations the panes want asked ([`Pane::take_confirm`]).
    pub fn take_confirms(&mut self) -> Vec<ConfirmRequest> {
        self.panes
            .iter_mut()
            .filter_map(|pane| pane.take_confirm())
            .collect()
    }

    /// Hands the answer to a pane's confirmation to the pane that asked
    /// ([`Pane::confirmed`]).
    ///
    /// # Arguments
    /// * `request` - The question.
    /// * `choice` - The answer.
    /// * `ctx` - Shared state passed to the panes.
    ///
    /// # Returns
    /// Whether a pane took it.
    pub fn confirmed(
        &mut self,
        request: ConfirmRequest,
        choice: ConfirmChoice,
        ctx: &mut PaneContext,
    ) -> bool {
        self.panes
            .iter_mut()
            .any(|pane| pane.confirmed(request, choice, ctx))
    }

    /// The pane the user last pressed the pointer in, or `None` when the last press was
    /// outside every pane.
    pub fn focused_pane(&self) -> Option<PaneId> {
        self.focused
    }

    /// The pane the user works in: the one they last pressed the pointer in (a press outside
    /// every pane, such as on a menu, keeps it), while the last frame drew it.
    pub fn active_pane(&self) -> Option<PaneId> {
        self.active.filter(|id| self.drawn.contains(id))
    }

    /// Whether the last frame drew the pane `id`'s body (its page is the one shown).
    ///
    /// # Arguments
    /// * `id` - The pane.
    pub fn is_drawn(&self, id: PaneId) -> bool {
        self.drawn.contains(&id)
    }

    /// Draws the row of page tabs across `ui`, each the same width, the shown page's filled.
    ///
    /// # Arguments
    /// * `ui` - A top panel's `Ui`.
    /// * `tr` - Translator for the tab labels.
    ///
    /// # Returns
    /// The page whose tab was clicked this frame, if any; the caller shows it.
    pub fn show_tabs(&self, ui: &mut egui::Ui, tr: &Translator) -> Option<Page> {
        let row = ui.available_rect_before_wrap();
        let width = row.width() / Page::ALL.len() as f32;
        let mut clicked = None;
        for (i, page) in Page::ALL.into_iter().enumerate() {
            let rect = egui::Rect::from_min_size(
                egui::pos2(row.left() + width * i as f32, row.top()),
                egui::vec2(width, TAB_HEIGHT),
            );
            let label = tr.tr(page.title_msgid());
            let selected = page == self.page;
            let id = egui::Id::unique(("gcodekit5-page-tab", i));
            let response = Tab::new(id, &label, selected).show_at(ui, rect, |ui, response| {
                paint_tab(ui, response, rect, &label, selected);
            });
            if response.clicked() {
                clicked = Some(page);
            }
        }
        ui.allocate_rect(
            egui::Rect::from_min_size(row.min, egui::vec2(row.width(), TAB_HEIGHT)),
            egui::Sense::hover(),
        );
        clicked
    }

    /// Draws the shown page's panes into the rest of `ui`; a manager page draws nothing
    /// here (the app draws it).
    ///
    /// # Arguments
    /// * `ui` - The `Ui` whose remaining room the page fills.
    /// * `ctx` - Shared state passed to every pane.
    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext) {
        let press = ui.input(|i| {
            i.pointer
                .any_pressed()
                .then(|| i.pointer.press_origin())
                .flatten()
        });
        let mut frame = Content {
            panes: &mut self.panes,
            ctx,
            press,
            pressed_in: None,
            drawn: Drawn::new(),
        };
        let side = ui.available_width() * SIDE_COLUMN;
        match self.page {
            Page::Designer => {
                egui::Panel::right("gcodekit5-designer-inspector")
                    .resizable(true)
                    .default_size(side)
                    .show(ui, |ui| {
                        let half = ui.available_height() * 0.5;
                        egui::Panel::bottom("gcodekit5-designer-objects")
                            .resizable(true)
                            .default_size(half)
                            .show(ui, |ui| frame.pane(ui, PaneId::DesignerLayers));
                        frame.pane(ui, PaneId::DesignerProperties);
                    });
                frame.pane(ui, PaneId::Designer);
            }
            Page::MachineControl => {
                egui::Panel::right("gcodekit5-machine-editor")
                    .resizable(true)
                    .default_size(side)
                    .show(ui, |ui| {
                        let console = ui.available_height() * 0.3;
                        egui::Panel::bottom("gcodekit5-machine-console")
                            .resizable(true)
                            .default_size(console)
                            .show(ui, |ui| frame.pane(ui, PaneId::Console));
                        frame.pane(ui, PaneId::Editor);
                    });
                frame.pane(ui, PaneId::MachineControl);
            }
            Page::Visualizer => frame.pane(ui, PaneId::Visualizer),
            Page::CamTools => frame.pane(ui, PaneId::CamTools),
            Page::DeviceManager | Page::DeviceConfig | Page::CncTools | Page::Materials => {}
        }
        if press.is_some() {
            self.focused = frame.pressed_in;
            self.active = frame.pressed_in.or(self.active);
        }
        self.drawn = frame.drawn;
        for pane in self
            .panes
            .iter_mut()
            .filter(|p| !self.drawn.contains(&p.id()))
        {
            pane.cancel_hold();
        }
    }
}

/// Paints one page tab: filled with the selection colour when it is the shown page, the
/// hover colour while the pointer is over it, and its label centred (cut short with an
/// ellipsis when it does not fit).
fn paint_tab(ui: &egui::Ui, response: &egui::Response, rect: egui::Rect, label: &str, on: bool) {
    let visuals = ui.visuals();
    let (fill, text) = if on {
        (visuals.selection.bg_fill, visuals.selection.stroke.color)
    } else if response.hovered() {
        (
            visuals.widgets.hovered.weak_bg_fill,
            visuals.widgets.hovered.fg_stroke.color,
        )
    } else {
        (
            visuals.widgets.inactive.weak_bg_fill,
            visuals.widgets.inactive.fg_stroke.color,
        )
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, fill);
    painter.vline(
        rect.right(),
        rect.y_range(),
        visuals.widgets.noninteractive.bg_stroke,
    );
    let mut job = egui::text::LayoutJob::single_section(
        label.to_owned(),
        egui::TextFormat::simple(egui::TextStyle::Button.resolve(ui.style()), text),
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width() - 8.0);
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let pos = rect.center() - galley.size() * 0.5;
    painter.galley(pos, galley, text);
}

/// Draws the panes of one page for one frame, noting which were drawn and which holds the
/// pointer press.
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

impl Content<'_, '_, '_> {
    /// Draws the pane `id` into the rest of `ui`.
    fn pane(&mut self, ui: &mut egui::Ui, id: PaneId) {
        egui::CentralPanel::default().show(ui, |ui| self.body(ui, id));
    }

    /// Draws the pane `id`'s body into `ui`.
    fn body(&mut self, ui: &mut egui::Ui, id: PaneId) {
        self.drawn.push(id);
        if self.press.is_some_and(|pos| ui.max_rect().contains(pos)) {
            self.pressed_in = Some(id);
        }
        if let Some(pane) = self.panes.iter_mut().find(|p| p.id() == id) {
            pane.ui(ui, self.ctx);
        }
    }
}
