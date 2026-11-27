//! # GCodeKit5
//!
//! The application binary's library: logging set-up. The application itself
//! is in the workspace crates (`gcodekit5-ui-egui` runs the egui front end).
#![deny(unsafe_code)]

/// Initialize logging with the default configuration
///
/// Sets up structured logging with:
/// - Console output with pretty formatting (Windows release: a compact, ANSI-free file in
///   the per-user data directory)
/// - RUST_LOG environment variable support
/// - UTF timestamps
#[cfg_attr(test, mutants::skip)]
pub fn init_logging() -> anyhow::Result<()> {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::fmt;
    use tracing_subscriber::prelude::*;

    // RUST_LOG wins when set; otherwise log at INFO.
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // In Windows GUI mode (release builds) there is no console, so log to a file in the
    // per-user data directory (the install directory is often unwritable).
    #[cfg(all(target_os = "windows", not(debug_assertions)))]
    {
        let opened = dirs::data_local_dir().and_then(|base| open_log_file(&base).ok());
        match opened {
            Some(file) => {
                // No ANSI escapes and one record per line: the sink is a plain text file.
                let fmt_layer = fmt::layer()
                    .with_writer(file)
                    .with_ansi(false)
                    .with_target(true)
                    .with_level(true)
                    .with_thread_ids(true)
                    .with_thread_names(true)
                    .with_line_number(true)
                    .compact();

                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(fmt_layer)
                    .try_init()?;
            }
            None => {
                // No writable log location: keep going without a sink rather than crash.
                tracing_subscriber::registry().with(env_filter).try_init()?;
            }
        }
    }

    // In debug mode or non-Windows, log to stdout as before
    #[cfg(not(all(target_os = "windows", not(debug_assertions))))]
    {
        let fmt_layer = fmt::layer()
            .with_writer(std::io::stdout)
            .with_target(true)
            .with_level(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_line_number(true)
            .pretty();

        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .try_init()?;
    }

    Ok(())
}

/// Opens (creating it and any missing directories) the append-only log file
/// `<base>/gcodekit5/gcodekit5.log`.
///
/// # Arguments
/// * `base` - the per-user data directory, for example `%LOCALAPPDATA%` on Windows.
///
/// # Errors
/// Returns the I/O error if the directory or the file cannot be created.
pub fn open_log_file(base: &std::path::Path) -> std::io::Result<std::fs::File> {
    let dir = base.join("gcodekit5");
    std::fs::create_dir_all(&dir)?;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("gcodekit5.log"))
}
