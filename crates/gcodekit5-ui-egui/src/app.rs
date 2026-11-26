//! Application state and root UI of the egui front end: the window frame, the light/dark
//! theme, the page tabs and the page shown ([`Workspace`]), and the status bar.
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
//! Preferences, About, Device Info and Help windows are drawn over the page while open. The
//! manager pages (Device Manager, Device Config, CNC Tools, Materials) are drawn by the app
//! ([`GcodeKitApp::show_page`] shows one); their unsaved forms are asked about on quit.
//!
//! The machine connection ([`AppState::machine`]) is polled once per frame, in
//! [`GcodeKitApp::logic`] or, when only [`GcodeKitApp::ui`] runs, there; its events and status
//! reach the panes through [`PaneContext::with_machine`]. Machine commands are run by the
//! Machine Control pane ([`Workspace::route`]).
//!
//! Quitting goes through [`GcodeKitApp::request_quit`] whatever asked for it (the window's
//! close button, the Quit command, a confirmed restart; session end is not hooked, see
//! [`QuitSource`]). A running job is asked about
//! first, then unsaved work; while a prompt is up the window is held open with
//! `ViewportCommand::CancelClose`. Once quitting is decided the app sends
//! `ViewportCommand::Close`, and `craft_quit::on_close_request` (in `eframe::App::logic`)
//! turns that close into `-[NSApplication terminate:]` on macOS.
//!
//! The designer's file commands, its unsaved-changes questions and the G-code it hands to the
//! editor are in [`designer_files`].
//! A CAM page's preset Load / Save dialogs are in `cam_presets`.
//!
//! Dependencies: `egui`, `eframe`, `craft-window`, `craft-platform`, `craft-keymap`,
//! `craft-menu`, `craft-dialogs`, `craft-quit`, `gcodekit5-app` (translations, the
//! unsaved-changes answer), `gcodekit5-settings` (configuration).

mod cam_presets;
pub mod designer_files;
mod library_windows;
mod machine_windows;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use craft_keymap::egui::Dispatcher;
use craft_keymap::{Active, Keymap};
use craft_menu::Gate;
use craft_platform::appearance::SystemThemeFn;
use craft_window::policy::{Decision, Frame};
use gcodekit5_app::UnsavedChangesChoice;
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::{MachineController, MachineEvent};
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::Config;

use crate::commands::{self, Command};
use crate::dialogs::{
    Asked, ConfirmChoice, ConfirmRequest, DialogAnswer, Dialogs, FileFilter, FilePurpose,
};
use crate::machine::{Machine, lock};
use crate::menu::Menus;
use crate::panes::cam_tools::{CamToolsPane, SharedCamTools};
use crate::panes::console::ConsolePane;
use crate::panes::designer::layers::LayersPane;
use crate::panes::designer::properties::PropertiesPane;
use crate::panes::designer::{DesignerPane, SharedDesigner};
use crate::panes::editor::{EditorPane, SharedEditor};
use crate::panes::machine_control::MachineControlPane;
use crate::panes::visualizer::VisualizerPane;
use crate::theme;
use crate::windows::{
    AboutWindow, DeviceInfoPanel, DeviceManagerWindow, FirmwareSettingsWindow, HelpWindow,
    MaterialsManagerWindow, SettingsOutcome, SettingsWindow, ToolsManagerWindow, help,
};
use crate::workspace::{JogKey, Page, PaneContext, PaneId, Workspace};
use designer_files::{DesignFlow, DesignNext, EditorNext, OPEN_BEHIND_HANDOFF};
use gcodekit5_app::materials_manager_backend::MATERIALS_FILE;
use gcodekit5_app::tools_manager_backend::TOOLS_FILE;

/// The GTK build's device profiles file, in the legacy settings folder.
const DEVICES_FILE: &str = "devices.json";

/// How long quitting waits for a stopped job to be confirmed by the controller.
const STOP_JOB_TIMEOUT: Duration = Duration::from_secs(2);

/// What asked the app to quit. Every source goes through [`GcodeKitApp::request_quit`] and is
/// treated the same; the source is logged.
// `Os` is a real, reserved source hidden from the docs, not a non-exhaustive marker.
#[allow(clippy::manual_non_exhaustive)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuitSource {
    /// The window's close button (a close request from the window system).
    WindowClose,
    /// The Quit command (menu, keyboard shortcut) or a confirmed restart.
    MenuQuit,
    /// Reserved for a session-end hook (log-off, shutdown); nothing sends it yet.
    ///
    /// What happens on session end today, with no hook (winit 0.30 handles none):
    /// - Windows: winit does not handle `WM_QUERYENDSESSION` / `WM_ENDSESSION`, so log-off
    ///   ends the process without a close request and nothing is asked. A hook through
    ///   eframe's `with_msg_hook` would have to read a raw `MSG` pointer, which needs
    ///   `unsafe`, so it is not added.
    /// - macOS: winit implements no `applicationShouldTerminate:`, so logout goes straight
    ///   to `-[NSApplication terminate:]` without asking. The hook (answering
    ///   `NSTerminateCancel` while a prompt is up) belongs in craft-quit upstream.
    /// - Linux and the BSDs: the compositor closes the window (a close request, handled as
    ///   [`QuitSource::WindowClose`]) or the session ends the process.
    ///
    /// The GTK front end did not handle session end either.
    #[doc(hidden)]
    Os,
}

