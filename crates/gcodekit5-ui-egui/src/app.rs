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
//! Dependencies: `egui`, `eframe`, `craft-window`, `craft-platform`, `craft-keymap`,
//! `craft-menu`, `craft-dialogs`, `craft-quit`, `gcodekit5-app` (translations, the
//! unsaved-changes answer), `gcodekit5-settings` (configuration).

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
use crate::panes::console::ConsolePane;
use crate::panes::editor::{EditorPane, SharedEditor};
use crate::panes::machine_control::MachineControlPane;
use crate::panes::visualizer::VisualizerPane;
use crate::theme;
use crate::windows::{AboutWindow, SettingsOutcome, SettingsWindow};
use crate::workspace::{PaneContext, PaneId, Workspace};

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
    /// flag here whenever it changes; the designer sets its own once it exists (Task 12).
    pub dirty: DirtyState,
    /// The machine connection: the shared controller, this frame's events and the state
    /// derived from them. Replace it (with [`AppState::with_machine`]) before the app is built
    /// from the state, since the Machine Control pane shares its controller.
    pub machine: Machine,
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
            dirty: DirtyState::default(),
            machine: Machine::new(MachineController::new()),
        }
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
    /// Quitting was decided: the app sent `Close` and lets the close request through.
    confirmed: bool,
}

