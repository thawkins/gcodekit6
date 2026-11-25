//! egui + craft-libs front end for GCodeKit5.
//!
//! Dependencies: `egui` and `eframe` for the UI and native window, `craft-ui` for widgets and
//! docking, `craft-window` for the window frame policy, `craft-platform` for the desktop's
//! light/dark preference, `gcodekit5-app` and `gcodekit5-settings` for state and settings.

#![deny(unsafe_code)]

pub mod app;
pub mod theme;
pub mod workspace;

pub use app::{AppState, GcodeKitApp};

/// Options controlling how the front end starts.
#[derive(Debug, Clone, Default)]
pub struct LaunchOptions {
    /// G-code files to open at startup.
    pub files: Vec<std::path::PathBuf>,
}

/// Runs the egui front end until the window is closed.
///
/// # Arguments
/// * `options` - Startup options such as files to open.
///
/// # Returns
/// `Ok(())` on a clean exit, or an error if the native window cannot start.
pub fn run(options: LaunchOptions) -> anyhow::Result<()> {
    // `options.files` is consumed once the app opens files (Tasks 4 and 9).
    let LaunchOptions { files: _ } = options;
    let mut state = AppState::load();
    gcodekit5_app::i18n::set_language(&state.config.ui.language);
    let decision = craft_window::policy::decide_for(
        craft_window::Prefix::new("GCODEKIT5"),
        craft_window::policy::Backend::from_env(),
        craft_window::policy::TitleBarPref::Integrated,
    );
    state.frame = decision.frame;
    let viewport = egui::ViewportBuilder::default()
        .with_title("GCodeKit")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([640.0, 400.0]);
    let native = eframe::NativeOptions {
        viewport: craft_window::policy::apply(viewport, &state.frame),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "GCodeKit",
        native,
        Box::new(move |cc| Ok(Box::new(GcodeKitApp::new(cc, state)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe failed: {e}"))
}
