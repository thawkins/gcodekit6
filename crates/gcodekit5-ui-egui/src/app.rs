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
//! File dialogs and confirmations go through [`Dialogs`]: queued by a command, shown at the
//! end of the frame and answered on a later one, so the event loop never blocks. The
//! Preferences and About windows are drawn over the workspace while open.
//!
//! Dependencies: `egui`, `eframe`, `craft-window`, `craft-platform`, `craft-keymap`,
//! `craft-menu`, `craft-dialogs`, `gcodekit5-app` (translations), `gcodekit5-settings`
//! (configuration).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use craft_keymap::egui::Dispatcher;
use craft_keymap::{Active, Keymap};
use craft_menu::Gate;
use craft_platform::appearance::SystemThemeFn;
use craft_window::policy::{Decision, Frame};
use gcodekit5_app::i18n::Translator;
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::Config;

use crate::commands::{self, Command};
use crate::dialogs::{
    ConfirmChoice, ConfirmRequest, DialogAnswer, Dialogs, FileFilter, FilePurpose,
};
use crate::menu::Menus;
use crate::theme;
use crate::windows::{AboutWindow, SettingsOutcome, SettingsWindow};
use crate::workspace::{PaneContext, Workspace};

/// State held by the egui front end.
#[derive(Debug)]
pub struct AppState {
    /// The loaded settings and where they are saved.
    pub settings: SettingsManager,
    /// Translator for the configured UI language.
    pub translator: Translator,
    /// craft-window's frame decision: who draws the title bar, the caption buttons' layout
    /// and look, and the desktop it was made for (whether "Use system title bar" matters).
    pub decision: Decision,
    /// Set when the user asked to restart; [`crate::run`] relaunches after the window closes.
    pub restart: Arc<AtomicBool>,
}

