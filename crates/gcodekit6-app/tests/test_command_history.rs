//! Tests for `gcodekit6_app::command_history` (moved from the inline test module).

use gcodekit6_app::command_history::*;

#[test]
fn test_add_command() {
    let mut history = CommandHistory::new();
    history.add("G0 X10".to_string());
    history.add("G0 Y20".to_string());

    assert_eq!(history.len(), 2);
}

#[test]
fn test_no_duplicate_consecutive() {
    let mut history = CommandHistory::new();
    history.add("G0 X10".to_string());
    history.add("G0 X10".to_string());

    assert_eq!(history.len(), 1);
}

#[test]
fn test_navigate_up() {
    let mut history = CommandHistory::new();
    history.add("cmd1".to_string());
    history.add("cmd2".to_string());
    history.add("cmd3".to_string());

    let prev = history.previous("current");
    assert_eq!(prev, Some("cmd3".to_string()));

    let prev = history.previous("current");
    assert_eq!(prev, Some("cmd2".to_string()));
}

#[test]
fn test_navigate_down() {
    let mut history = CommandHistory::new();
    history.add("cmd1".to_string());
    history.add("cmd2".to_string());

    history.previous("current");
    history.previous("current");

    let next = history.next();
    assert_eq!(next, Some("cmd2".to_string()));

    let next = history.next();
    assert_eq!(next, Some("current".to_string()));
}

#[test]
fn test_max_history() {
    let mut history = CommandHistory::new();

    for i in 0..300 {
        history.add(format!("cmd{}", i));
    }

    assert_eq!(history.len(), MAX_HISTORY);
    assert_eq!(history.get(0), Some("cmd50"));
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/command_history")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// The history survives a save and load through GTK's file, keeping its limit.
#[test]
fn test_history_file_round_trip() {
    let file = scratch("round_trip").join(HISTORY_FILE);
    let mut history = CommandHistory::new();
    for i in 0..(MAX_HISTORY + 5) {
        history.add(format!("G0 X{i}"));
    }
    history.save_to(&file).expect("saved");
    let loaded = CommandHistory::load_from(&file);
    assert_eq!(loaded.len(), MAX_HISTORY);
    assert_eq!(loaded.get(0), Some("G0 X5"));
    assert_eq!(
        loaded.get(MAX_HISTORY - 1).map(str::to_owned),
        Some(format!("G0 X{}", MAX_HISTORY + 4))
    );
    let names: Vec<_> = std::fs::read_dir(file.parent().expect("dir"))
        .expect("dir")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(names, [HISTORY_FILE], "no temporary file left");
}

/// A missing or damaged file gives an empty history; a longer file is cut to the limit.
#[test]
fn test_history_file_missing_damaged_or_long() {
    let dir = scratch("damaged");
    assert!(CommandHistory::load_from(&dir.join("missing.json")).is_empty());
    let damaged = dir.join("damaged.json");
    std::fs::write(&damaged, "{ not json").expect("write");
    assert!(CommandHistory::load_from(&damaged).is_empty());
    let long = dir.join("long.json");
    let commands: Vec<String> = (0..300).map(|i| format!("cmd{i}")).collect();
    std::fs::write(
        &long,
        serde_json::json!({ "commands": commands }).to_string(),
    )
    .expect("write");
    let loaded = CommandHistory::load_from(&long);
    assert_eq!(loaded.len(), MAX_HISTORY);
    assert_eq!(loaded.get(0), Some("cmd50"));
}

/// Saving creates a missing settings folder, as GTK's save did.
#[test]
fn test_history_save_creates_missing_folder() {
    let file = scratch("missing_folder")
        .join("legacy")
        .join("gcodekit6")
        .join(HISTORY_FILE);
    let mut history = CommandHistory::new();
    history.add("$H".to_string());
    history.save_to(&file).expect("saved into a new folder");
    assert_eq!(CommandHistory::load_from(&file).get(0), Some("$H"));
}