/// Which documents have unsaved changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DirtyState {
    /// The G-code editor has unsaved changes.
    pub editor: bool,
    /// The design has unsaved changes.
    pub designer: bool,
}

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
    /// Set when the user asked to restart; the app relaunches itself when it exits
    /// (`eframe::App::on_exit`). Cleared when the quit is cancelled.
    pub restart: Arc<AtomicBool>,
    /// Which documents have unsaved changes; asked about on quit. The app copies the editor's
    /// and the designer's flags here whenever they change.
    pub dirty: DirtyState,
    /// The machine connection: the shared controller, this frame's events and the state
    /// derived from them. Replace it (with [`AppState::with_machine`]) before the app is built
    /// from the state, since the Machine Control pane shares its controller.
    pub machine: Machine,
    /// The device profiles file (the GTK build's `devices.json` in the legacy settings
    /// folder); `None` without a settings folder. Replace it (with
    /// [`AppState::with_device_file`]) before the app is built from the state.
    pub device_file: Option<PathBuf>,
    /// The custom tools file (the GTK build's `custom_tools.json` in the legacy settings
    /// folder, where `ToolsManagerBackend::default_storage_path` puts it); `None` without a
    /// settings folder. Replace it (with [`AppState::with_tools_file`]) before the app is
    /// built from the state.
    pub tools_file: Option<PathBuf>,
    /// The custom materials file (the GTK build's `custom_materials.json` in the legacy
    /// settings folder); `None` without a settings folder. Replace it (with
    /// [`AppState::with_materials_file`]) before the app is built from the state.
    pub materials_file: Option<PathBuf>,
    /// The console's command history file (the GTK build's `command_history.json` in the
    /// legacy settings folder); `None` without a settings folder, which keeps the history
    /// for the session. Replace it (with [`AppState::with_history_file`]) before the app is
    /// built from the state.
    pub history_file: Option<PathBuf>,
    /// The Designer's stock and tool for the program it last generated, which the
    /// visualizer's stock removal simulation takes (GTK's `sync_stock_from_designer`).
    pub stock_seed: Option<crate::panes::visualizer::StockSeed>,
}

impl AppState {
    /// State over `settings`, with their UI language, framed as `decision` says.
    ///
    /// # Arguments
    /// * `settings` - The settings to use and save to.
    /// * `decision` - craft-window's frame decision for the window.
    pub fn new(settings: SettingsManager, decision: Decision) -> Self {
        let translator = Translator::new(&settings.config().ui.language);
        let device_file = settings.dirs().map(|dirs| dirs.legacy.join(DEVICES_FILE));
        let tools_file = settings.dirs().map(|dirs| dirs.legacy.join(TOOLS_FILE));
        let materials_file = settings.dirs().map(|dirs| dirs.legacy.join(MATERIALS_FILE));
        let history_file = settings.dirs().map(|dirs| {
            dirs.legacy
                .join(gcodekit5_app::command_history::HISTORY_FILE)
        });
        Self {
            settings,
            translator,
            decision,
            restart: Arc::new(AtomicBool::new(false)),
            dirty: DirtyState::default(),
            machine: Machine::new(MachineController::new()),
            device_file,
            tools_file,
            materials_file,
            history_file,
            stock_seed: None,
        }
    }

    /// The same state keeping its custom tools in `file` (a test folder, for example).
    ///
    /// # Arguments
    /// * `file` - The custom tools file.
    pub fn with_tools_file(mut self, file: PathBuf) -> Self {
        self.tools_file = Some(file);
        self
    }

    /// The same state keeping its custom materials in `file` (a test folder, for example).
    ///
    /// # Arguments
    /// * `file` - The custom materials file.
    pub fn with_materials_file(mut self, file: PathBuf) -> Self {
        self.materials_file = Some(file);
        self
    }

    /// The same state keeping the console's command history in `file` (a test folder, for
    /// example).
    ///
    /// # Arguments
    /// * `file` - The command history file.
    pub fn with_history_file(mut self, file: PathBuf) -> Self {
        self.history_file = Some(file);
        self
    }

    /// The same state keeping its device profiles in `file` (a test folder, for example).
    ///
    /// # Arguments
    /// * `file` - The device profiles file.
    pub fn with_device_file(mut self, file: PathBuf) -> Self {
        self.device_file = Some(file);
        self
    }

    /// The same state driving `controller` (a test transport, for example).
    ///
    /// # Arguments
    /// * `controller` - The machine controller to use.
    pub fn with_machine(mut self, controller: MachineController) -> Self {
        self.machine = Machine::new(controller);
        self
    }

