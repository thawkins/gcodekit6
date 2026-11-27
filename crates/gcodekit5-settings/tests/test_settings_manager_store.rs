//! Tests for `SettingsManager` on craft-settings: settings saved through the
//! manager come back through a fresh manager and through the store, and the
//! legacy `config.json` is never written again.

use craft_settings::SettingsStore;
use gcodekit5_settings::migrate::ConfigDirs;
use gcodekit5_settings::{Config, SettingsManager};
use std::path::PathBuf;

/// A fresh `target/temp/settings_manager_store/<test>-<pid>` directory.
fn scratch(test: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/settings_manager_store")
        .join(format!("{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// The legacy GTK folder with the fixture's `config.json` in it.
fn legacy_with_config(root: &std::path::Path) -> PathBuf {
    let legacy = root.join("legacy");
    std::fs::create_dir_all(&legacy).expect("create legacy dir");
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gtk_config/config.json"),
        legacy.join("config.json"),
    )
    .expect("copy fixture config");
    legacy
}

#[test]
fn test_manager_save_reloads_through_manager_and_store() {
    let root = scratch("test_manager_save_reloads_through_manager_and_store");
    let dirs = ConfigDirs::new(legacy_with_config(&root), root.join("new"));

    let (mut config, error) = SettingsManager::load(&dirs);
    assert!(error.is_none(), "{error:?}");
    assert_eq!(config.ui.language, "fr", "legacy settings imported on load");
    config.machine.jog_feed_rate = 2345.0;
    SettingsManager::save(&dirs, &config).expect("save");

    let (reloaded, error) = SettingsManager::load(&dirs);
    assert!(error.is_none(), "{error:?}");
    assert_eq!(reloaded.machine.jog_feed_rate, 2345.0);
    let (store, _) = SettingsStore::<Config>::open_at(&dirs.config);
    assert_eq!(store.get().machine.jog_feed_rate, 2345.0);
    assert_eq!(store.get(), &config);
}

#[test]
fn test_manager_apply_leaves_legacy_config_json_untouched() {
    let root = scratch("test_manager_apply_leaves_legacy_config_json_untouched");
    let legacy = legacy_with_config(&root);
    let before = std::fs::read(legacy.join("config.json")).expect("read legacy");
    let dirs = ConfigDirs::new(legacy.clone(), root.join("new"));

    let (mut manager, error) = SettingsManager::open(dirs.clone());
    assert!(error.is_none(), "{error:?}");
    let mut config = manager.config().clone();
    config.ui.language = "it".to_string();
    manager.apply(config).expect("save");

    assert_eq!(
        std::fs::read(legacy.join("config.json")).expect("read legacy"),
        before,
        "config.json is byte-identical after a save"
    );
    let (store, _) = SettingsStore::<Config>::open_at(&dirs.config);
    assert_eq!(store.get().ui.language, "it");
}

#[test]
fn test_manager_apply_saves_and_keeps_config() {
    let root = scratch("test_manager_apply_saves_and_keeps_config");
    let dirs = ConfigDirs::new(root.join("legacy"), root.join("new"));

    let (mut manager, error) = SettingsManager::open(dirs.clone());
    assert!(error.is_none(), "{error:?}");
    let mut config = manager.config().clone();
    config.ui.language = "de".to_string();
    manager.apply(config.clone()).expect("apply");
    assert_eq!(manager.config(), &config, "applied config kept in memory");

    let (reloaded, error) = SettingsManager::open(dirs);
    assert!(error.is_none(), "{error:?}");
    assert_eq!(reloaded.config().ui.language, "de");
}

#[test]
fn test_detached_manager_refuses_to_apply() {
    let mut manager = SettingsManager::detached(Config::default());
    let mut config = Config::default();
    config.ui.language = "de".to_string();
    assert!(manager.apply(config).is_err(), "nowhere to save");
    assert_eq!(manager.config(), &Config::default(), "unchanged on failure");
}
