#![deny(unsafe_code)]
// On Windows, hide the console window for GUI applications
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

#[cfg(all(feature = "gtk", feature = "egui"))]
compile_error!("features `gtk` and `egui` are exclusive: build with one front end");
#[cfg(not(any(feature = "gtk", feature = "egui")))]
compile_error!("no front end: enable feature `gtk` (default) or `egui`");

use gcodekit5_design::init_logging;

#[cfg_attr(test, mutants::skip)]
fn main() -> anyhow::Result<()> {
    // Initialize logging
    init_logging()?;

    // Launch GTK Application
    #[cfg(feature = "gtk")]
    gcodekit5_ui::gtk_app::main();

    // Launch the egui + craft-libs front end, opening any files named on the command line.
    #[cfg(feature = "egui")]
    gcodekit5_ui_egui::run(gcodekit5_ui_egui::LaunchOptions {
        files: std::env::args_os().skip(1).map(Into::into).collect(),
    })?;

    Ok(())
}