    /// Whether a job is streaming to the machine; asked about on quit.
    pub fn job_running(&self) -> bool {
        self.machine.job_running()
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

/// Where the quit decision stands.
#[derive(Debug, Default)]
struct QuitFlow {
    /// The quit prompt queued or on screen, while a quit is being decided.
    asking: Option<ConfirmRequest>,
    /// The user agreed to stop the running job for this quit.
    stop_job: bool,
    /// The documents whose changes the user chose to discard for this quit.
    discarded: DirtyState,
    /// The user chose to discard the CNC Tools page's unsaved tool for this quit.
    discarded_tool: bool,
    /// The user chose to discard the Materials Manager's unsaved material for this quit.
    discarded_material: bool,
    /// The user chose to discard the Device Manager's unsaved profile for this quit.
    discarded_device: bool,
    /// Quitting was decided: the app sent `Close` and lets the close request through.
    confirmed: bool,
}

/// What to do once the editor's G-code is saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AfterSave {
    /// Go on quitting (the quit prompt's Save).
    Quit,
    /// Ask for a file to open (Open with unsaved changes, Save chosen).
    Open,
    /// Hand a program to the editor (the designer's Generate / Frame, or a CAM tool's Send
    /// to Editor / Preview, over unsaved edits).
    ToEditor(designer_files::Handoff),
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
    /// The help browser.
    help_window: HelpWindow,
    /// The Device Config page's firmware settings.
    firmware_settings: FirmwareSettingsWindow,
    /// The Device Info panel (the left column of the Device Config page).
    device_info: DeviceInfoPanel,
    /// The Device Manager page, which also holds the device profiles.
    device_manager: DeviceManagerWindow,
    /// The CNC Tools page, which also holds the tool library.
    tools_manager: ToolsManagerWindow,
    /// The Materials page, which also holds the material library.
    materials_manager: MaterialsManagerWindow,
    /// The last command dispatched.
    last_dispatched: Option<Command>,
    /// The quit being decided, if any.
    quit: QuitFlow,
    /// The G-code editor, also drawn by the workspace.
    editor: Rc<RefCell<EditorPane>>,
    /// The editor's dirty flag as last copied to [`AppState::dirty`].
    editor_dirty: bool,
    /// The designer, also drawn by the workspace.
    designer: Rc<RefCell<DesignerPane>>,
    /// The design's modified flag as last copied to [`AppState::dirty`].
    designer_dirty: bool,
    /// The CAM tools, also drawn by the workspace.
    cam_tools: Rc<RefCell<CamToolsPane>>,
    /// What waits for the editor's save dialog to be answered.
    after_save: Option<AfterSave>,
    /// What waits for the editor's unsaved-changes prompt (File > Open or a handoff); one
    /// at a time.
    editor_prompt: Option<EditorNext>,
    /// The design's New / Open / Save waiting for a dialog.
    design: DesignFlow,
    /// The egui frame whose machine events were last polled.
    machine_polled: Option<u64>,
    /// The key that ran each jog command of this frame's key presses (see
    /// [`PaneContext::with_jog_key`]).
    jog_keys: Vec<(Command, JogKey)>,
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
            .field("help_window", &self.help_window)
            .field("last_dispatched", &self.last_dispatched)
            .field("quit", &self.quit)
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
        crate::icons::install(&cc.egui_ctx);
        match &cc.wgpu_render_state {
            Some(render_state) => crate::panes::visualizer::install(render_state),
            None => tracing::warn!("no wgpu renderer; the visualizer shows no toolpath"),
        }
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
        let mut workspace = Workspace::new();
        workspace.set_pane(Box::new(machine_control_pane(&state)));
        workspace.set_pane(Box::new(
            ConsolePane::new(Arc::clone(state.machine.controller()))
                .with_history_file(state.history_file.clone()),
        ));
        let editor = Rc::new(RefCell::new(EditorPane::new()));
        workspace.set_pane(Box::new(SharedEditor(Rc::clone(&editor))));
        workspace.set_pane(Box::new(VisualizerPane::new()));
        let designer = Rc::new(RefCell::new(DesignerPane::new()));
        workspace.set_pane(Box::new(SharedDesigner(Rc::clone(&designer))));
        workspace.set_pane(Box::new(LayersPane::new(Rc::clone(&designer))));
        workspace.set_pane(Box::new(PropertiesPane::new(Rc::clone(&designer))));
        let device_manager = DeviceManagerWindow::new(state.device_file.clone());
        let tools_manager = ToolsManagerWindow::new(state.tools_file.clone(), state.config());
        let materials_manager = MaterialsManagerWindow::new(state.materials_file.clone());
        let cam_tools = Rc::new(RefCell::new(CamToolsPane::new()));
        cam_tools.borrow_mut().set_libraries(
            tools_manager.tools(),
            materials_manager.materials(),
            device_manager.active_profile(),
        );
        workspace.set_pane(Box::new(SharedCamTools::new(Rc::clone(&cam_tools))));
        workspace.configure(state.config());
        workspace.set_device(device_manager.active_profile().as_ref());
        {
            // Wake the event loop whenever the machine reports something.
            let wake_ctx = Arc::clone(&wake_ctx);
            lock(state.machine.controller()).set_repaint_hook(Box::new(move || {
                if let Some(ctx) = wake_ctx.get() {
                    ctx.request_repaint();
                }
            }));
        }
        Self {
            state,
            workspace,
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
            help_window: HelpWindow::default(),
            firmware_settings: FirmwareSettingsWindow::default(),
            device_info: DeviceInfoPanel::default(),
            device_manager,
            tools_manager,
            materials_manager,
            last_dispatched: None,
            quit: QuitFlow::default(),
            editor,
            editor_dirty: false,
            designer,
            designer_dirty: false,
            cam_tools,
            after_save: None,
            editor_prompt: None,
            design: DesignFlow::default(),
            machine_polled: None,
            jog_keys: Vec::new(),
            #[cfg(target_os = "macos")]
            movable: None,
        }
    }

    /// The frame's quit and dialog logic, run before [`GcodeKitApp::ui`] and also while the
    /// window is minimized or hidden (eframe calls `logic` then, not `ui`): takes the dialog
    /// answers, turns a close request into [`GcodeKitApp::request_quit`] and sends the window
    /// commands that asked for.
    ///
    /// # Arguments
    /// * `ctx` - The egui context of the root window.
    pub fn logic(&mut self, ctx: &egui::Context) {
        let _ = self.wake_ctx.set(ctx.clone());
        self.poll_machine(ctx);
        self.cancel_holds_without_focus(ctx);
        while let Some(answer) = self.dialogs.poll() {
            self.handle_answer(answer);
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            self.request_quit(QuitSource::WindowClose);
        }
        for command in self.viewport_commands.drain(..) {
            ctx.send_viewport_cmd(command);
        }
    }

    /// Quits, after asking: a running job first ("Stop job and quit?"), then unsaved changes
    /// in the editor and in the design (Save / Discard / Cancel). While a prompt is up the
    /// window is held open (`ViewportCommand::CancelClose`); with nothing to ask, or once
    /// every prompt is answered to go on, the window is closed (`ViewportCommand::Close`).
    /// A cancelled quit forgets a pending restart.
    ///
    /// # Arguments
    /// * `source` - What asked to quit.
    pub fn request_quit(&mut self, source: QuitSource) {
        tracing::debug!(?source, "quit requested");
        if self.quit.confirmed {
            // Our own Close comes back as a close request: let it through.
            if source != QuitSource::WindowClose {
                self.viewport_commands.push(egui::ViewportCommand::Close);
            }
            return;
        }
        // While a save dialog for this quit is up, the quit waits for it.
        let saving = self.after_save == Some(AfterSave::Quit) || self.design_saving_for_quit();
        if self.quit.asking.is_none() && !saving {
            self.continue_quit();
        }
        if self.quit.asking.is_some() || saving {
            self.viewport_commands
                .push(egui::ViewportCommand::CancelClose);
        }
    }