/// What to do once the editor's G-code is saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AfterSave {
    /// Go on quitting (the quit prompt's Save).
    Quit,
    /// Ask for a file to open (Open with unsaved changes, Save chosen).
    Open,
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
    /// The quit being decided, if any.
    quit: QuitFlow,
    /// The G-code editor, also drawn by the workspace.
    editor: Rc<RefCell<EditorPane>>,
    /// The editor's dirty flag as last copied to [`AppState::dirty`].
    editor_dirty: bool,
    /// What waits for the editor's save dialog to be answered.
    after_save: Option<AfterSave>,
    /// An Open asked first about the editor's unsaved changes.
    open_after_confirm: bool,
    /// The egui frame whose machine events were last polled.
    machine_polled: Option<u64>,
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
        let mut workspace = Workspace::default_layout();
        workspace.set_pane(Box::new(machine_control_pane(&state)));
        workspace.set_pane(Box::new(ConsolePane::new(Arc::clone(
            state.machine.controller(),
        ))));
        let editor = Rc::new(RefCell::new(EditorPane::new()));
        workspace.set_pane(Box::new(SharedEditor(Rc::clone(&editor))));
        workspace.set_pane(Box::new(VisualizerPane::new()));
        workspace.configure(state.config());
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
            last_dispatched: None,
            quit: QuitFlow::default(),
            editor,
            editor_dirty: false,
            after_save: None,
            open_after_confirm: false,
            machine_polled: None,
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
        // While the editor's save dialog for this quit is up, the quit waits for it.
        let saving = self.after_save == Some(AfterSave::Quit);
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
                    UnsavedChangesChoice::Save => {
                        // The designer cannot save in this front end yet (Task 12), so nothing
                        // is saved and the app stays open.
                        tracing::warn!(?request, "cannot save before quitting yet; quit cancelled");
                        self.cancel_quit();
                    }
                    UnsavedChangesChoice::Cancel => self.cancel_quit(),
                }
            }
            // The quit flow never asks this; forget the quit rather than act on it.
            (ConfirmRequest::Restart, _) => self.cancel_quit(),
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
        self.dispatch_keys(&ctx);
        let gate = Gate {
            text_editing: ctx.text_edit_focused(),
            ..Gate::default()
        };
        let mut queue = std::mem::take(&mut self.queue);
        queue.extend(self.menus.take_native(&ctx, gate));
        self.show_frame_and_menu(ui, gate, &mut queue);
        egui::Panel::bottom("gcodekit5-status-bar").show(ui, |ui| {
            if crate::status_bar::show(
                ui,
                &self.state.translator,
                self.state.machine.status(),
                self.state.config().ui.measurement_system,
            ) {
                queue.push(Command::SoftReset);
            }
        });
        let mut panes =
            PaneContext::new(&self.state.translator, &mut queue).with_machine(&self.state.machine);
        self.workspace.show(ui, &mut panes);
        for command in queue.drain(..) {
            self.dispatch(command);
        }
        self.sync_editor(false);
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
        let hits = ctx.input_mut(|input| self.dispatcher.dispatch(input, &self.keymap, &active));
        self.queue.extend(
            hits.iter()
                .filter_map(|hit| Command::from_ref(&hit.command)),
        );
    }

    /// The keymap scope of the pane that has the keyboard: the pane the user last pressed the
    /// pointer in, but only while no widget has keyboard focus and no popup (an open dropdown)
    /// is up, so a pane's keys never take a key from a focused widget.
    fn keyboard_panel(&self, ctx: &egui::Context) -> Option<&'static str> {
        let widget_has_keys =
            ctx.memory(|m| m.focused().is_some()) || egui::Popup::is_any_open(ctx);
        if widget_has_keys {
            return None;
        }
        self.workspace.focused_pane().and_then(PaneId::keymap_scope)
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
    }

    /// Hands the events no pane has seen yet to every pane, drawn or not.
    fn deliver_machine_events(&mut self) {
        let events = self.state.machine.take_pending();
        if !events.is_empty() {
            self.workspace
                .machine_events(&events, &self.state.translator);
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
        let mut ctx = PaneContext::new(&self.state.translator, &mut requested)
            .with_machine(&self.state.machine);
        if !self.workspace.route(cmd, &mut ctx) {
            tracing::warn!(?cmd, "no pane runs this command");
        }
        self.queue.extend(requested);
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
        match cmd {
            Command::FileOpen => {
                if self.editor.borrow().buffer().is_dirty() {
                    self.open_after_confirm = true;
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
            Command::FileNew
            | Command::FileNew3d
            | Command::FileImport
            | Command::FileImportImage
            | Command::FileExportGcode
            | Command::FileExportSvg
            | Command::FileRun
            | Command::Undo
            | Command::Redo => tracing::debug!(?cmd, "document command not available yet"),
            Command::StreamStart => {
                // Stream exactly what the editor shows, also within the typing pause.
                self.sync_editor(true);
                self.route_pane_command(cmd);
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
            | Command::ZoomFit
            | Command::ZoomIn
            | Command::ZoomOut
            | Command::ResetView
            | Command::ViewTop
            | Command::ViewFront
            | Command::ViewIso => self.route_pane_command(cmd),
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
    }

    /// Acts on a dialog's answer; every answer is also kept in
    /// [`GcodeKitApp::last_dialog_answer`].
    fn handle_answer(&mut self, answer: DialogAnswer) {
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
            } if self.open_after_confirm => {
                // Not opening without the answer: unsaved G-code would be lost.
                tracing::warn!(%reason, "unsaved-changes prompt unavailable; open abandoned");
                self.open_after_confirm = false;
            }
            DialogAnswer::Confirm {
                request: ConfirmRequest::UnsavedGcode,
                choice,
            } if self.open_after_confirm => {
                self.open_after_confirm = false;
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

    /// The G-code editor.
    pub fn editor(&self) -> &Rc<RefCell<EditorPane>> {
        &self.editor
    }

    /// Copies the editor's dirty flag to [`AppState::dirty`] when it changed, and hands the
    /// editor's text to the machine when it changed and typing paused (or `force`), unless a
    /// job is streaming.
    fn sync_editor(&mut self, force: bool) {
        let Ok(mut editor) = self.editor.try_borrow_mut() else {
            return;
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
            return;
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
    fn save_editor(&mut self, then: Option<AfterSave>) {
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
            (false, Some(AfterSave::Quit)) => self.cancel_quit(),
            (false, Some(AfterSave::Open)) | (_, None) => {}
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

    /// The docking workspace, for layout changes and tests.
    pub fn workspace_mut(&mut self) -> &mut Workspace {
        &mut self.workspace
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
