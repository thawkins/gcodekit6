//! Locks that a custom tool's number survives a save and a reload (audit T15, P2FW-1),
//! and that a `custom_tools.json` written before the number was stored still loads.

use gcodekit5_app::tools_manager_backend::ToolsManagerBackend;
use gcodekit5_core::data::tools::{Tool, ToolId, ToolType};

/// A fresh storage file of this test's own under `target/temp`.
fn test_storage(test: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/tools_manager_number")
        .join(format!("{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test storage dir");
    dir.join("custom_tools.json")
}

fn number_on_disk(storage: &std::path::Path, id: &str) -> Option<u32> {
    ToolsManagerBackend::with_storage_path(storage.to_path_buf())
        .get_tool(&ToolId(id.to_string()))
        .map(|t| t.number)
}

#[test]
fn test_tools_manager_number_survives_save_and_reload() {
    let storage = test_storage("survives");
    let mut tool = Tool::new(
        ToolId("numbered_tool".to_string()),
        42,
        "Numbered Tool".to_string(),
        ToolType::EndMillFlat,
        6.0,
        40.0,
    );
    tool.custom = true;
    let mut backend = ToolsManagerBackend::with_storage_path(storage.clone());
    backend.save_tool(tool.clone()).expect("save");
    assert_eq!(number_on_disk(&storage, "numbered_tool"), Some(42));

    // Edit the number and save again: the edit is kept after a restart.
    tool.number = 7;
    backend.save_tool(tool).expect("save edit");
    assert_eq!(number_on_disk(&storage, "numbered_tool"), Some(7));
}

#[test]
fn test_tools_manager_number_missing_in_old_file_loads_as_zero() {
    let storage = test_storage("old_file");
    let old_json = r#"[
  {
    "id": "old_tool",
    "name": "Old Tool",
    "description": "",
    "tool_type": "EndMillFlat",
    "diameter": 3.175,
    "shaft_diameter": null,
    "length": 38.0,
    "flute_length": 12.0,
    "flutes": 2,
    "corner_radius": null,
    "tip_angle": null,
    "material": "Carbide",
    "coating": null,
    "shank": "Collet",
    "params": {
      "rpm": 18000,
      "rpm_range": [12000, 24000],
      "feed_rate": 900.0,
      "plunge_rate": 300.0,
      "stepover_percent": 40.0,
      "depth_per_pass": 1.0
    },
    "manufacturer": null,
    "part_number": null,
    "cost": null,
    "notes": "",
    "custom": true
  }
]"#;
    std::fs::write(&storage, old_json).expect("write old file");
    let backend = ToolsManagerBackend::with_storage_path(storage.clone());
    assert!(backend.load_error().is_none(), "old file rejected");
    let tool = backend
        .get_tool(&ToolId("old_tool".to_string()))
        .expect("old tool loaded");
    assert_eq!(tool.name, "Old Tool");
    assert_eq!(tool.number, 0);
}