    /// Asks the next quit question, or closes the window when none is left.
    fn continue_quit(&mut self) {
        let (state, quit) = (&self.state, &self.quit);
        let next = if state.job_running() && !quit.stop_job {
            Some(ConfirmRequest::StopJob)
        } else if state.dirty.editor && !quit.discarded.editor {
            Some(ConfirmRequest::UnsavedGcode)
        } else if state.dirty.designer && !quit.discarded.designer {
            Some(ConfirmRequest::UnsavedDesign)
        } else if self.tools_manager.has_unsaved_changes() && !quit.discarded_tool {
            Some(ConfirmRequest::UnsavedTool)
        } else if self.materials_manager.has_unsaved_changes() && !quit.discarded_material {
            Some(ConfirmRequest::UnsavedMaterial)
        } else if self.device_manager.has_unsaved_changes() && !quit.discarded_device {
            Some(ConfirmRequest::UnsavedDevice)
        } else {
            None
        };
        match next {
            Some(request) => {
                self.quit.asking = Some(request);
                self.dialogs.confirm_quit(request);
                // A minimized or hidden window cannot show its prompt: bring it back first.
                self.viewport_commands.extend([
                    egui::ViewportCommand::Minimized(false),
                    egui::ViewportCommand::Visible(true),
                    egui::ViewportCommand::Focus,
                ]);
            }
            None => {
                self.quit = QuitFlow {
                    confirmed: true,
                    ..QuitFlow::default()
                };
                self.viewport_commands.push(egui::ViewportCommand::Close);
            }
        }
    }

    /// Acts on the answer to the quit prompt `request`.
    fn quit_answer(&mut self, request: ConfirmRequest, choice: ConfirmChoice) {
        self.quit.asking = None;
        match (request, choice) {
            (ConfirmRequest::StopJob, ConfirmChoice::Yes) => {
                self.quit.stop_job = true;
                self.stop_job_for_quit();
                self.continue_quit();
            }
            (ConfirmRequest::StopJob, ConfirmChoice::No | ConfirmChoice::Cancel) => {
                self.cancel_quit();
            }
            (ConfirmRequest::UnsavedGcode | ConfirmRequest::UnsavedDesign, choice) => {
                match choice.as_unsaved() {
                    UnsavedChangesChoice::Discard => {
                        if request == ConfirmRequest::UnsavedGcode {
                            self.quit.discarded.editor = true;
                        } else {
                            self.quit.discarded.designer = true;
                        }
                        self.continue_quit();
                    }
                    UnsavedChangesChoice::Save if request == ConfirmRequest::UnsavedGcode => {
                        self.save_editor(Some(AfterSave::Quit));
                    }
                    UnsavedChangesChoice::Save => self.save_design(Some(DesignNext::Quit)),
                    UnsavedChangesChoice::Cancel => self.cancel_quit(),
                }
            }
            (ConfirmRequest::UnsavedTool | ConfirmRequest::UnsavedMaterial, choice) => {
                self.library_quit_answer(request, choice.as_unsaved());
            }
            (ConfirmRequest::UnsavedDevice, choice) => self.device_quit_answer(choice.as_unsaved()),
            // The quit flow never asks this; forget the quit rather than act on it.
            (
                ConfirmRequest::Restart
                | ConfirmRequest::ZeroAllAxes(_)
                | ConfirmRequest::ClearEditor,
                _,
            ) => self.cancel_quit(),
        }
    }

