//! Single integration-test binary for `gcodekit5-app`.
//!
//! Each `test_*.rs` (or legacy) file is included as a child module, so the crate
//! links one test binary instead of one per file. Run one file with
//! `cargo nextest run -p gcodekit5-app -E 'test(<module>::)'`.

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

#[path = "test_command_history.rs"]
mod test_command_history;
#[path = "test_device_console_manager.rs"]
mod test_device_console_manager;
#[path = "test_device_profiles_model.rs"]
mod test_device_profiles_model;
#[path = "test_files_atomic.rs"]
mod test_files_atomic;
#[path = "test_firmware_settings_model.rs"]
mod test_firmware_settings_model;
#[path = "test_grbl_stream.rs"]
mod test_grbl_stream;
#[path = "test_i18n.rs"]
mod test_i18n;
#[path = "test_job_preferences.rs"]
mod test_job_preferences;
#[path = "test_machine_busy_guard.rs"]
mod test_machine_busy_guard;
#[path = "test_machine_controller.rs"]
mod test_machine_controller;
#[path = "test_machine_lines_answered.rs"]
mod test_machine_lines_answered;
#[path = "test_machine_probe_stream.rs"]
mod test_machine_probe_stream;
#[path = "test_machine_repetier.rs"]
mod test_machine_repetier;
#[path = "test_machine_sent_events.rs"]
mod test_machine_sent_events;
#[path = "test_materials_manager_backend.rs"]
mod test_materials_manager_backend;
#[path = "test_override_target.rs"]
mod test_override_target;
#[path = "test_report_units.rs"]
mod test_report_units;
#[path = "test_tool_material_library.rs"]
mod test_tool_material_library;
#[path = "test_tools_manager_backend.rs"]
mod test_tools_manager_backend;
#[path = "test_translation_catalogs.rs"]
mod test_translation_catalogs;
#[path = "test_unsaved_changes_choice.rs"]
mod test_unsaved_changes_choice;
#[path = "test_work_zero.rs"]
mod test_work_zero;
