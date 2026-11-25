//! The Preferences window edits a copy of the settings: Apply saves it through
//! `SettingsManager` (to a `target/temp` folder here), Cancel drops it, and a language or
//! title-bar change asks for a restart. Replaces the GTK crate's
//! `test_i18n_language_restart.rs` for this front end.

use std::path::PathBuf;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_app::i18n::{Translator, needs_restart};
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::MeasurementSystem;
use gcodekit5_settings::migrate::ConfigDirs;
use gcodekit5_ui_egui::windows::settings::{SettingsOutcome, SettingsWindow};

/// Fresh settings folders under `target/temp/egui_settings_window/<test>-<pid>`.
fn scratch(test: &str) -> ConfigDirs {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/egui_settings_window")
        .join(format!("{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create scratch dir");
    ConfigDirs::new(root.join("legacy"), root.join("config"))
}

struct State {
    window: SettingsWindow,
    settings: SettingsManager,
    tr: Translator,
    /// Every outcome other than `Open` and `Closed`, in order.
    outcomes: Vec<SettingsOutcome>,
}

/// A harness showing the Preferences window over `dirs`, already open.
fn harness(dirs: ConfigDirs, title_bar_row: bool) -> Harness<'static, State> {
    let (settings, problem) = SettingsManager::open(dirs);
    assert!(problem.is_none(), "{problem:?}");
    let mut window = SettingsWindow::new(title_bar_row, &settings.config().ui);
    window.open(&settings);
    let state = State {
        window,
        settings,
        tr: Translator::new("en"),
        outcomes: Vec::new(),
    };
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, state: &mut State| {
                let outcome = state.window.show(ui.ctx(), &mut state.settings, &state.tr);
                if !matches!(outcome, SettingsOutcome::Open | SettingsOutcome::Closed) {
                    state.outcomes.push(outcome);
                }
            },
            state,
        );
    harness.run();
    harness
}

/// Opens the combo box labelled `field` and picks `option`.
fn choose(harness: &mut Harness<'_, State>, field: &str, option: &str) {
    harness.get_by_label(field).click();
    harness.run();
    harness.get_by_label(option).click();
    harness.run();
}

fn click(harness: &mut Harness<'_, State>, label: &str) {
    harness.get_by_label(label).click();
    harness.run();
}

#[test]
fn test_units_toggle_persists() {
    let dirs = scratch("test_units_toggle_persists");
    let mut harness = harness(dirs.clone(), true);
    choose(&mut harness, "Measurement System", "Imperial");
    click(&mut harness, "Apply");
    assert_eq!(
        harness.state().outcomes,
        [SettingsOutcome::Applied { restart: false }]
    );
    assert!(!harness.state().window.is_open());
    assert_eq!(
        harness.state().settings.config().ui.measurement_system,
        MeasurementSystem::Imperial
    );

    let (reloaded, problem) = SettingsManager::open(dirs);
    assert!(problem.is_none(), "{problem:?}");
    assert_eq!(
        reloaded.config().ui.measurement_system,
        MeasurementSystem::Imperial
    );
}

#[test]
fn test_cancel_discards() {
    let dirs = scratch("test_cancel_discards");
    let mut harness = harness(dirs.clone(), true);
    choose(&mut harness, "Measurement System", "Imperial");
    click(&mut harness, "Cancel");
    assert_eq!(harness.state().outcomes, [SettingsOutcome::Cancelled]);
    assert!(!harness.state().window.is_open());
    assert_eq!(
        harness.state().settings.config().ui.measurement_system,
        MeasurementSystem::Metric
    );

    let (reloaded, _) = SettingsManager::open(dirs);
    assert_eq!(
        reloaded.config().ui.measurement_system,
        MeasurementSystem::Metric
    );
}

#[test]
fn test_language_change_requests_restart() {
    // When a restart is needed: only when the effective language changes.
    assert!(needs_restart("en", "de"));
    assert!(needs_restart("de", "system"));
    assert!(needs_restart("system", "fr"));
    assert!(!needs_restart("de", "de"));
    assert!(!needs_restart("system", "system"));
    // An empty preference is the system language.
    assert!(!needs_restart("system", ""));
    assert!(!needs_restart("", "system"));

    let dirs = scratch("test_language_change_requests_restart");
    let mut harness = harness(dirs.clone(), true);
    click(&mut harness, "User Interface");
    choose(&mut harness, "Language", "Deutsch");
    click(&mut harness, "Apply");
    assert_eq!(
        harness.state().outcomes,
        [SettingsOutcome::Applied { restart: true }]
    );
    let (reloaded, _) = SettingsManager::open(dirs);
    assert_eq!(reloaded.config().ui.language, "de");
}

#[test]
fn test_title_bar_row_follows_decision() {
    // Hidden where the setting changes nothing.
    let mut harness_hidden = harness(scratch("test_title_bar_row_hidden"), false);
    click(&mut harness_hidden, "User Interface");
    assert!(
        harness_hidden
            .query_by_label("Use system title bar")
            .is_none(),
        "row shown although the setting does nothing here"
    );
    drop(harness_hidden);

    // Shown elsewhere; changing it asks for a restart.
    let dirs = scratch("test_title_bar_row_follows_decision");
    let mut harness = harness(dirs.clone(), true);
    click(&mut harness, "User Interface");
    click(&mut harness, "Use system title bar");
    click(&mut harness, "Apply");
    assert_eq!(
        harness.state().outcomes,
        [SettingsOutcome::Applied { restart: true }]
    );
    let (reloaded, _) = SettingsManager::open(dirs);
    assert!(reloaded.config().ui.use_system_title_bar);
}

#[test]
fn test_apply_keeps_changes_made_while_open() {
    let dirs = scratch("test_apply_keeps_changes_made_while_open");
    let mut harness = harness(dirs.clone(), true);

    // Settings the window does not edit change elsewhere while it is open.
    let recent = PathBuf::from("/work/recent.nc");
    let mut config = harness.state().settings.config().clone();
    config.recent_files = vec![recent.clone()];
    config.last_opened_path = Some("/work".to_string());
    harness
        .state_mut()
        .settings
        .apply(config)
        .expect("save unedited fields");

    choose(&mut harness, "Measurement System", "Imperial");
    click(&mut harness, "Apply");
    assert_eq!(
        harness.state().outcomes,
        [SettingsOutcome::Applied { restart: false }]
    );

    let (reloaded, problem) = SettingsManager::open(dirs);
    assert!(problem.is_none(), "{problem:?}");
    let config = reloaded.config();
    assert_eq!(config.recent_files, [recent]);
    assert_eq!(config.last_opened_path.as_deref(), Some("/work"));
    assert_eq!(config.ui.measurement_system, MeasurementSystem::Imperial);
}

#[test]
fn test_clearing_default_directory_saves_none() {
    let dirs = scratch("test_clearing_default_directory_saves_none");
    let mut harness = harness(dirs.clone(), true);
    assert_ne!(
        harness
            .state()
            .settings
            .config()
            .file_processing
            .output_directory,
        PathBuf::new(),
        "starts with a default directory"
    );
    harness.get_by_label("Default Directory").focus();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.key_press(egui::Key::Backspace);
    harness.run();
    click(&mut harness, "Apply");
    assert_eq!(
        harness.state().outcomes,
        [SettingsOutcome::Applied { restart: false }]
    );

    let (reloaded, _) = SettingsManager::open(dirs);
    assert_eq!(
        reloaded.config().file_processing.output_directory,
        PathBuf::new()
    );
}
