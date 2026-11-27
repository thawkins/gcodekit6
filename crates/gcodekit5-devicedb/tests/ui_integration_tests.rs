//! Regression tests for the device profile manager (load, backup, atomic save).

use gcodekit5_devicedb::{DeviceManager, DeviceProfile};

/// Files in `dir` whose name starts with `devices.json.bak-`.
fn backups(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .expect("read_dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with("devices.json.bak-"))
                .unwrap_or(false)
        })
        .collect()
}

#[test]
fn test_devicedb_load_skips_bad_entry_and_backs_up_before_save() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("devices.json");
    let good = DeviceProfile {
        id: "good".to_string(),
        name: "Good".to_string(),
        ..DeviceProfile::default()
    };
    let original = serde_json::json!({
        "profiles": [
            good,
            { "id": "bad", "num_axes": "not a number" }
        ],
        "active_id": "good"
    })
    .to_string();
    std::fs::write(&path, &original).expect("write");

    let manager = DeviceManager::new(path.clone());
    manager
        .load()
        .expect("load must succeed despite one bad entry");
    assert!(manager.get_profile("good").is_some());
    assert!(manager.get_profile("bad").is_none());
    assert_eq!(
        manager.get_active_profile().map(|p| p.id),
        Some("good".to_string())
    );
    assert!(manager.has_unreadable_entries());
    assert!(backups(dir.path()).is_empty(), "load itself must not write");

    manager.save().expect("save");
    let baks = backups(dir.path());
    assert_eq!(baks.len(), 1, "one backup before the lossy save");
    assert_eq!(std::fs::read_to_string(&baks[0]).expect("bak"), original);
    assert!(!manager.has_unreadable_entries());

    // Later saves do not keep making backups.
    manager.save().expect("save again");
    assert_eq!(backups(dir.path()).len(), 1);
}

#[test]
fn test_devicedb_corrupt_file_is_backed_up_before_overwrite() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("devices.json");
    std::fs::write(&path, "{ not json").expect("write");

    let manager = DeviceManager::new(path.clone());
    assert!(manager.load().is_err());
    // The UI ignores the load error and may then save a new profile.
    manager
        .save_profile(DeviceProfile::default())
        .expect("save after failed load");
    let baks = backups(dir.path());
    assert_eq!(baks.len(), 1);
    assert_eq!(
        std::fs::read_to_string(&baks[0]).expect("bak"),
        "{ not json"
    );
}

#[test]
fn test_devicedb_set_active_rolls_back_when_save_fails() {
    // Review of #87: a failed save left the new profile active in memory.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("devices.json");
    let manager = DeviceManager::new(path.clone());
    let first = DeviceProfile {
        id: "first".to_string(),
        ..DeviceProfile::default()
    };
    let second = DeviceProfile {
        id: "second".to_string(),
        ..DeviceProfile::default()
    };
    manager.save_profile(first).expect("save first");
    manager.save_profile(second).expect("save second");
    manager.set_active_profile("first").expect("activate first");

    // Make the next save fail: the profiles file becomes a directory.
    std::fs::remove_file(&path).expect("remove file");
    std::fs::create_dir(&path).expect("block path");

    assert!(manager.set_active_profile("second").is_err());
    assert_eq!(
        manager.get_active_profile().map(|p| p.id),
        Some("first".to_string())
    );
}

/// A fresh folder `target/temp/devicedb/<name>-<pid>`.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/devicedb")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

/// devices.json is saved through the shared atomic writer: the file's permissions are
/// kept, no temporary file is left, and a read-only file is refused rather than replaced.
#[cfg(unix)]
#[test]
fn test_devicedb_save_keeps_permissions_and_refuses_a_read_only_file() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("atomic_save");
    let path = dir.join("devices.json");
    let manager = DeviceManager::new(path.clone());
    manager
        .save_profile(DeviceProfile {
            id: "first".to_string(),
            ..DeviceProfile::default()
        })
        .expect("save");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("chmod");
    manager
        .save_profile(DeviceProfile {
            id: "second".to_string(),
            ..DeviceProfile::default()
        })
        .expect("save over the private file");
    let mode = std::fs::metadata(&path).expect("meta").permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "permissions not kept: {mode:o}");
    let reloaded = DeviceManager::new(path.clone());
    reloaded.load().expect("reload");
    assert_eq!(reloaded.get_all_profiles().len(), 2);
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .expect("read_dir")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).expect("chmod");
    let before = std::fs::read_to_string(&path).expect("read");
    assert!(
        manager.delete_profile("second").is_err(),
        "a read-only file was replaced"
    );
    assert_eq!(std::fs::read_to_string(&path).expect("read"), before);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("chmod");
}