impl AppState {
    /// State over `settings`, with their UI language, framed as `decision` says.
    ///
    /// # Arguments
    /// * `settings` - The settings to use and save to.
    /// * `decision` - craft-window's frame decision for the window.
    pub fn new(settings: SettingsManager, decision: Decision) -> Self {
        let translator = Translator::new(&settings.config().ui.language);
        Self {
            settings,
            translator,
            decision,
            restart: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The current settings.
    pub fn config(&self) -> &Config {
        self.settings.config()
    }

    /// The saved settings, or defaults that are never saved when there is no settings
    /// folder.
    pub fn load_settings() -> SettingsManager {
        match SettingsManager::config_dirs() {
            Ok(dirs) => {
                let (settings, problem) = SettingsManager::open(dirs);
                if let Some(problem) = problem {
                    tracing::warn!(%problem, "settings loaded with problems");
                }
                settings
            }
            Err(error) => {
                tracing::warn!(%error, "no settings folder; using defaults");
                SettingsManager::detached(Config::default())
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
    /// File dialogs and confirmations, answered on a later frame.
    dialogs: Dialogs,
    /// The context [`GcodeKitApp::dialogs`] wakes when an answer comes in, once known.
    wake_ctx: Arc<OnceLock<egui::Context>>,
    /// The last dialog answer no feature has taken yet (until the editor and designer can).
    last_dialog_answer: Option<DialogAnswer>,
    /// The Preferences window.
    settings_window: SettingsWindow,
    /// The About window.
    about_window: AboutWindow,
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
            .field("dialogs", &self.dialogs)
            .field("last_dialog_answer", &self.last_dialog_answer)
            .field("settings_window", &self.settings_window)
            .field("about_window", &self.about_window)
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
        let wake_ctx: Arc<OnceLock<egui::Context>> = Arc::new(OnceLock::new());
        let wake = {
            let wake_ctx = Arc::clone(&wake_ctx);
            Arc::new(move || {
                if let Some(ctx) = wake_ctx.get() {
                    ctx.request_repaint();
                }
            })
        };
        let dialogs = Dialogs::new(state.translator, wake);
        let settings_window = SettingsWindow::new(
            crate::frame::title_bar_setting_applies(&state.decision),
            &state.config().ui,
        );
        let mut about_window = AboutWindow::default();
        if state.config().ui.show_about_on_startup {
            about_window.open();
        }
        Self {
            state,
            workspace: Workspace::default_layout(),
            system_theme: None,
            keymap,
            dispatcher: Dispatcher::new(),
            menus,
            queue: Vec::new(),
            viewport_commands: Vec::new(),
            dialogs,
            wake_ctx,
            last_dialog_answer: None,
            settings_window,
            about_window,
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
        let _ = self.wake_ctx.set(ctx.clone());
        while let Some(answer) = self.dialogs.poll() {
            self.handle_answer(answer);
        }
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
        self.show_windows(&ctx);
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
            Command::FileOpen => self
                .dialogs
                .open_file(FilePurpose::OpenGcode, &[FileFilter::GCODE]),
            // No document has a path of its own yet, so Save asks where to save too.
            Command::FileSave | Command::FileSaveAs => self
                .dialogs
                .save_file(FilePurpose::SaveGcode, &[FileFilter::GCODE]),
            Command::OpenSettings => self.settings_window.open(&self.state.settings),
            Command::About => self.about_window.open(),
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
            Command::OpenDeviceManager
            | Command::OpenToolsManager
            | Command::OpenMaterialsManager
            | Command::OpenHelp => tracing::debug!(?cmd, "window command not available yet"),
        }
    }

    /// Draws the Preferences and About windows while they are open, and acts on what
    /// Preferences decided.
    fn show_windows(&mut self, ctx: &egui::Context) {
        let outcome =
            self.settings_window
                .show(ctx, &mut self.state.settings, &self.state.translator);
        match outcome {
            SettingsOutcome::Applied { restart: true } => {
                self.dialogs.confirm(ConfirmRequest::Restart);
            }
            SettingsOutcome::Closed
            | SettingsOutcome::Open
            | SettingsOutcome::Cancelled
            | SettingsOutcome::Applied { restart: false }
            | SettingsOutcome::Failed(_) => {}
        }
        self.about_window.show(ctx, &self.state.translator);
    }

    /// Acts on a dialog's answer. Answers for features that do not exist yet (opening and
    /// saving files, Tasks 7 and 9) are kept in [`GcodeKitApp::last_dialog_answer`].
    fn handle_answer(&mut self, answer: DialogAnswer) {
        match &answer {
            DialogAnswer::Confirm {
                request: ConfirmRequest::Restart,
                choice,
            } => {
                if *choice == ConfirmChoice::Yes {
                    self.state.restart.store(true, Ordering::SeqCst);
                    self.viewport_commands.push(egui::ViewportCommand::Close);
                }
            }
            DialogAnswer::Unavailable { reason } => {
                tracing::warn!(%reason, "dialog unavailable");
            }
            DialogAnswer::File { .. }
            | DialogAnswer::Cancelled { .. }
            | DialogAnswer::Confirm { .. } => {
                tracing::debug!(?answer, "dialog answer not consumed yet");
            }
        }
        self.last_dialog_answer = Some(answer);
    }

    /// The dialog queue.
    pub fn dialogs(&self) -> &Dialogs {
        &self.dialogs
    }

    /// The dialog queue, for showing its requests at the end of a frame and for tests.
    pub fn dialogs_mut(&mut self) -> &mut Dialogs {
        &mut self.dialogs
    }

    /// The last dialog answer [`GcodeKitApp::ui`] took.
    pub fn last_dialog_answer(&self) -> Option<&DialogAnswer> {
        self.last_dialog_answer.as_ref()
    }

    /// The last command [`GcodeKitApp::dispatch`] ran.
    pub fn last_dispatched(&self) -> Option<Command> {
        self.last_dispatched
    }

    /// Shows the light/dark mode the settings and the desktop ask for.
    fn sync_theme(&self, ctx: &egui::Context) {
        let observed = self.system_theme.as_ref().and_then(|watch| watch(ctx));
        let mode = theme::resolve(
            self.state.config().ui.theme,
            observed.or(ctx.system_theme()),
        );
        if ctx.theme() != mode {
            theme::install(ctx);
            ctx.set_theme(mode);
        }
    }
}

impl eframe::App for GcodeKitApp {
    /// Draws the whole window.
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.ui(ui);
        self.dialogs.end_frame(Some(&*frame));
    }

    /// Feeds the native menu's key equivalents to egui before it reads the input.
    #[cfg(target_os = "macos")]
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.menus.raw_input(raw_input);
    }
}
