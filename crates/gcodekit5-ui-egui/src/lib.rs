//! egui + craft-libs front end for GCodeKit5.
//!
//! Dependencies: `egui` and `eframe` for the UI and native window, `craft-ui` for widgets.

#![deny(unsafe_code)]

pub mod app;

pub use app::GcodeKitApp;

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
    eframe::run_native(
        "GCodeKit",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(GcodeKitApp::default()))),
    )
    .map_err(|e| anyhow::anyhow!("eframe failed: {e}"))
}