    /// Stops the running job (soft reset, queued lines dropped) and waits until the controller
    /// reports the stream ended or the connection closed, at most [`STOP_JOB_TIMEOUT`]; the
    /// events seen meanwhile join this frame's events.
    fn stop_job_for_quit(&mut self) {
        let controller = Arc::clone(self.state.machine.controller());
        lock(&controller).stop();
        let deadline = Instant::now() + STOP_JOB_TIMEOUT;
        loop {
            let events = lock(&controller).poll();
            let ended = events.iter().any(|e| {
                matches!(
                    e,
                    MachineEvent::StreamStopped { .. }
                        | MachineEvent::StreamFinished { .. }
                        | MachineEvent::Disconnected
                )
            });
            self.state.machine.absorb(events);
            if ended || !self.state.job_running() {
                return;
            }
            if Instant::now() >= deadline {
                tracing::warn!("job did not confirm the stop in time; quitting anyway");
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Abandons the quit: the app stays open and a pending restart is forgotten, so a later
    /// quit does not relaunch the app.
    fn cancel_quit(&mut self) {
        tracing::debug!("quit cancelled");
        self.quit = QuitFlow::default();
        self.state.restart.store(false, Ordering::SeqCst);
    }

    /// Asks whether to restart now to apply changed settings; Yes quits through
    /// [`GcodeKitApp::request_quit`] and relaunches the app once it exits.
    pub fn request_restart(&mut self) {
        self.dialogs.confirm(ConfirmRequest::Restart);
    }

    /// Whether quitting was decided (the app sent `ViewportCommand::Close`).
    pub fn quit_confirmed(&self) -> bool {
        self.quit.confirmed
    }

    /// Draws the application UI into `ui`.
    ///
    /// # Arguments
    /// * `ui` - The egui `Ui` to draw into.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.sync_theme(ui.ctx());
        let ctx = ui.ctx().clone();
        let _ = self.wake_ctx.set(ctx.clone());
        self.poll_machine(&ctx);
        self.cancel_holds_without_focus(&ctx);
        while let Some(answer) = self.dialogs.poll() {
            self.handle_answer(answer);
        }
        self.workspace.begin_frame(&ctx);
        self.dispatch_keys(&ctx);
        self.update_firmware_settings(&ctx);
        self.workspace.frame(&self.state.machine);
        let gate = Gate {
            text_editing: ctx.text_edit_focused(),
            ..Gate::default()
        };
        let mut queue = std::mem::take(&mut self.queue);
        queue.extend(self.menus.take_native(&ctx, gate));
        self.show_frame_and_menu(ui, gate, &mut queue);
        let tab = egui::Panel::top("gcodekit5-page-tabs")
            .show(ui, |ui| {
                self.workspace.show_tabs(ui, &self.state.translator)
            })
            .inner;
        if let Some(page) = tab {
            self.show_page(page);
        }
        let profile = self.device_manager.active_profile();
        egui::Panel::bottom("gcodekit5-status-bar").show(ui, |ui| {
            if crate::status_bar::show(
                ui,
                &self.state.translator,
                self.state.machine.status(),
                self.state.config().ui.measurement_system,
                self.state.config().ui.feed_rate_units,
                profile.as_ref(),
            ) {
                queue.push(Command::SoftReset);
            }
        });
        let mut panes = PaneContext::new(&self.state.translator, &mut queue)
            .with_machine(&self.state.machine)
            .with_stock_seed(self.state.stock_seed);
        self.workspace.show(ui, &mut panes);
        if self.workspace.page().panes().is_empty() {
            egui::CentralPanel::default().show(ui, |ui| self.show_manager_page(ui));
        }
        for request in self.workspace.take_confirms() {
            self.dialogs.confirm(request);
        }
        for command in queue.drain(..) {
            self.dispatch(command);
        }
        self.sync_editor(false);
        self.sync_designer();
        self.deliver_editor_gcode();
        self.deliver_designer_shapes();
        self.ask_cam_preset();
        self.ask_cam_folder();
        self.ask_cam_image();
        self.deliver_machine_events();
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
        let active = Active {
            typing: ctx.text_edit_focused(),
            panel: self.keyboard_panel(ctx),
            ..Active::NONE
        };
        let (hits, modifiers) = ctx.input_mut(|input| {
            let hits = self.dispatcher.dispatch(input, &self.keymap, &active);
            (hits, input.modifiers)
        });
        // The key of each jog a key ran, so a continuous jog can last while it is held.
        self.jog_keys.clear();
        for command in hits.iter().filter_map(|hit| {
            let command = Command::from_ref(&hit.command)?;
            if command.is_jog() {
                let key = hit.trigger.chord.key.to_egui();
                self.jog_keys.push((command, JogKey { key, modifiers }));
            }
            Some(command)
        }) {
            self.queue.push(command);
        }
    }

    /// The keymap scope of the pane that has the keyboard: the pane the user last pressed the
    /// pointer in, but only while no widget has keyboard focus and no popup (an open dropdown)
    /// is up, so a pane's keys never take a key from a focused widget.
    fn keyboard_panel(&self, ctx: &egui::Context) -> Option<&'static str> {
        // A window, popup or modal on screen (or closed only last frame) has the user's
        // attention: no pane takes plain keys then.
        let widget_has_keys = ctx.memory(|m| m.focused().is_some())
            || egui::Popup::is_any_open(ctx)
            || !crate::workspace::overlay_layers(ctx).is_empty();
        if widget_has_keys {
            return None;
        }
        // Only a pane on the page shown, drawn last frame: a page change by any path (a tab,
        // a command, a quit prompt's Cancel) leaves the keys with no hidden pane.
        self.workspace
            .focused_pane()
            .filter(|id| self.workspace.has_keyboard(*id))
            .and_then(PaneId::keymap_scope)
    }

    /// Takes the machine controller's events once per egui frame and fans them out
    /// ([`AppState::machine`]); a second call in the same frame does nothing.
    fn poll_machine(&mut self, ctx: &egui::Context) {
        let frame = ctx.cumulative_frame_nr();
        if self.machine_polled != Some(frame) {
            self.machine_polled = Some(frame);
            self.state.machine.poll();
        }
        self.deliver_machine_events();
        self.drive_probe_backstop(ctx);
    }

    /// A probe run's backstop must fire even when the controller has gone quiet: when it is
    /// due, the run gets an empty batch of events (which fires it); until then a repaint is
    /// requested for the moment it falls due.
    fn drive_probe_backstop(&mut self, ctx: &egui::Context) {
        // A pane's run (Machine Control's quick probe) takes the empty batch through the
        // workspace.
        match self.workspace.wake_in() {
            Some(wait) if wait.is_zero() => {
                self.workspace.machine_events(&[], &self.state.translator);
            }
            Some(wait) => ctx.request_repaint_after(wait),
            None => {}
        }
        let Ok(mut cam) = self.cam_tools.try_borrow_mut() else {
            return;
        };
        match cam.probe_backstop_in() {
            Some(wait) if wait.is_zero() => cam.probe_events(&[], &self.state.translator),
            Some(wait) => ctx.request_repaint_after(wait),
            None => {}
        }
    }

    /// Hands the events no pane has seen yet to every pane, drawn or not.
    fn deliver_machine_events(&mut self) {
        let events = self.state.machine.take_pending();
        if !events.is_empty() {
            self.workspace
                .machine_events(&events, &self.state.translator);
            self.machine_windows_events(&events);
        }
    }

    /// Ends every press-and-hold input (a hold-to-jog) while the window does not have the
    /// keyboard focus or is minimized: its button release would never arrive.
    fn cancel_holds_without_focus(&mut self, ctx: &egui::Context) {
        let (focused, minimized) =
            ctx.input(|i| (i.focused, i.viewport().minimized.unwrap_or(false)));
        if !focused || minimized {
            self.workspace.cancel_holds();
        }
    }

    /// Hands a command to the pane that runs it: Machine Control the machine commands, the
    /// Visualizer the view commands.
    fn route_pane_command(&mut self, cmd: Command) {
        let mut requested = Vec::new();
        let key = self
            .jog_keys
            .iter()
            .position(|(command, _)| *command == cmd)
            .map(|i| self.jog_keys.remove(i).1);
        let mut ctx = PaneContext::new(&self.state.translator, &mut requested)
            .with_machine(&self.state.machine)
            .with_jog_key(key);
        if !self.workspace.route(cmd, &mut ctx) {
            tracing::warn!(?cmd, "no pane runs this command");
        }
        self.queue.extend(requested);
    }

    /// Hands `cmd` to the designer when it is the pane the user works in
    /// ([`Workspace::active_pane`]).
    ///
    /// # Returns
    /// Whether the designer ran it.
    fn route_to_active_designer(&mut self, cmd: Command) -> bool {
        if self.workspace.active_pane() != Some(PaneId::Designer) {
            return false;
        }
        let mut requested = Vec::new();
        let mut ctx = PaneContext::new(&self.state.translator, &mut requested)
            .with_machine(&self.state.machine);
        let ran = self.workspace.route_to(PaneId::Designer, cmd, &mut ctx);
        self.queue.extend(requested);
        ran
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

    /// Runs `cmd`. Machine commands go to the Machine Control pane. Commands whose features
    /// land in later milestones are logged, one arm per
    /// group, and recorded in [`GcodeKitApp::last_dispatched`] like every other command.
    ///
    /// # Arguments
    /// * `cmd` - The command to run.
    pub fn dispatch(&mut self, cmd: Command) {
        self.last_dispatched = Some(cmd);
        if self.dispatch_design(cmd) {
            return;
        }
        match cmd {
            Command::FileOpen => {
                if self.refuse_open_during_job() {
                    return;
                }
                if self.editor_prompt.is_some() {
                    // One editor prompt at a time: its answer must not reach this Open.
                    tracing::warn!("editor prompt already up; open refused");
                    let message = self.state.translator.tr(OPEN_BEHIND_HANDOFF).into_owned();
                    self.editor.borrow_mut().set_notice(message);
                } else if self.editor.borrow().buffer().is_dirty() {
                    self.editor_prompt = Some(EditorNext::Open);
                    self.dialogs.confirm(ConfirmRequest::UnsavedGcode);
                } else {
                    self.ask_open_gcode();
                }
            }
            Command::FileSave => self.save_editor(None),
            Command::FileSaveAs => {
                self.after_save = None;
                self.ask_save_gcode();
            }
            Command::OpenSettings => self.settings_window.open(&self.state.settings),
            Command::About => self.about_window.open(),
            Command::OpenFirmwareSettings => self.show_page(Page::DeviceConfig),
            Command::OpenDeviceInfo => self.show_page(Page::DeviceConfig),
            Command::OpenDeviceManager => self.show_page(Page::DeviceManager),
            Command::Quit => self.request_quit(QuitSource::MenuQuit),
            Command::Cut => self
                .viewport_commands
                .push(egui::ViewportCommand::RequestCut),
            Command::Copy => self
                .viewport_commands
                .push(egui::ViewportCommand::RequestCopy),
            Command::Paste => self
                .viewport_commands
                .push(egui::ViewportCommand::RequestPaste),
            // Run by `dispatch_design` above.
            Command::FileNew
            | Command::FileNew3d
            | Command::FileImport
            | Command::FileImportImage
            | Command::FileExportGcode
            | Command::FileExportSvg => {}
            Command::FileRun => tracing::debug!(?cmd, "document command not available yet"),
            Command::Undo | Command::Redo => {
                if !self.route_to_active_designer(cmd) {
                    tracing::debug!(?cmd, "no document to undo in");
                }
            }
            Command::StreamStart => {
                // Stream exactly what the editor shows, also within the typing pause.
                if self.sync_editor(true) {
                    self.route_pane_command(cmd);
                } else {
                    tracing::warn!("Start not sent: the editor could not be synced");
                }
            }
            Command::Connect
            | Command::Disconnect
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
            | Command::ViewIso => self.route_pane_command(cmd),
            Command::ZoomFit | Command::ZoomIn | Command::ZoomOut | Command::ResetView => {
                if !self.route_to_active_designer(cmd) {
                    self.route_pane_command(cmd);
                }
            }
            Command::OpenToolsManager => self.show_page(Page::CncTools),
            Command::OpenMaterialsManager => self.show_page(Page::Materials),
            Command::OpenHelp => self.help_window.open(help::HelpTopic::Index),
        }
    }

    /// Draws the Preferences, About, Device Info and help windows while they are open,
    /// runs the manager pages' per-frame work, acts on what Preferences decided and opens
    /// the topic a help button asked for.
    fn show_windows(&mut self, ctx: &egui::Context) {
        let outcome =
            self.settings_window
                .show(ctx, &mut self.state.settings, &self.state.translator);
        match outcome {
            SettingsOutcome::Applied { restart } => {
                self.workspace.configure(self.state.config());
                if restart {
                    self.request_restart();
                }
            }
            SettingsOutcome::Closed
            | SettingsOutcome::Open
            | SettingsOutcome::Cancelled
            | SettingsOutcome::Failed(_) => {}
        }
        self.about_window.show(ctx, &self.state.translator);
        self.show_machine_windows(ctx);
        self.show_library_windows();
        if let Some(topic) = help::take_request(ctx) {
            self.help_window.open(topic);
        }
        self.help_window.show(ctx, &self.state.translator);
    }

    /// Acts on a dialog's answer; every answer is also kept in
    /// [`GcodeKitApp::last_dialog_answer`].
    fn handle_answer(&mut self, answer: DialogAnswer) {
        if self.handle_design_answer(&answer)
            || self.handle_cam_preset_answer(&answer)
            || self.handle_firmware_settings_answer(&answer)
            || self.handle_library_answer(&answer)
        {
            self.last_dialog_answer = Some(answer);
            return;
        }
        match &answer {
            DialogAnswer::File {
                purpose: FilePurpose::OpenGcode,
                path,
            } => self.open_gcode(path),
            DialogAnswer::File {
                purpose: FilePurpose::SaveGcode,
                path,
            } => self.save_editor_as(path),
            DialogAnswer::Cancelled {
                purpose: FilePurpose::SaveGcode,
            } => self.abandon_after_save(),
            DialogAnswer::Unavailable {
                asked: Asked::File(FilePurpose::SaveGcode),
                reason,
            } => {
                tracing::warn!(%reason, "save dialog unavailable");
                self.abandon_after_save();
            }
            DialogAnswer::Unavailable {
                asked: Asked::Confirm(ConfirmRequest::UnsavedGcode),
                reason,
            } if self.editor_prompt == Some(EditorNext::Open) => {
                // Not opening without the answer: unsaved G-code would be lost.
                tracing::warn!(%reason, "unsaved-changes prompt unavailable; open abandoned");
                self.editor_prompt = None;
            }
            DialogAnswer::Confirm {
                request: ConfirmRequest::UnsavedGcode,
                choice,
            } if self.editor_prompt == Some(EditorNext::Open) => {
                self.editor_prompt = None;
                match choice.as_unsaved() {
                    UnsavedChangesChoice::Save => self.save_editor(Some(AfterSave::Open)),
                    UnsavedChangesChoice::Discard => self.ask_open_gcode(),
                    UnsavedChangesChoice::Cancel => {}
                }
            }
            DialogAnswer::QuitConfirm { request, choice } if self.quit.asking == Some(*request) => {
                self.quit_answer(*request, *choice);
            }
            DialogAnswer::Confirm {
                request: ConfirmRequest::Restart,
                choice,
            } => {
                if *choice == ConfirmChoice::Yes {
                    self.state.restart.store(true, Ordering::SeqCst);
                    self.request_quit(QuitSource::MenuQuit);
                }
            }
            DialogAnswer::Unavailable {
                asked: Asked::Quit(request),
                reason,
            } if self.quit.asking == Some(*request) => {
                // Quitting without the answer could lose unsaved work: stay open, and the
                // next quit asks again.
                tracing::warn!(?request, %reason, "quit prompt unavailable; quit cancelled");
                self.cancel_quit();
            }
            DialogAnswer::Confirm {
                request: request @ (ConfirmRequest::ZeroAllAxes(_) | ConfirmRequest::ClearEditor),
                choice,
            } => {
                let mut queue = Vec::new();
                let mut panes = PaneContext::new(&self.state.translator, &mut queue)
                    .with_machine(&self.state.machine);
                self.workspace.confirmed(*request, *choice, &mut panes);
                self.queue.extend(queue);
            }
            DialogAnswer::Unavailable { reason, .. } => {
                tracing::warn!(%reason, "dialog unavailable");
            }
            DialogAnswer::File { .. }
            | DialogAnswer::Cancelled { .. }
            | DialogAnswer::QuitConfirm { .. }
            | DialogAnswer::Confirm { .. } => {
                tracing::debug!(?answer, "dialog answer not consumed yet");
            }
        }
        self.last_dialog_answer = Some(answer);
    }

    /// Opens `path` in the editor and hands its text to the machine as the program to
    /// stream. A file that cannot be read leaves the editor as it was, with a notice.
    ///
    /// # Arguments
    /// * `path` - The G-code file.
    pub fn open_gcode(&mut self, path: &Path) {
        if self.refuse_open_during_job() {
            return;
        }
        {
            let mut editor = self.editor.borrow_mut();
            if let Err(error) = editor.load(path) {
                tracing::warn!(%error, "cannot open G-code");
                let tr = &self.state.translator;
                editor.set_notice(format!("{}: {error:#}", tr.tr("Could not open file")));
            }
        }
        self.sync_editor(true);
    }

    /// While a job streams, opening a file would swap the editor (and the program armed for
    /// the next Start) under the toolpath the job is drawing: says so in the editor and
    /// returns `true`.
    fn refuse_open_during_job(&mut self) -> bool {
        if !self.state.job_running() {
            return false;
        }
        let message = self
            .state
            .translator
            .tr("Stop the running job before opening a file.")
            .into_owned();
        self.editor.borrow_mut().set_notice(message);
        true
    }

    /// Opens the first G-code file named on the command line in the editor: the first path
    /// whose extension is one of the Open dialog's G-code types (any case). Every other path
    /// is logged and skipped, so a design, an image or a stray argument never becomes the
    /// program to stream.
    ///
    /// # Arguments
    /// * `files` - The paths from the command line.
    pub fn open_startup_files(&mut self, files: &[PathBuf]) {
        let mut opened = false;
        for path in files {
            if !opened && is_gcode_path(path) {
                opened = true;
                self.open_gcode(path);
            } else {
                tracing::warn!(path = %path.display(), "startup file skipped: not the first G-code file");
            }
        }
    }

    /// The help browser.
    pub fn help(&self) -> &HelpWindow {
        &self.help_window
    }

    /// The G-code editor.
    pub fn editor(&self) -> &Rc<RefCell<EditorPane>> {
        &self.editor
    }

    /// The designer pane.
    pub fn designer(&self) -> &Rc<RefCell<DesignerPane>> {
        &self.designer
    }

    /// The CAM tools pane.
    pub fn cam_tools(&self) -> &Rc<RefCell<CamToolsPane>> {
        &self.cam_tools
    }

    /// Copies the design's modified flag to [`AppState::dirty`] when it changed.
    fn sync_designer(&mut self) {
        let modified = match self.designer.try_borrow() {
            Ok(designer) => designer.is_modified(),
            Err(error) => {
                tracing::warn!(%error, "designer busy; dirty flag not synced");
                return;
            }
        };
        if modified != self.designer_dirty {
            self.designer_dirty = modified;
            self.state.dirty.designer = modified;
        }
    }

    /// Copies the editor's dirty flag to [`AppState::dirty`] when it changed, and hands the
    /// editor's text to the machine when it changed and typing paused (or `force`), unless a
    /// job is streaming. Returns `false` when the editor could not be reached (it is
    /// borrowed), so the caller must not act on a program that may be out of date.
    fn sync_editor(&mut self, force: bool) -> bool {
        let Ok(mut editor) = self.editor.try_borrow_mut() else {
            tracing::warn!("editor busy; program not synced");
            return false;
        };
        let dirty = editor.buffer().is_dirty();
        if dirty != self.editor_dirty {
            self.editor_dirty = dirty;
            self.state.dirty.editor = dirty;
        }
        if !editor.program_stale()
            || !(force || editor.edit_settled(Instant::now()))
            || self.state.job_running()
        {
            return true;
        }
        match self.state.machine.set_program(editor.buffer().text()) {
            Ok(0) => {
                self.state.machine.clear_program();
                editor.program_fed();
            }
            Ok(_) => editor.program_fed(),
            Err(reason) => {
                self.state.machine.clear_program();
                let tr = &self.state.translator;
                editor
                    .program_rejected(format!("{}: {reason}", tr.tr("Cannot stream this program")));
            }
        }
        true
    }

    /// Asks for a G-code file to open.
    fn ask_open_gcode(&mut self) {
        self.dialogs
            .open_file(FilePurpose::OpenGcode, &FilePurpose::OpenGcode.filters());
    }

    /// Asks where to save the editor's G-code.
    fn ask_save_gcode(&mut self) {
        self.dialogs
            .save_file(FilePurpose::SaveGcode, &FilePurpose::SaveGcode.filters());
    }

    /// Saves the editor's G-code to its file, or asks where when it has none; `then` runs
    /// once it is saved.
    pub(super) fn save_editor(&mut self, then: Option<AfterSave>) {
        let has_path = self.editor.borrow().buffer().path().is_some();
        if has_path {
            let result = self.editor.borrow_mut().buffer_mut().save();
            self.after_saving(result, then);
        } else {
            self.after_save = then;
            self.ask_save_gcode();
        }
    }

    /// Saves the editor's G-code to `path`, chosen in the save dialog.
    fn save_editor_as(&mut self, path: &Path) {
        let result = self.editor.borrow_mut().buffer_mut().save_as(path);
        let then = self.after_save.take();
        self.after_saving(result, then);
    }

    /// Reports a failed save, and runs `then` after a successful one; a failed save for a
    /// quit cancels the quit.
    fn after_saving(&mut self, result: anyhow::Result<()>, then: Option<AfterSave>) {
        let saved = match result {
            Ok(()) => {
                // The G-code is saved whatever the flag said (also when only the quit flow
                // thought it dirty).
                self.editor_dirty = false;
                self.state.dirty.editor = false;
                self.editor.borrow_mut().clear_notice();
                true
            }
            Err(error) => {
                tracing::warn!(%error, "cannot save G-code");
                let tr = &self.state.translator;
                self.editor
                    .borrow_mut()
                    .set_notice(format!("{}: {error:#}", tr.tr("Could not save file")));
                false
            }
        };
        match (saved, then) {
            (true, Some(AfterSave::Quit)) => self.continue_quit(),
            (true, Some(AfterSave::Open)) => self.ask_open_gcode(),
            (true, Some(AfterSave::ToEditor(what))) => self.send_to_editor(what),
            (false, Some(AfterSave::Quit)) => self.cancel_quit(),
            (false, Some(AfterSave::Open | AfterSave::ToEditor(_))) | (_, None) => {}
        }
    }

    /// The save dialog was closed without saving: a quit waiting for it is cancelled.
    fn abandon_after_save(&mut self) {
        if self.after_save.take() == Some(AfterSave::Quit) {
            self.cancel_quit();
        }
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

    /// The front end's state.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// The front end's state, for the panes that report unsaved work and running jobs, and
    /// for tests.
    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }

    /// The pages and their panes, for tests.
    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    /// The pages and their panes, for tests.
    pub fn workspace_mut(&mut self) -> &mut Workspace {
        &mut self.workspace
    }

    /// The page shown.
    pub fn page(&self) -> Page {
        self.workspace.page()
    }

    /// Shows `page`. A manager page that was not shown is prepared first (it tells why its
    /// file could not be read, CNC Tools reopens the last tool), as opening its window did.
    ///
    /// # Arguments
    /// * `page` - The page to show.
    pub fn show_page(&mut self, page: Page) {
        if self.workspace.page() == page {
            return;
        }
        self.workspace.set_page(page);
        let tr = &self.state.translator;
        match page {
            Page::DeviceManager => self.device_manager.open(tr),
            Page::CncTools => self.tools_manager.open(tr),
            Page::Materials => self.materials_manager.open(tr),
            Page::Designer
            | Page::Visualizer
            | Page::MachineControl
            | Page::CamTools
            | Page::DeviceConfig => {}
        }
    }

    /// Draws the manager page shown into `ui`.
    fn show_manager_page(&mut self, ui: &mut egui::Ui) {
        let tr = &self.state.translator;
        match self.workspace.page() {
            Page::DeviceManager => {
                self.device_manager
                    .show(ui, &self.state.machine, self.state.settings.config(), tr);
            }
            Page::DeviceConfig => self.show_firmware_settings(ui),
            Page::CncTools => self.tools_manager.show(ui, tr),
            Page::Materials => self.materials_manager.show(ui, tr),
            Page::Designer | Page::Visualizer | Page::MachineControl | Page::CamTools => {}
        }
    }

    /// The keymap, for the user's shortcut overrides and for tests.
    pub fn keymap_mut(&mut self) -> &mut Keymap {
        &mut self.keymap
    }

    /// What runs when the app exits: a hold-to-jog is cancelled (0x85), the machine
    /// connection is closed (writing 0x85 itself while a jog is outstanding) and the serial
    /// port released, and then, when the user asked to restart, `relaunch` starts the app
    /// again.
    ///
    /// # Arguments
    /// * `relaunch` - Starts the new process (`relaunch_command().spawn()` in the app).
    #[doc(hidden)]
    pub fn on_exit_with(&mut self, relaunch: impl FnOnce() -> std::io::Result<()>) {
        self.workspace.cancel_holds();
        self.state.machine.disconnect();
        if self.state.restart.swap(false, Ordering::SeqCst)
            && let Err(error) = relaunch()
        {
            tracing::error!(%error, "could not restart GCodeKit5");
        }
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

/// Whether `path` has one of the G-code extensions the Open dialog offers
/// ([`FileFilter::GCODE`], any case). The filter itself is used rather than
/// `FilePurpose::OpenGcode.filters()`, which is empty on macOS and adds "All Files" elsewhere.
fn is_gcode_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|extension| {
            FileFilter::GCODE
                .extensions
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
}

/// The Machine Control pane over `state`'s controller, with the configured connection and jog
/// settings.
fn machine_control_pane(state: &AppState) -> MachineControlPane {
    let config = state.config();
    let m = &config.machine;
    MachineControlPane::new(Arc::clone(state.machine.controller()))
        .with_connection(config.connection.clone())
        .with_jog(
            m.jog_increment as f32,
            m.jog_feed_rate as f32,
            [m.x_limit as f32, m.y_limit as f32, m.z_limit as f32],
        )
}

impl eframe::App for GcodeKitApp {
    /// Runs the quit and dialog logic, then lets craft-quit take a close request the app did
    /// not cancel over (macOS quits through `-[NSApplication terminate:]`).
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.logic(ctx);
        craft_quit::on_close_request(ctx);
    }

    /// Draws the whole window.
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.ui(ui);
        self.dialogs.end_frame(Some(&*frame));
    }

    /// Closes the machine connection, then relaunches the app when the user asked to restart
    /// (the relaunched process must find the serial port free). Runs here, not after
    /// `eframe::run_native` returns, because on macOS the process ends inside
    /// `-[NSApplication terminate:]` once eframe's exit handling has run.
    ///
    /// Edge case: eframe calls this on every shutdown it runs itself, so if the event loop
    /// ends with an error after a restart was confirmed (the flag is set from the confirmation
    /// until the window closes), the app is still relaunched. A panic or a kill skips this
    /// and nothing is relaunched.
    fn on_exit(&mut self) {
        self.on_exit_with(|| {
            crate::relaunch_command()
                .and_then(|mut command| command.spawn())
                .map(drop)
        });
    }

    /// Feeds the native menu's key equivalents to egui before it reads the input.
    #[cfg(target_os = "macos")]
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.menus.raw_input(raw_input);
    }
}
