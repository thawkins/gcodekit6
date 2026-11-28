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

    // Carry the GCodeKit5 folders over before anything (the Windows log file included)
    // creates the GCodeKit6 ones; the outcome is logged once logging is up.
    let mut renamed = gcodekit6_settings::rename::migrate_renamed_folders(
        &gcodekit6_settings::rename::platform_bases(),
    );
    // Inside Flatpak the GCodeKit5 sandbox's folders live under its own app ID.
    renamed.extend(gcodekit6_settings::rename::migrate_folder_pairs(
        &gcodekit6_settings::rename::flatpak_folders(
            &|name| std::env::var_os(name),
            std::path::Path::new(gcodekit6_settings::rename::FLATPAK_INFO),
        ),
    ));

    // Initialize logging
    init_logging()?;
    renamed.iter().for_each(|migration| migration.log());

    // Launch the egui + craft-libs front end, opening any files named on the command line.
    gcodekit6_ui_egui::run(gcodekit6_ui_egui::LaunchOptions {
        files: files.into_iter().map(Into::into).collect(),
    })?;

    Ok(())
}
