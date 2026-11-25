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

#[path = "machine_support.rs"]
mod machine_support;
#[path = "test_cam_bitmap_engraving.rs"]
mod test_cam_bitmap_engraving;
#[path = "test_cam_drill_press.rs"]
mod test_cam_drill_press;
#[path = "test_cam_generate_during_job.rs"]
mod test_cam_generate_during_job;
#[path = "test_cam_gerber.rs"]
mod test_cam_gerber;
#[path = "test_cam_input_bounds.rs"]
mod test_cam_input_bounds;
#[path = "test_cam_jigsaw.rs"]
mod test_cam_jigsaw;
#[path = "test_cam_presets.rs"]
mod test_cam_presets;
#[path = "test_cam_probe_backstop.rs"]
mod test_cam_probe_backstop;
#[path = "test_cam_probe_review.rs"]
mod test_cam_probe_review;
#[path = "test_cam_probe_routines.rs"]
mod test_cam_probe_routines;
#[path = "test_cam_probe_run.rs"]
mod test_cam_probe_run;
#[path = "test_cam_speeds_feeds.rs"]
mod test_cam_speeds_feeds;
#[path = "test_cam_spoilboard_grid.rs"]
mod test_cam_spoilboard_grid;
#[path = "test_cam_spoilboard_surfacing.rs"]
mod test_cam_spoilboard_surfacing;
#[path = "test_cam_tabbed_box.rs"]
mod test_cam_tabbed_box;
#[path = "test_cam_tools.rs"]
mod test_cam_tools;
#[path = "test_commands.rs"]
mod test_commands;
#[path = "test_console_pane.rs"]
mod test_console_pane;
#[path = "test_designer_app.rs"]
mod test_designer_app;
#[path = "test_designer_array.rs"]
mod test_designer_array;
#[path = "test_designer_canvas.rs"]
mod test_designer_canvas;
#[path = "test_designer_editing.rs"]
mod test_designer_editing;
#[path = "test_designer_editor_handoff.rs"]
mod test_designer_editor_handoff;
#[path = "test_designer_files.rs"]
mod test_designer_files;
#[path = "test_designer_files_app.rs"]
mod test_designer_files_app;
#[path = "test_designer_gallery.rs"]
mod test_designer_gallery;
#[path = "test_designer_import.rs"]
mod test_designer_import;
#[path = "test_designer_layers.rs"]
mod test_designer_layers;
#[path = "test_designer_properties.rs"]
mod test_designer_properties;
#[path = "test_designer_properties_cam.rs"]
mod test_designer_properties_cam;
#[path = "test_designer_properties_commit.rs"]
mod test_designer_properties_commit;
#[path = "test_designer_properties_shapes.rs"]
mod test_designer_properties_shapes;
#[path = "test_designer_snapshots.rs"]
mod test_designer_snapshots;
#[path = "test_designer_text_tool.rs"]
mod test_designer_text_tool;
#[path = "test_designer_toolbox.rs"]
mod test_designer_toolbox;
#[path = "test_designer_toolpath.rs"]
mod test_designer_toolpath;
#[path = "test_dialogs.rs"]
mod test_dialogs;
#[path = "test_editor.rs"]
mod test_editor;
#[path = "test_frame.rs"]
mod test_frame;
#[path = "test_hidpi.rs"]
mod test_hidpi;
#[path = "test_machine_jog.rs"]
mod test_machine_jog;
#[path = "test_machine_pane.rs"]
mod test_machine_pane;
#[path = "test_machine_probe_report.rs"]
mod test_machine_probe_report;
#[path = "test_quit.rs"]
mod test_quit;
#[path = "test_settings_window.rs"]
mod test_settings_window;
#[path = "test_smoke.rs"]
mod test_smoke;
#[path = "test_status_bar.rs"]
mod test_status_bar;
#[path = "test_translations.rs"]
mod test_translations;
#[path = "test_visualizer.rs"]
mod test_visualizer;
#[path = "test_workspace.rs"]
mod test_workspace;
