//! Application state and root UI of the egui front end: the window frame, the light/dark
//! theme and the docking workspace.
//!
//! Who draws the title bar is decided by `craft_window::policy` before the window opens and
//! carried in [`AppState::decision`]; [`crate::frame`] draws what that decision leaves to the
//! app. The light/dark mode follows `craft_platform::appearance` unless the user forced one in
//! [`UiSettings::theme`](gcodekit5_settings::config::UiSettings::theme).
//!
//! Dependencies: `egui`, `eframe`, `craft-window`, `craft-platform`, `gcodekit5-app`
//! (translations), `gcodekit5-settings` (configuration).

use craft_platform::appearance::SystemThemeFn;
use craft_window::policy::Decision;
use gcodekit5_app::i18n::Translator;
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::Config;

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
    /// The `movable` flag last set on the macOS window (`title_bar::sync_movable`).
    #[cfg(target_os = "macos")]
    movable: Option<bool>,
}

impl std::fmt::Debug for GcodeKitApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcodeKitApp")
            .field("state", &self.state)
            .field("workspace", &self.workspace)
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
        Self {
            system_theme: craft_platform::appearance::egui_service(),
            ..Self::from_state(state)
        }
    }

    /// Creates the application without a desktop appearance watcher (headless use).
    ///
    /// # Arguments
    /// * `state` - The front end's state.
    pub fn from_state(state: AppState) -> Self {
        Self {
            state,
            workspace: Workspace::default_layout(),
            system_theme: None,
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
        crate::frame::show(ui, &self.state.translator, &self.state.decision.frame);
        let mut ctx = PaneContext::new(&self.state.translator);
        self.workspace.show(ui, &mut ctx);
        #[cfg(target_os = "macos")]
        if matches!(
            self.state.decision.frame,
            craft_window::policy::Frame::Inset(_)
        ) {
            craft_window::title_bar::sync_movable(ui.ctx(), &mut self.movable);
        }
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
}
