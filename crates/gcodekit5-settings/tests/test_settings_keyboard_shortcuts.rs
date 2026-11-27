//! The user's keyboard shortcuts (`ui.keyboard_shortcuts`, written by the egui front end) are
//! saved with the settings, and a later save of other settings does not lose
//! them. A settings file from before the field loads with no changed shortcuts.

use craft_settings::SettingsStore;
use gcodekit5_settings::config::ShortcutOverride;
use gcodekit5_settings::migrate::ConfigDirs;
use gcodekit5_settings::{Config, SettingsManager};
use std::path::PathBuf;

/// Fresh settings folders under `target/temp/settings_keyboard_shortcuts/<test>-<pid>`.
fn scratch(test: &str) -> ConfigDirs {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/settings_keyboard_shortcuts")
        .join(format!("{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create scratch dir");
    ConfigDirs::new(root.join("legacy"), root.join("config"))
}

fn entry(command: &str, keys: &[&str]) -> ShortcutOverride {
    ShortcutOverride {
        command: command.to_string(),
        keys: keys.iter().map(|k| (*k).to_string()).collect(),
    }
}

#[test]
fn test_shortcuts_saved_and_reloaded() {
    let dirs = scratch("test_shortcuts_saved_and_reloaded");
    let (mut manager, problem) = SettingsManager::open(dirs.clone());
    assert!(problem.is_none(), "{problem:?}");
    assert!(manager.config().ui.keyboard_shortcuts.is_empty());
    let mut config = manager.config().clone();
    config.ui.keyboard_shortcuts = vec![
        entry("file.open", &["Cmd+Shift+O"]),
        entry("edit.redo", &["Cmd+Y", "Cmd+Shift+Z", "F4"]),
        entry("help.about", &[]),
    ];
    manager.apply(config.clone()).expect("apply");

    let (reloaded, problem) = SettingsManager::open(dirs.clone());
    assert!(problem.is_none(), "{problem:?}");
    assert_eq!(
        reloaded.config().ui.keyboard_shortcuts,
        config.ui.keyboard_shortcuts
    );
    let text = std::fs::read_to_string(dirs.config.join("config.toml")).expect("read toml");
    assert!(text.contains("keyboard_shortcuts"), "{text}");
}

/// The list replaces the saved one whole: a command reset to its defaults (its entry gone)
/// stays reset after saving, and Reset All leaves none.
#[test]
fn test_removed_entries_stay_removed() {
    let dirs = scratch("test_removed_entries_stay_removed");
    let (mut manager, _) = SettingsManager::open(dirs.clone());
    let mut config = manager.config().clone();
    config.ui.keyboard_shortcuts = vec![
        entry("file.open", &["Cmd+Shift+O"]),
        entry("help.about", &["F9"]),
    ];
    manager.apply(config.clone()).expect("apply");
    config.ui.keyboard_shortcuts = vec![entry("help.about", &["F9"])];
    manager.apply(config.clone()).expect("apply");
    let (reloaded, _) = SettingsManager::open(dirs.clone());
    assert_eq!(
        reloaded.config().ui.keyboard_shortcuts,
        [entry("help.about", &["F9"])]
    );
    config.ui.keyboard_shortcuts.clear();
    manager.apply(config).expect("apply");
    let (reloaded, _) = SettingsManager::open(dirs);
    assert!(reloaded.config().ui.keyboard_shortcuts.is_empty());
}

/// A later load, change of another field and save through `SettingsManager` keeps the
/// shortcuts saved before unchanged.
#[test]
fn test_other_settings_save_keeps_the_shortcuts() {
    let dirs = scratch("test_other_settings_save_keeps_the_shortcuts");
    let mut config = Config::default();
    config.ui.keyboard_shortcuts = vec![entry("jog.x_pos", &["J"])];
    SettingsManager::save(&dirs, &config).expect("save");

    let (mut manager, problem) = SettingsManager::open(dirs.clone());
    assert!(problem.is_none(), "{problem:?}");
    assert_eq!(
        manager.config().ui.keyboard_shortcuts,
        config.ui.keyboard_shortcuts
    );
    let mut changed = manager.config().clone();
    changed.ui.language = "fr".to_string();
    manager.apply(changed).expect("save");

    let (store, _) = SettingsStore::<Config>::open_at(&dirs.config);
    assert_eq!(store.get().ui.language, "fr");
    assert_eq!(
        store.get().ui.keyboard_shortcuts,
        [entry("jog.x_pos", &["J"])]
    );
}

/// A settings file written before the field existed (and the GTK `config.json` import) loads
/// with no changed shortcuts.
#[test]
fn test_file_without_shortcuts_loads_empty() {
    let toml = "[ui]\nlanguage = \"de\"\n";
    let config: Config = toml::from_str(toml).expect("parse");
    assert_eq!(config.ui.language, "de");
    assert!(config.ui.keyboard_shortcuts.is_empty());

    let legacy: Config = serde_json::from_str(
        &std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gtk_config/config.json"),
        )
        .expect("read fixture"),
    )
    .expect("parse GTK config.json");
    assert!(legacy.ui.keyboard_shortcuts.is_empty());
}
