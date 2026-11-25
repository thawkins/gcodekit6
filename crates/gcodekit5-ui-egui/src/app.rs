//! Application state and root UI of the egui front end: the window frame, the light/dark
//! theme and the docking workspace.
//!
//! Who draws the title bar is decided by `craft_window::policy` before the window opens and
//! carried in [`AppState::decision`]; [`crate::frame`] draws what that decision leaves to the
//! app. The light/dark mode follows `craft_platform::appearance` unless the user forced one in
//! [`UiSettings::theme`](gcodekit5_settings::config::UiSettings::theme).
//!
//! Commands come from three places each frame: key presses, run through the keymap by
//! `craft_keymap::egui::Dispatcher` before any widget reads them; the menus (in-window or
//! native); and the panes ([`PaneContext::request`]). All of them end in
//! [`GcodeKitApp::dispatch`].
//!
//! Dependencies: `egui`, `eframe`, `craft-window`, `craft-platform`, `craft-keymap`,
//! `craft-menu`, `gcodekit5-app` (translations), `gcodekit5-settings` (configuration).

use craft_keymap::egui::Dispatcher;
use craft_keymap::{Active, Keymap};
use craft_menu::Gate;
use craft_platform::appearance::SystemThemeFn;
use craft_window::policy::{Decision, Frame};
use gcodekit5_app::i18n::Translator;
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::Config;

use crate::commands::{self, Command, DialogRequest};
use crate::menu::Menus;
use crate::theme;
use crate::workspace::{PaneContext, Workspace};

/// State held by the egui front end.
#[derive(Debug)]
pub struct AppState {
    /// The loaded settings.
    pub config: Config,
    /// Translator for the configured UI language.
    pub translator: Translator,
    /// craft-window's frame decision: who draws the title bar, the caption buttons' layout
    /// and look, and the desktop it was made for (whether "Use system title bar" matters).
    pub decision: Decision,
}

impl AppState {
    /// State over `config`, with its UI language, framed as `decision` says.
    ///
    /// # Arguments
    /// * `config` - The settings to use.
    /// * `decision` - craft-window's frame decision for the window.
    pub fn new(config: Config, decision: Decision) -> Self {
        let translator = Translator::new(&config.ui.language);
        Self {
            config,
            translator,
            decision,
        }
    }

    /// The saved settings, or defaults when they cannot be read.
    pub fn load_config() -> Config {
        match SettingsManager::config_dirs() {
            Ok(dirs) => {
                let (config, problem) = SettingsManager::load(&dirs);
                if let Some(problem) = problem {
                    tracing::warn!(%problem, "settings loaded with problems");
                }
                config
            }
            Err(error) => {
                tracing::warn!(%error, "no settings folder; using defaults");
                Config::default()
            }
        }
    }
}

/// Root application of the egui front end.
pub struct GcodeKitApp {
    state: AppState,
    workspace: Workspace,
    system_theme: Option<SystemThemeFn>,
    /// Key bindings of every command.
    keymap: Keymap,
    /// Runs the frame's key presses through [`GcodeKitApp::keymap`].
    dispatcher: Dispatcher,
    /// The menus, laid out for this platform.
    menus: Menus,
    /// Commands asked for this frame, run once the UI is drawn.
    queue: Vec<Command>,
    /// Window commands the dispatched commands asked for, sent at the end of the frame.
    viewport_commands: Vec<egui::ViewportCommand>,
    /// A file dialog a command asked for; the dialog layer (Task 6) takes it.
    pending_dialog: Option<DialogRequest>,
    /// The last command dispatched.
    last_dispatched: Option<Command>,
    /// The `movable` flag last set on the macOS window (`title_bar::sync_movable`).
    #[cfg(target_os = "macos")]
    movable: Option<bool>,
}

impl std::fmt::Debug for GcodeKitApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcodeKitApp")
            .field("state", &self.state)
            .field("workspace", &self.workspace)
            .field("menus", &self.menus)
            .field("pending_dialog", &self.pending_dialog)
            .field("last_dispatched", &self.last_dispatched)
            .finish_non_exhaustive()
    }
}

impl GcodeKitApp {
    /// Creates the application for a new native window, watching the desktop's light/dark
    /// preference.
    ///
    /// # Arguments
    /// * `cc` - eframe's creation context.
    /// * `state` - The front end's state.
    pub fn new(cc: &eframe::CreationContext, state: AppState) -> Self {
        theme::install(&cc.egui_ctx);
        #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
        let mut app = Self {
            system_theme: craft_platform::appearance::egui_service(),
            ..Self::from_state(state)
        };
        #[cfg(target_os = "macos")]
        app.menus.install_native(&cc.egui_ctx);
        app
    }

    /// Creates the application without a desktop appearance watcher (headless use).
    ///
    /// # Arguments
    /// * `state` - The front end's state.
    pub fn from_state(state: AppState) -> Self {
        let keymap = commands::keymap();
        let menus = Menus::new(&state.translator, &keymap);
        Self {
            state,
            workspace: Workspace::default_layout(),
            system_theme: None,
            keymap,
            dispatcher: Dispatcher::new(),
            menus,
            queue: Vec::new(),
            viewport_commands: Vec::new(),
            pending_dialog: None,
            last_dispatched: None,
            #[cfg(target_os = "macos")]
            movable: None,
        }
    }

