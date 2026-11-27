//! The custom library file both manager backends share (custom tools and custom
//! materials): an unreadable file is copied aside exactly once, before the first write or
//! reset, and never wiped.

use gcodekit5_app::material_form::MaterialForm;
use gcodekit5_app::materials_manager_backend::{MATERIALS_FILE, MaterialsManagerBackend};
use gcodekit5_app::tool_form::ToolForm;
use gcodekit5_app::tools_manager_backend::{TOOLS_FILE, ToolsManagerBackend};
use std::path::{Path, PathBuf};

/// A fresh folder `target/temp/custom_library_file/<name>-<pid>`.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/custom_library_file")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// The backups in `dir`.
fn backups(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .expect("read dir")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.to_string_lossy().contains(".bak-"))
        .collect()
}

fn material(id: &str) -> gcodekit5_core::data::materials::Material {
    MaterialForm {
        id: id.into(),
        name: id.into(),
        subcategory: "S".into(),
        ..MaterialForm::default()
    }
    .build(None)
    .expect("valid")
}

#[test]
fn test_custom_library_file_backs_up_once_across_saves() {
    let dir = scratch("materials_once");
    let file = dir.join(MATERIALS_FILE);
    std::fs::write(&file, "garbage").expect("write");
    let mut backend = MaterialsManagerBackend::with_storage_path(file.clone());
    assert!(backend.load_error().is_some());
    backend.save_material(material("m1")).expect("save");
    let first = backend.last_backup().expect("backed up").to_path_buf();
    backend.save_material(material("m2")).expect("save");
    assert_eq!(backend.last_backup(), Some(first.as_path()));
    assert_eq!(backups(&dir), vec![first.clone()]);
    assert_eq!(std::fs::read_to_string(&first).expect("backup"), "garbage");
    let reloaded = MaterialsManagerBackend::with_storage_path(file);
    assert!(reloaded.load_error().is_none());
}

#[test]
fn test_custom_library_file_reset_backs_up_unreadable_tools() {
    let dir = scratch("tools_reset");
    let file = dir.join(TOOLS_FILE);
    std::fs::write(&file, "[{ not json").expect("write");
    let mut backend = ToolsManagerBackend::with_storage_path(file.clone());
    assert!(backend.load_error().is_some());
    backend.reset_custom_tools().expect("reset");
    assert!(!file.exists(), "the file is deleted");
    let backup = backend.last_backup().expect("backed up").to_path_buf();
    assert_eq!(
        std::fs::read_to_string(&backup).expect("backup"),
        "[{ not json"
    );
    // The file is gone, so a later save writes it without a second backup.
    let tool = ToolForm {
        id: "after_reset".into(),
        name: "After Reset".into(),
        ..ToolForm::default()
    }
    .build(None)
    .expect("valid");
    backend.save_tool(tool).expect("save");
    assert_eq!(backups(&dir), vec![backup]);
    assert!(file.exists());
}

#[test]
fn test_custom_library_file_missing_file_is_not_an_error() {
    let dir = scratch("missing");
    let mut tools = ToolsManagerBackend::with_storage_path(dir.join(TOOLS_FILE));
    let materials = MaterialsManagerBackend::with_storage_path(dir.join(MATERIALS_FILE));
    assert!(tools.load_error().is_none());
    assert!(materials.load_error().is_none());
    assert_eq!(
        materials.get_all_materials().len(),
        gcodekit5_core::data::materials::init_standard_library()
            .get_all_materials()
            .len()
    );
    tools.reset_custom_tools().expect("reset of a missing file");
    assert!(tools.last_backup().is_none());
    assert!(backups(&dir).is_empty());
}
