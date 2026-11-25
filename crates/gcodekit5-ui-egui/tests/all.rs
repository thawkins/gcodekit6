//! Single integration-test binary for `gcodekit5-ui-egui`.
//!
//! Each `test_*.rs` file is included as a child module. Run one file with
//! `cargo nextest run -p gcodekit5-ui-egui -E 'test(<module>::)'`.

/// Binary-wide lock for tests that touch process-global state.
pub fn global_env_mutex() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

/// Takes [`global_env_mutex`], recovering it if a previous holder panicked.
pub fn global_env_lock() -> std::sync::MutexGuard<'static, ()> {
    global_env_mutex()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[path = "test_commands.rs"]
mod test_commands;
#[path = "test_dialogs.rs"]
mod test_dialogs;
#[path = "test_frame.rs"]
mod test_frame;
#[path = "test_quit.rs"]
mod test_quit;
#[path = "test_settings_window.rs"]
mod test_settings_window;
#[path = "test_smoke.rs"]
mod test_smoke;
#[path = "test_translations.rs"]
mod test_translations;
#[path = "test_workspace.rs"]
mod test_workspace;
