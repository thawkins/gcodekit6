//! Tests for `gcodekit6_settings::rename`: the one-shot copy of the GCodeKit5 folders to
//! their GCodeKit6 names, and the `GCODEKIT5_CONFIG_DIR` fallback.
//!
//! Every test works in a directory of its own under `target/temp`, never in the user's
//! folders, and reads the environment through a closure instead of the process.

use gcodekit6_settings::rename::{OLD_APP_ID, config_dir_var, migrate_renamed_folders};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// A fresh `target/temp/settings_rename/<test>-<pid>` directory.
fn scratch(test: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/settings_rename")
        .join(format!("{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Writes `text` to `path`, creating its parent folders.
fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
    std::fs::write(path, text).expect("write file");
}

#[test]
fn test_rename_copies_old_folder_recursively() {
    let base = scratch("test_rename_copies_old_folder_recursively");
    let old = base.join(OLD_APP_ID);
    write(&old.join("config.toml"), "[ui]\n");
    write(&old.join("command_history.json"), "[\"G0 X1\"]");
    write(&old.join("logs/nested/gcodekit5.log"), "line\n");

    let report = migrate_renamed_folders(std::slice::from_ref(&base));

    assert_eq!(report.len(), 1);
    assert!(matches!(report[0].result, Ok(true)));
    assert_eq!(report[0].to, base.join("gcodekit6"));
    let new = base.join("gcodekit6");
    for name in [
        "config.toml",
        "command_history.json",
        "logs/nested/gcodekit5.log",
    ] {
        assert_eq!(
            std::fs::read(new.join(name)).expect("read new file"),
            std::fs::read(old.join(name)).expect("read old file"),
            "{name} is copied byte for byte"
        );
    }
    assert!(
        old.join("config.toml").is_file(),
        "old folder is left in place"
    );
    assert!(!base.join("gcodekit6.migrating").exists());
}

#[test]
fn test_rename_runs_once_and_never_overwrites_new_folder() {
    let base = scratch("test_rename_runs_once_and_never_overwrites_new_folder");
    write(&base.join(OLD_APP_ID).join("config.toml"), "old");
    write(&base.join("gcodekit6").join("config.toml"), "new");

    let report = migrate_renamed_folders(std::slice::from_ref(&base));

    assert!(matches!(report[0].result, Ok(false)));
    assert_eq!(
        std::fs::read_to_string(base.join("gcodekit6/config.toml")).expect("read new"),
        "new"
    );
}

#[test]
fn test_rename_without_old_folder_does_nothing() {
    let base = scratch("test_rename_without_old_folder_does_nothing");

    let report = migrate_renamed_folders(std::slice::from_ref(&base));

    assert!(matches!(report[0].result, Ok(false)));
    assert!(!base.join("gcodekit6").exists());
}

#[test]
fn test_rename_replaces_an_interrupted_copy() {
    let base = scratch("test_rename_replaces_an_interrupted_copy");
    write(&base.join(OLD_APP_ID).join("devices.json"), "[]");
    write(
        &base.join("gcodekit6.migrating").join("partial.tmp"),
        "half",
    );

    let report = migrate_renamed_folders(std::slice::from_ref(&base));

    assert!(matches!(report[0].result, Ok(true)));
    assert!(base.join("gcodekit6/devices.json").is_file());
    assert!(!base.join("gcodekit6/partial.tmp").exists());
    assert!(!base.join("gcodekit6.migrating").exists());
}

#[test]
fn test_rename_failure_is_reported_not_raised() {
    let base = scratch("test_rename_failure_is_reported_not_raised");
    // A file where the staging folder goes cannot be removed as a folder, so the copy fails.
    write(&base.join(OLD_APP_ID).join("config.toml"), "x");
    write(&base.join("gcodekit6.migrating"), "not a folder");

    let report = migrate_renamed_folders(std::slice::from_ref(&base));

    assert!(report[0].result.is_err(), "{:?}", report[0].result);
    assert!(!base.join("gcodekit6").exists());
}

/// An environment with exactly these variables.
fn env_of(vars: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
    move |name| {
        vars.iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| OsString::from(value))
    }
}

#[test]
fn test_config_dir_var_prefers_new_variable() {
    let env = env_of(&[
        ("GCODEKIT6_CONFIG_DIR", "/new"),
        ("GCODEKIT5_CONFIG_DIR", "/old"),
    ]);
    assert_eq!(
        config_dir_var("GCODEKIT6_CONFIG_DIR", &env),
        Some(OsString::from("/new"))
    );
}

#[test]
fn test_config_dir_var_falls_back_to_old_variable() {
    let env = env_of(&[("GCODEKIT5_CONFIG_DIR", "")]);
    assert_eq!(
        config_dir_var("GCODEKIT6_CONFIG_DIR", &env),
        Some(OsString::new())
    );
    assert_eq!(config_dir_var("OTHER_VAR", &env), None);
}

#[test]
fn test_config_dir_var_unset_is_none() {
    let env = env_of(&[]);
    assert_eq!(config_dir_var("GCODEKIT6_CONFIG_DIR", &env), None);
}
