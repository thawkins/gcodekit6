//! The Visualizer sidebar's and the Designer inspector's Hide are kept between runs: in the
//! settings' `panel_visibility` (GTK's `visualizer_sidebar` key; `designer_properties` for
//! the inspector), saved when they change and read at start.

use std::path::Path;

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use gcodekit6_settings::SettingsManager;
use gcodekit6_settings::config::Config;
use gcodekit6_settings::migrate::ConfigDirs;
use gcodekit6_ui_egui::panes::visualizer::VisualizerPane;
use gcodekit6_ui_egui::workspace::{Page, Pane};
use gcodekit6_ui_egui::{AppState, GcodeKitApp};

use crate::machine_support::app_harness;

/// A fresh settings folder under `target/temp` for `name`.
fn dirs(name: &str) -> ConfigDirs {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/egui_panel_visibility")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    ConfigDirs::new(dir.join("legacy"), dir.join("config"))
}

/// The app over the settings in `dirs`, a couple of frames in.
fn app(dirs: &ConfigDirs) -> egui_kittest::Harness<'static, GcodeKitApp> {
    let (settings, _) = SettingsManager::open(dirs.clone());
    let state = AppState::new(settings, crate::test_smoke::native_decision());
    let mut h = app_harness(GcodeKitApp::from_state(state));
    h.run_steps(2);
    h
}

/// The saved `panel_visibility` value of `key`.
fn saved(dirs: &ConfigDirs, key: &str) -> Option<bool> {
    let (settings, _) = SettingsManager::open(dirs.clone());
    settings.config().ui.panel_visibility.get(key).copied()
}

#[test]
fn test_visualizer_pane_follows_the_setting() {
    let mut config = Config::default();
    config
        .ui
        .panel_visibility
        .insert("visualizer_sidebar".to_string(), false);
    let mut pane = VisualizerPane::new();
    pane.configure(&config);
    assert!(pane.sidebar_hidden());
    assert_eq!(pane.panel_visibility(), Some(("visualizer_sidebar", false)));
    pane.configure(&Config::default());
    assert!(!pane.sidebar_hidden(), "shown by default, as in GTK");
}

#[test]
fn test_visualizer_sidebar_hide_is_kept_between_runs() {
    let dirs = dirs("visualizer");
    let mut h = app(&dirs);
    h.state_mut().show_page(Page::Visualizer);
    h.run_steps(2);
    h.get_by_label("Hide Sidebar").click();
    h.run_steps(2);
    assert_eq!(saved(&dirs, "visualizer_sidebar"), Some(false));
    drop(h);

    let mut h = app(&dirs);
    h.state_mut().show_page(Page::Visualizer);
    h.run_steps(2);
    assert!(h.query_by_label("Hide Sidebar").is_none(), "still hidden");
    h.get_by_role_and_label(Role::Button, "Show Sidebar")
        .click();
    h.run_steps(2);
    assert_eq!(saved(&dirs, "visualizer_sidebar"), Some(true));
}

#[test]
fn test_designer_inspector_hide_is_kept_between_runs() {
    let dirs = dirs("designer");
    let mut h = app(&dirs);
    h.state_mut().show_page(Page::Designer);
    h.run_steps(2);
    h.get_by_label("Hide Properties").click();
    h.run_steps(2);
    assert_eq!(saved(&dirs, "designer_properties"), Some(false));
    drop(h);

    let mut h = app(&dirs);
    h.state_mut().show_page(Page::Designer);
    h.run_steps(2);
    assert!(
        h.query_by_label("Hide Properties").is_none(),
        "still hidden"
    );
    assert!(h.query_by_label("Unhide Properties").is_some());
}
