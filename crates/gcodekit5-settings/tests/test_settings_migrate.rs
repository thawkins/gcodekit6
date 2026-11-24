//! Tests for `gcodekit5_settings::migrate`: the one-shot import of the GTK
//! front end's JSON files into the craft-settings layout (D3).
//!
//! Every test works in a directory of its own under `target/temp`, never in
//! the user's configuration folder.

use craft_settings::{SettingsStore, Source};
use gcodekit5_settings::Config;
use gcodekit5_settings::migrate::{
    ConfigDirs, LEGACY_DATA_FILES, MARKER_FILE, MigrationReport, import_legacy, open_store,
};
use std::path::{Path, PathBuf};

/// The checked-in fixture: files as the GTK build writes them.
fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gtk_config")
}

/// A fresh `target/temp/settings_migrate/<test>-<pid>` directory.
fn scratch(test: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/settings_migrate")
        .join(format!("{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Copies every file of the fixture into `dest`.
fn copy_fixture(dest: &Path) {
    std::fs::create_dir_all(dest).expect("create legacy dir");
    for entry in std::fs::read_dir(fixture_dir()).expect("read fixture dir") {
        let path = entry.expect("fixture entry").path();
        let name = path.file_name().expect("fixture file name");
        std::fs::copy(&path, dest.join(name)).expect("copy fixture file");
    }
}

fn json(path: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(path).expect("read json file");
    serde_json::from_str(&text).expect("parse json file")
}

#[test]
fn test_import_gtk_config() {
    let root = scratch("test_import_gtk_config");
    let dirs = ConfigDirs::new(root.join("legacy"), root.join("new"));
    copy_fixture(&dirs.legacy);
    let legacy_config =
        Config::load_from_file(&dirs.legacy.join("config.json")).expect("fixture config loads");
    let legacy_bytes = std::fs::read(dirs.legacy.join("config.json")).expect("read legacy");

    let report = import_legacy(&dirs).expect("import succeeds");
    assert_eq!(
        report,
        MigrationReport::Imported {
            items: 1 + LEGACY_DATA_FILES.len()
        }
    );

    // Settings: config.toml through craft-settings, every value kept.
    let (store, load) = SettingsStore::<Config>::open_at(&dirs.config);
    assert_eq!(load.source, Source::File);
    assert!(!load.has_problems(), "{load:?}");
    assert_eq!(store.get(), &legacy_config);
    assert_eq!(store.get().ui.language, "fr");
    assert_eq!(
        store.get().recent_files,
        vec![
            PathBuf::from("/home/user/gcode/tabbedbox.gcode"),
            PathBuf::from("/home/user/gcode/spoilboard.nc"),
        ]
    );
    assert_eq!(store.get().machine.x_limit, 420.0);

    // Machine profiles, tool library, materials, command history: unchanged.
    for name in LEGACY_DATA_FILES {
        assert_eq!(
            json(&dirs.config.join(name)),
            json(&fixture_dir().join(name)),
            "{name} round-trips"
        );
    }

    // The legacy files stay in place, untouched.
    assert_eq!(
        std::fs::read(dirs.legacy.join("config.json")).expect("legacy kept"),
        legacy_bytes
    );

    // Second run: nothing is imported again, and the new config is untouched.
    let toml_before = std::fs::read(dirs.config.join("config.toml")).expect("config.toml");
    assert_eq!(
        import_legacy(&dirs).expect("second run"),
        MigrationReport::AlreadyMigrated
    );
    let (store, load, migration) = open_store(&dirs);
    assert_eq!(migration.expect("open"), MigrationReport::AlreadyMigrated);
    assert_eq!(load.source, Source::File);
    assert_eq!(store.get(), &legacy_config);
    assert_eq!(
        std::fs::read(dirs.config.join("config.toml")).expect("config.toml"),
        toml_before
    );
}

#[test]
fn test_import_without_legacy_files() {
    let root = scratch("test_import_without_legacy_files");
    let dirs = ConfigDirs::new(root.join("legacy"), root.join("new"));

    assert_eq!(
        import_legacy(&dirs).expect("import succeeds"),
        MigrationReport::NothingToImport
    );
    assert!(!dirs.config.join("config.toml").exists());
}

#[test]
fn test_import_rejects_unreadable_legacy_config() {
    let root = scratch("test_import_rejects_unreadable_legacy_config");
    let dirs = ConfigDirs::new(root.join("legacy"), root.join("new"));
    std::fs::create_dir_all(&dirs.legacy).expect("create legacy dir");
    std::fs::write(dirs.legacy.join("config.json"), "{ not json").expect("write");

    assert!(import_legacy(&dirs).is_err());
    // Not marked as done, so a fixed file is imported on the next run.
    std::fs::copy(
        fixture_dir().join("config.json"),
        dirs.legacy.join("config.json"),
    )
    .expect("copy fixture config");
    assert_eq!(
        import_legacy(&dirs).expect("import succeeds"),
        MigrationReport::Imported { items: 1 }
    );
}

/// Production layout: `APP_ID` puts the new files in the legacy folder on
/// every platform, so the import runs with both folders the same.
#[test]
fn test_import_in_place_when_folders_coincide() {
    let root = scratch("test_import_in_place_when_folders_coincide");
    let dir = root.join("gcodekit5");
    let dirs = ConfigDirs::new(dir.clone(), dir.clone());
    copy_fixture(&dir);
    let legacy_config =
        Config::load_from_file(&dir.join("config.json")).expect("fixture config loads");

    assert_eq!(
        import_legacy(&dirs).expect("import succeeds"),
        MigrationReport::Imported { items: 5 }
    );
    for name in LEGACY_DATA_FILES.iter().chain(["config.json"].iter()) {
        assert_eq!(
            std::fs::read(dir.join(name)).expect("read data file"),
            std::fs::read(fixture_dir().join(name)).expect("read fixture file"),
            "{name} is byte-identical"
        );
    }
    let (store, load) = SettingsStore::<Config>::open_at(&dir);
    assert_eq!(load.source, Source::File);
    assert_eq!(store.get(), &legacy_config);
    assert!(dir.join(MARKER_FILE).is_file());
    assert_eq!(
        import_legacy(&dirs).expect("second run"),
        MigrationReport::AlreadyMigrated
    );
}

/// A damaged `config.json` beside a valid `config.toml` is not an error:
/// the settings already moved, so the legacy file is not even parsed.
#[test]
fn test_broken_legacy_config_ignored_when_toml_is_valid() {
    let root = scratch("test_broken_legacy_config_ignored_when_toml_is_valid");
    let dirs = ConfigDirs::new(root.join("legacy"), root.join("new"));
    std::fs::create_dir_all(&dirs.legacy).expect("create legacy dir");
    std::fs::write(dirs.legacy.join("config.json"), "{ not json").expect("write");
    let mut saved = Config::default();
    saved.ui.language = "de".to_string();
    let (mut store, _) = SettingsStore::<Config>::open_at(&dirs.config);
    store.set(saved.clone());
    store.flush().expect("write config.toml");

    assert_eq!(
        import_legacy(&dirs).expect("import succeeds"),
        MigrationReport::NothingToImport
    );
    assert_eq!(
        import_legacy(&dirs).expect("second run"),
        MigrationReport::AlreadyMigrated
    );
    let (store, _) = SettingsStore::<Config>::open_at(&dirs.config);
    assert_eq!(store.get(), &saved);
}

/// A damaged `config.toml` is moved aside by craft-settings; the store then
/// starts from defaults, so the legacy `config.json` is imported.
#[test]
fn test_broken_toml_falls_back_to_legacy_config() {
    let root = scratch("test_broken_toml_falls_back_to_legacy_config");
    let dirs = ConfigDirs::new(root.join("legacy"), root.join("new"));
    std::fs::create_dir_all(&dirs.legacy).expect("create legacy dir");
    std::fs::copy(
        fixture_dir().join("config.json"),
        dirs.legacy.join("config.json"),
    )
    .expect("copy fixture config");
    std::fs::create_dir_all(&dirs.config).expect("create config dir");
    std::fs::write(dirs.config.join("config.toml"), "[ui\nbroken").expect("write");
    let legacy_config =
        Config::load_from_file(&dirs.legacy.join("config.json")).expect("fixture config loads");

    assert_eq!(
        import_legacy(&dirs).expect("import succeeds"),
        MigrationReport::Imported { items: 1 }
    );
    let (store, load) = SettingsStore::<Config>::open_at(&dirs.config);
    assert_eq!(load.source, Source::File);
    assert_eq!(store.get(), &legacy_config);
    let moved_aside = std::fs::read_dir(&dirs.config)
        .expect("read config dir")
        .filter_map(Result::ok)
        .any(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("config.broken-")
        });
    assert!(moved_aside, "the damaged config.toml is kept aside");
}
