#![deny(unsafe_code)]
// On Windows, hide the console window for GUI applications
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use gcodekit5_design::init_logging;

#[cfg_attr(test, mutants::skip)]
fn main() -> anyhow::Result<()> {
    // Initialize logging
    init_logging()?;

    // Launch the egui + craft-libs front end, opening any files named on the command line.
    gcodekit5_ui_egui::run(gcodekit5_ui_egui::LaunchOptions {
        files: std::env::args_os().skip(1).map(Into::into).collect(),
    })?;

    Ok(())
}