    /// Draws the application UI into `ui`.
    ///
    /// # Arguments
    /// * `ui` - The egui `Ui` to draw into.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.sync_theme(ui.ctx());
        let ctx = ui.ctx().clone();
        self.dispatch_keys(&ctx);
        let gate = Gate {
            text_editing: ctx.text_edit_focused(),
            ..Gate::default()
        };
        let mut queue = std::mem::take(&mut self.queue);
        queue.extend(self.menus.take_native(&ctx, gate));
        self.show_frame_and_menu(ui, gate, &mut queue);
        let mut panes = PaneContext::new(&self.state.translator, &mut queue);
        self.workspace.show(ui, &mut panes);
        for command in queue.drain(..) {
            self.dispatch(command);
        }
        self.queue = queue;
        for command in self.viewport_commands.drain(..) {
            ctx.send_viewport_cmd(command);
        }
        #[cfg(target_os = "macos")]
        if matches!(
            self.state.decision.frame,
            craft_window::policy::Frame::Inset(_)
        ) {
            craft_window::title_bar::sync_movable(ui.ctx(), &mut self.movable);
        }
    }

    /// Runs the frame's key presses through the keymap, before any widget reads them. While a
    /// menu of the in-window bar is open, or the bar has the keyboard, the keys stay with it.
    fn dispatch_keys(&mut self, ctx: &egui::Context) {
        craft_menu::bar::leave_for_shortcut(ctx);
        if craft_menu::bar::is_open(ctx) || craft_menu::bar::is_focused(ctx) {
            return;
        }
        // No pane reports keyboard focus yet, so panel-scoped bindings (jogging, the
        // visualizer's view keys) wait for the panes that track it.
        let active = Active {
            typing: ctx.text_edit_focused(),
            ..Active::NONE
        };
        let hits = ctx.input_mut(|input| self.dispatcher.dispatch(input, &self.keymap, &active));
        self.queue.extend(
            hits.iter()
                .filter_map(|hit| Command::from_ref(&hit.command)),
        );
    }

    /// Draws the window frame and the in-window menu bar (unless the native menu is up): in
    /// the title-bar strip when the app draws one, else in a top panel of its own.
    fn show_frame_and_menu(&self, ui: &mut egui::Ui, gate: Gate, queue: &mut Vec<Command>) {
        let tr = &self.state.translator;
        let frame = &self.state.decision.frame;
        let menus = &self.menus;
        let in_window = !menus.has_native();
        let mut draw = |ui: &mut egui::Ui, max_width: f32| {
            queue.extend(menus.show(ui, tr, gate, max_width));
        };
        let in_strip = in_window && !matches!(frame, Frame::Native);
        let slot: Option<crate::frame::MenuSlot<'_>> =
            if in_strip { Some(&mut draw) } else { None };
        crate::frame::show(ui, tr, frame, slot);
        if in_window && matches!(frame, Frame::Native) {
            egui::Panel::top("gcodekit5-menu-bar").show(ui, |ui| {
                draw(ui, f32::INFINITY);
            });
        }
    }

    /// Runs `cmd`. Commands whose features land in later milestones are logged, one arm per
    /// group, and recorded in [`GcodeKitApp::last_dispatched`] like every other command.
    ///
    /// # Arguments
    /// * `cmd` - The command to run.
    pub fn dispatch(&mut self, cmd: Command) {
        self.last_dispatched = Some(cmd);
        match cmd {
            Command::FileOpen => self.pending_dialog = Some(DialogRequest::Open),
            // No document has a path of its own yet, so Save asks where to save too.
            Command::FileSave | Command::FileSaveAs => {
                self.pending_dialog = Some(DialogRequest::SaveAs);
            }
            Command::Quit => self.viewport_commands.push(egui::ViewportCommand::Close),
            Command::Cut => self
                .viewport_commands
                .push(egui::ViewportCommand::RequestCut),
            Command::Copy => self
                .viewport_commands
                .push(egui::ViewportCommand::RequestCopy),
            Command::Paste => self
                .viewport_commands
                .push(egui::ViewportCommand::RequestPaste),
            Command::FileNew
            | Command::FileNew3d
            | Command::FileImport
            | Command::FileImportImage
            | Command::FileExportGcode
            | Command::FileExportSvg
            | Command::FileRun
            | Command::Undo
            | Command::Redo => tracing::debug!(?cmd, "document command not available yet"),
            Command::Connect
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
            | Command::JogZNeg => tracing::debug!(?cmd, "machine command not available yet"),
            Command::ZoomFit
            | Command::ZoomIn
            | Command::ZoomOut
            | Command::ResetView
            | Command::ViewTop
            | Command::ViewFront
            | Command::ViewIso => tracing::debug!(?cmd, "view command not available yet"),
            Command::OpenSettings
            | Command::OpenDeviceManager
            | Command::OpenToolsManager
            | Command::OpenMaterialsManager
            | Command::OpenHelp
            | Command::About => tracing::debug!(?cmd, "window command not available yet"),
        }
    }

    /// The file dialog a command asked for and no dialog has taken yet.
    pub fn pending_dialog(&self) -> Option<DialogRequest> {
        self.pending_dialog
    }

    /// The last command [`GcodeKitApp::dispatch`] ran.
    pub fn last_dispatched(&self) -> Option<Command> {
        self.last_dispatched
    }

    /// Shows the light/dark mode the settings and the desktop ask for.
    fn sync_theme(&self, ctx: &egui::Context) {
        let observed = self.system_theme.as_ref().and_then(|watch| watch(ctx));
        let mode = theme::resolve(self.state.config.ui.theme, observed.or(ctx.system_theme()));
        if ctx.theme() != mode {
            theme::install(ctx);
            ctx.set_theme(mode);
        }
    }
}

impl eframe::App for GcodeKitApp {
    /// Draws the whole window.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui(ui);
    }

    /// Feeds the native menu's key equivalents to egui before it reads the input.
    #[cfg(target_os = "macos")]
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.menus.raw_input(raw_input);
    }
}
