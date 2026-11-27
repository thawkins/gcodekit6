//! Regression tests for settings loading: tolerant loading of partial config
//! files, and a damaged `config.toml` kept aside rather than overwritten.

use gcodekit5_settings::migrate::ConfigDirs;
use gcodekit5_settings::{Config, SettingsManager};
use std::path::PathBuf;

#[test]
fn test_config_load_partial_json_uses_defaults() {
    // An older config missing whole sections and individual fields must still load.
    let json = r#"{ "connection": { "port": "/dev/ttyACM0" }, "machine": { "x_limit": 400.0 } }"#;
    let config: Config = serde_json::from_str(json).expect("partial config should load");
    assert_eq!(config.connection.port, "/dev/ttyACM0");
    assert_eq!(config.connection.baud_rate, 115200);
    assert_eq!(config.machine.x_limit, 400.0);
    assert_eq!(config.machine.y_limit, 200.0);
    config.validate().expect("defaults should validate");
}

#[test]
fn test_config_merge_keeps_ui_settings_with_system_theme() {
    let mut base = Config::default();
    let mut other = Config::default();
    other.ui.theme = gcodekit5_settings::Theme::System;
    other.ui.window_width = 1920;
    other.ui.font_size = 16;
    other.ui.language = "es".to_string();

    base.merge(&other);

    assert_eq!(base.ui.window_width, 1920);
    assert_eq!(base.ui.font_size, 16);
    assert_eq!(base.ui.language, "es");
}

#[test]
fn test_config_merge_ignores_unpopulated_ui_section() {
    let mut base = Config::default();
    base.ui.window_width = 1024;
    let mut other = Config::default();
    other.ui.window_width = 0;
    other.ui.window_height = 0;

    base.merge(&other);

    assert_eq!(base.ui.window_width, 1024);
}

/// Fresh settings folders under `target/temp/config_regression/<test>-<pid>`.
fn scratch(test: &str) -> ConfigDirs {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/config_regression")
        .join(format!("{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create scratch dir");
    ConfigDirs::new(root.join("legacy"), root.join("config"))
}

/// The `config.broken-*` copies craft-settings moved aside in `dir`.
fn quarantined(dir: &std::path::Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .expect("read config dir")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("config.broken-"))
        })
        .collect()
}

/// Audit A-01 on the live load path: a `config.toml` that cannot be read is moved aside
/// (craft-settings' quarantine) and the user is told where, instead of being replaced by
/// defaults on the next save. The quarantined copy survives later saves byte for byte.
#[test]
fn test_damaged_config_toml_is_quarantined_and_reported() {
    let dirs = scratch("test_damaged_config_toml_is_quarantined_and_reported");
    std::fs::create_dir_all(&dirs.config).expect("create config dir");
    let damaged = "[ui\nlanguage = ";
    std::fs::write(dirs.config.join("config.toml"), damaged).expect("write damaged file");

    let (mut manager, problem) = SettingsManager::open(dirs.clone());
    let problem = problem.expect("the damaged file is reported");
    assert!(
        problem.contains("config.toml was damaged and moved to"),
        "{problem}"
    );
    assert_eq!(
        manager.config(),
        &Config::default(),
        "defaults in its place"
    );

    let moved = quarantined(&dirs.config);
    assert_eq!(moved.len(), 1, "{moved:?}");
    assert!(
        problem.contains(&moved[0].display().to_string()),
        "{problem}"
    );
    assert_eq!(
        std::fs::read_to_string(&moved[0]).expect("read quarantined file"),
        damaged
    );

    let mut config = manager.config().clone();
    config.ui.language = "de".to_string();
    manager.apply(config).expect("save over the defaults");
    let (reloaded, problem) = SettingsManager::open(dirs.clone());
    assert!(problem.is_none(), "{problem:?}");
    assert_eq!(reloaded.config().ui.language, "de");
    assert_eq!(quarantined(&dirs.config), moved, "no second quarantine");
    assert_eq!(
        std::fs::read_to_string(&moved[0]).expect("read quarantined file"),
        damaged,
        "a save leaves the quarantined copy alone"
    );
}
