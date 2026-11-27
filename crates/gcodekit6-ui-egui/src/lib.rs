//! egui + craft-libs front end for GCodeKit6.
//!
//! Dependencies: `egui` and `eframe` for the UI and native window, `craft-ui` for widgets and
//! page tabs, `craft-window` for the window frame policy, `craft-keymap` and `craft-menu` for
//! key bindings and menus, `craft-platform` for the desktop's
//! light/dark preference, `gcodekit6-app` and `gcodekit6-settings` for state and settings.

#![deny(unsafe_code)]

/// Marks a string literal for translation extraction without translating it, for msgids
/// returned from a `match` and translated later through a variable. `scripts/update-po.sh`
/// passes `--keyword=n_!` so these msgids are extracted.
macro_rules! n_ {
    ($s:literal) => {
        $s
    };
}

pub mod app;
pub mod commands;
pub mod dialogs;
mod files;
pub mod frame;
pub mod icons;
pub mod machine;
pub mod menu;
pub mod panes;
pub mod resources;
pub mod shortcuts;
pub mod status_bar;
pub mod theme;
pub mod windows;
pub mod workspace;

pub use app::{AppState, DirtyState, GcodeKitApp, QuitSource};

/// Options controlling how the front end starts.
#[derive(Debug, Clone, Default)]
pub struct LaunchOptions {
    /// G-code files to open at startup; the first is opened in the editor.
    pub files: Vec<std::path::PathBuf>,
}

/// Runs the egui front end until the window is closed. A restart the user asked for is
/// started from `eframe::App::on_exit` (see [`GcodeKitApp`]).
///
/// # Arguments
/// * `options` - Startup options such as files to open.
///
/// # Returns
/// `Ok(())` on a clean exit, or an error if the native window cannot start.
pub fn run(options: LaunchOptions) -> anyhow::Result<()> {
    let LaunchOptions { files } = options;
    let (settings, settings_problem) = AppState::load_settings();
    gcodekit6_app::i18n::set_language(&settings.config().ui.language);
    let decision = craft_window::policy::decide_for(
        craft_window::Prefix::new("GCODEKIT6"),
        craft_window::policy::Backend::from_env(),
        frame::title_bar_pref(&settings.config().ui),
    );
    let state = AppState::new(settings, decision).with_settings_problem(settings_problem);
    // Keep eframe's default `depth_buffer: 0` and `multisampling: 0`: the visualizer's wgpu
    // pipelines (`panes::visualizer::install`) draw inside egui's pass and are built for no
    // depth attachment and one sample per pixel.
    let native = eframe::NativeOptions {
        viewport: craft_window::policy::apply(main_viewport(), &state.decision.frame),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "GCodeKit",
        native,
        Box::new(move |cc| {
            let mut app = GcodeKitApp::new(cc, state);
            app.open_startup_files(&files);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe failed: {e}"))?;
    Ok(())
}

/// The main window's viewport before the frame policy is applied: it opens maximized, with
/// 1280x800 as the size it restores to when the user un-maximizes it.
///
/// # Returns
/// The viewport builder for the main window.
pub fn main_viewport() -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_title("GCodeKit")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([640.0, 400.0])
        .with_maximized(true)
}

/// The command that starts this executable again with the same arguments, for a restart the
/// user asked for after changing a setting that needs one.
///
/// # Errors
/// Returns an error when the path of the running executable is unavailable.
pub fn relaunch_command() -> std::io::Result<std::process::Command> {
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command.args(std::env::args_os().skip(1));
    Ok(command)
}
