#![deny(unsafe_code)]
// On Windows, hide the console window for GUI applications
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use gcodekit6_design::{CliAction, HELP_TEXT, init_logging, parse_args, version_line};

#[cfg_attr(test, mutants::skip)]
fn main() -> anyhow::Result<()> {
    // --version and --help print and exit before the GUI (or logging) starts.
    let files = match parse_args(std::env::args_os().skip(1)) {
        CliAction::PrintVersion => {
            println!("{}", version_line());
            return Ok(());
        }
        CliAction::PrintHelp => {
            print!("{HELP_TEXT}");
            return Ok(());
        }
        CliAction::Run(files) => files,
    };

    // Initialize logging
    init_logging()?;

    // Launch the egui + craft-libs front end, opening any files named on the command line.
    gcodekit6_ui_egui::run(gcodekit6_ui_egui::LaunchOptions {
        files: files.into_iter().map(Into::into).collect(),
    })?;

    Ok(())
}
