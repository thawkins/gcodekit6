//! The CNC Tools window: the GTK build's `custom_tools.json` loads and saves back, tools are
//! created, edited, validated and deleted, the speeds and feeds calculator sees every change
//! without a restart, an unreadable library is backed up rather than wiped, and GTC import
//! and custom tool export work.

use std::path::{Path, PathBuf};

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::tools_manager_backend::ToolsManagerBackend;
use gcodekit5_core::data::tools::{ShankType, Tool, ToolId, ToolType};
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::commands::Command;
use gcodekit5_ui_egui::windows::tools_manager::GtcFile;
use gcodekit5_ui_egui::{AppState, GcodeKitApp};

use crate::machine_support::app_harness;

/// A custom tools file as the GTK build's `ToolsManagerBackend` writes it.
const GTK_TOOLS: &str = r#"[
  {
    "id": "my_router_bit",
    "name": "My Router Bit",
    "description": "Downcut spiral",
    "tool_type": "EndMillFlat",
    "diameter": 3.175,
    "shaft_diameter": 3.175,
    "length": 38.0,
    "flute_length": 12.0,
    "flutes": 2,
    "corner_radius": null,
    "tip_angle": null,
    "material": "Carbide",
    "coating": "TiAlN",
    "shank": { "Straight": 31 },
    "params": {
      "rpm": 18000,
      "rpm_range": [12000, 24000],
      "feed_rate": 900.0,
      "plunge_rate": 300.0,
      "stepover_percent": 40.0,
      "depth_per_pass": 1.0
    },
    "manufacturer": "Acme",
    "part_number": "A-31",
    "cost": 12.5,
    "notes": "Keep sharp",
    "custom": true
  },
  {
    "id": "my_odd_bit",
    "name": "My Odd Bit",
    "description": "",
    "tool_type": "EndMillFlat",
    "diameter": 10.0,
    "shaft_diameter": null,
    "length": 60.0,
    "flute_length": 30.0,
    "flutes": 12,
    "corner_radius": null,
    "tip_angle": null,
    "material": "HSS",
    "coating": null,
    "shank": "Collet",
    "params": {
      "rpm": 8000,
      "rpm_range": [6000, 10000],
      "feed_rate": 500.0,
      "plunge_rate": 100.0,
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

/// A fresh scratch folder `target/temp/egui_tools_manager/<name>-<pid>`.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/egui_tools_manager")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

/// The tools file in a fresh scratch folder, holding `content` when given.
fn tools_file(name: &str, content: Option<&str>) -> PathBuf {
    let file = scratch(name).join("custom_tools.json");
    if let Some(content) = content {
        std::fs::write(&file, content).expect("write custom_tools.json");
    }
    file
}

/// The app keeping its custom tools in `file`, with the CNC Tools window open.
fn app_with(file: &Path) -> Harness<'static, GcodeKitApp> {
    let state = AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    )
    .with_tools_file(file.to_path_buf());
    let mut h = app_harness(GcodeKitApp::from_state(state));
    h.state_mut().dispatch(Command::OpenToolsManager);
    h.run_steps(3);
    assert!(h.query_by_label("CNC Tools").is_some(), "window not open");
    h
}

/// The custom tool `id` as the GTK build reads it back from `file`.
fn stored(file: &Path, id: &str) -> Option<Tool> {
    ToolsManagerBackend::with_storage_path(file.to_path_buf())
        .get_tool(&ToolId(id.to_string()))
        .filter(|t| t.custom)
        .cloned()
}

fn set_field(h: &mut Harness<'_, GcodeKitApp>, label: &str, text: &str) {
    set_input(h, Role::TextInput, label, text);
}

/// Replaces the text of the input labelled `label` (of `role`) with `text`.
fn set_input(h: &mut Harness<'_, GcodeKitApp>, role: Role, label: &str, text: &str) {
    h.get_by_role_and_label(role, label).focus();
    h.step();
    h.key_combination_modifiers(egui::Modifiers::COMMAND, &[egui::Key::A]);
    h.event(egui::Event::Text(text.to_string()));
    h.step();
}

fn click(h: &mut Harness<'_, GcodeKitApp>, label: &str) {
    h.get_by_label(label).click();
    h.run_steps(2);
}

/// Narrows the list to `text` (the search field).
fn search(h: &mut Harness<'_, GcodeKitApp>, text: &str) {
    h.state_mut().tools_manager_mut().filter_mut().search = text.to_string();
    h.run_steps(2);
}

fn notice(h: &Harness<'_, GcodeKitApp>) -> String {
    h.state()
        .tools_manager()
        .notice()
        .unwrap_or_default()
        .to_string()
}

/// Whether the speeds and feeds calculator lists a tool named `name`.
fn calculator_has(h: &Harness<'_, GcodeKitApp>, name: &str) -> bool {
    h.state()
        .cam_tools()
        .borrow_mut()
        .speeds_feeds_mut()
        .tools
        .iter()
        .any(|t| t.name == name)
}

#[test]
fn test_tools_manager_gtk_file_loads_and_saves_back() {
    let file = tools_file("gtk_file", Some(GTK_TOOLS));
    let before = stored(&file, "my_router_bit").expect("fixture loads");
    let mut h = app_with(&file);
    assert!(calculator_has(&h, "My Router Bit"), "startup library");
    search(&mut h, "router bit");
    click(&mut h, "My Router Bit");
    click(&mut h, "Geometry");
    assert_eq!(
        h.get_by_role_and_label(Role::TextInput, "Diameter (mm):")
            .value()
            .as_deref(),
        Some("3.175")
    );
    click(&mut h, "Notes");
    set_input(
        &mut h,
        Role::MultilineTextInput,
        "Additional Notes:",
        "Resharpened",
    );
    click(&mut h, "Save");
    assert!(notice(&h).contains("Tool saved"), "{}", notice(&h));
    let after = stored(&file, "my_router_bit").expect("saved");
    assert_eq!(after.notes, "Resharpened");
    let mut expected = before.clone();
    expected.notes = "Resharpened".to_string();
    assert_eq!(
        serde_json::to_value(&after).expect("json"),
        serde_json::to_value(&expected).expect("json"),
        "only the edited field changed"
    );
    assert_eq!(after.params.rpm, 18000, "cutting parameters kept");
    assert_eq!(after.shank, ShankType::Straight(31));
}

#[test]
fn test_tools_manager_create_edit_delete() {
    let file = tools_file("crud", None);
    let mut h = app_with(&file);
    click(&mut h, "New Tool");
    set_field(&mut h, "ID:", "bench_vbit");
    set_field(&mut h, "Name:", "Bench V-Bit");
    click(&mut h, "Save");
    assert!(notice(&h).contains("Tool saved"), "{}", notice(&h));
    assert_eq!(
        stored(&file, "bench_vbit").map(|t| t.name),
        Some("Bench V-Bit".to_string())
    );
    assert!(
        calculator_has(&h, "Bench V-Bit"),
        "calculator sees the new tool"
    );

    search(&mut h, "bench");
    click(&mut h, "Bench V-Bit");
    click(&mut h, "Geometry");
    set_field(&mut h, "Diameter (mm):", "4");
    click(&mut h, "Save");
    let tool = stored(&file, "bench_vbit").expect("saved");
    assert_eq!(tool.diameter, 4.0);
    assert_eq!(
        h.state()
            .cam_tools()
            .borrow_mut()
            .speeds_feeds_mut()
            .tools
            .iter()
            .find(|t| t.id.0 == "bench_vbit")
            .map(|t| t.diameter),
        Some(4.0),
        "calculator sees the edit"
    );

    click(&mut h, "Delete");
    assert!(h.query_by_label("Delete tool?").is_some(), "not asked");
    click(&mut h, "Yes");
    assert!(stored(&file, "bench_vbit").is_none());
    assert!(
        !calculator_has(&h, "Bench V-Bit"),
        "calculator sees the delete"
    );
}

#[test]
fn test_tools_manager_validation_refuses_bad_values() {
    let file = tools_file("validation", None);
    let mut h = app_with(&file);
    click(&mut h, "New Tool");
    set_field(&mut h, "ID:", "no_name");
    click(&mut h, "Save");
    assert!(
        notice(&h).contains("Tool name is required"),
        "{}",
        notice(&h)
    );
    set_field(&mut h, "Name:", "Named");
    click(&mut h, "Geometry");
    set_field(&mut h, "Length (mm):", "long");
    click(&mut h, "Save");
    assert!(notice(&h).contains("Invalid length"), "{}", notice(&h));
    assert!(!file.exists(), "nothing written");
    // A standard tool's id is taken.
    let standard = ToolsManagerBackend::with_storage_path(file.clone()).get_all_tools()[0]
        .id
        .0
        .clone();
    click(&mut h, "Basic Info");
    set_field(&mut h, "ID:", &standard);
    click(&mut h, "Geometry");
    set_field(&mut h, "Length (mm):", "50");
    click(&mut h, "Save");
    assert!(notice(&h).contains("already exists"), "{}", notice(&h));
    assert!(!file.exists(), "nothing written");
}

#[test]
fn test_tools_manager_switching_away_asks_about_unsaved_changes() {
    let file = tools_file("unsaved", Some(GTK_TOOLS));
    let mut h = app_with(&file);
    search(&mut h, "router bit");
    click(&mut h, "My Router Bit");
    set_field(&mut h, "Name:", "Renamed Bit");
    click(&mut h, "New Tool");
    assert!(h.query_by_label("Unsaved changes").is_some(), "not asked");
    click(&mut h, "Discard");
    assert_eq!(
        h.get_by_role_and_label(Role::TextInput, "ID:")
            .value()
            .as_deref(),
        Some(""),
        "the new tool form is open"
    );
    assert_eq!(
        stored(&file, "my_router_bit").map(|t| t.name),
        Some("My Router Bit".to_string()),
        "the edit was dropped"
    );
}

#[test]
fn test_tools_manager_unreadable_file_is_backed_up() {
    let file = tools_file("unreadable", Some("[{ broken"));
    let mut h = app_with(&file);
    assert!(notice(&h).contains("could not be read"), "{}", notice(&h));
    click(&mut h, "New Tool");
    set_field(&mut h, "ID:", "after_broken");
    set_field(&mut h, "Name:", "After Broken");
    click(&mut h, "Save");
    assert!(notice(&h).contains(".bak-"), "{}", notice(&h));
    let backup = std::fs::read_dir(file.parent().expect("dir"))
        .expect("dir")
        .filter_map(Result::ok)
        .find(|e| e.file_name().to_string_lossy().contains(".bak-"))
        .expect("backup file");
    assert_eq!(
        std::fs::read_to_string(backup.path()).expect("read"),
        "[{ broken"
    );
    assert!(stored(&file, "after_broken").is_some());
}

#[test]
fn test_tools_manager_gtc_import_and_export() {
    let dir = scratch("gtc");
    let file = dir.join("custom_tools.json");
    let catalog = dir.join("catalog.json");
    std::fs::write(
        &catalog,
        r#"{ "Version": "1", "Manufacturer": "Acme", "Tools": [
            { "ID": "AC 100", "Description": "Acme Ball 2mm", "Type": "Ball End Mill",
              "Diameter": 2.0, "Length": 40.0 } ] }"#,
    )
    .expect("catalog");
    let mut h = app_with(&file);
    let tr = Translator::new("en");
    h.state_mut()
        .tools_manager_mut()
        .import_gtc(&catalog, GtcFile::Json, &tr);
    h.run_steps(2);
    assert!(notice(&h).contains("Imported: 1"), "{}", notice(&h));
    let imported = stored(&file, "gtc_ac_100").expect("imported and saved");
    assert_eq!(imported.tool_type, ToolType::EndMillBall);
    assert!(
        calculator_has(&h, "Acme Ball 2mm"),
        "calculator sees the import"
    );

    let export = dir.join("export.json");
    h.state_mut().tools_manager_mut().export_to(&export, &tr);
    let exported = ToolsManagerBackend::with_storage_path(export);
    assert!(
        exported
            .get_tool(&ToolId("gtc_ac_100".to_string()))
            .is_some()
    );

    click(&mut h, "Reset Custom Tools...");
    assert!(
        h.query_by_label("Reset custom tools?").is_some(),
        "not asked"
    );
    click(&mut h, "Yes");
    assert!(!file.exists(), "custom tools file removed");
    assert!(!calculator_has(&h, "Acme Ball 2mm"));
}

/// Whether the action bar's Save is disabled.
fn save_disabled(h: &Harness<'_, GcodeKitApp>) -> bool {
    h.get_by_label("Save").accesskit_node().is_disabled()
}

#[test]
fn test_tools_manager_showing_a_stored_tool_is_not_an_edit() {
    // The file never stores the tool number (0 after loading), and a 12-flute tool is
    // outside the flutes field's range: drawing them must not change the form.
    let file = tools_file("not_dirty", Some(GTK_TOOLS));
    let mut h = app_with(&file);
    search(&mut h, "my_");
    click(&mut h, "My Router Bit");
    assert!(save_disabled(&h), "Save enabled without an edit");
    click(&mut h, "Geometry");
    click(&mut h, "My Odd Bit");
    assert!(h.query_by_label("Unsaved changes").is_none(), "asked");
    assert!(save_disabled(&h), "Save enabled without an edit");
    click(&mut h, "Basic Info");
    click(&mut h, "My Router Bit");
    assert!(h.query_by_label("Unsaved changes").is_none(), "asked");
    click(&mut h, "New Tool");
    assert!(h.query_by_label("Unsaved changes").is_none(), "asked");
    click(&mut h, "Cancel");
    assert!(h.query_by_label("Unsaved changes").is_none(), "asked");
    assert_eq!(
        stored(&file, "my_odd_bit").map(|t| t.flutes),
        Some(12),
        "file untouched"
    );
}

#[test]
fn test_tools_manager_filters_are_stored_when_they_change() {
    let dir = scratch("filters");
    let dirs = gcodekit5_settings::migrate::ConfigDirs::new(dir.join("legacy"), dir.join("config"));
    let (settings, _problem) = SettingsManager::open(dirs.clone());
    let state = AppState::new(settings, crate::test_smoke::native_decision())
        .with_tools_file(dir.join("custom_tools.json"));
    let mut h = app_harness(GcodeKitApp::from_state(state));
    h.state_mut().dispatch(Command::OpenToolsManager);
    h.run_steps(3);
    h.state_mut().tools_manager_mut().filter_mut().tool_type = Some(ToolType::VBit);
    search(&mut h, "bit");
    // Stored while the window is still open, and so in the settings file.
    let ui = &h.state().state().config().ui;
    assert_eq!(ui.tools_manager_type_filter, "vbit");
    assert_eq!(ui.tools_manager_search, "bit");
    let (reloaded, _problem) = SettingsManager::open(dirs);
    assert_eq!(reloaded.config().ui.tools_manager_type_filter, "vbit");
    assert!(h.state().tools_manager().is_open());
}

#[test]
fn test_tools_manager_untouched_filters_are_not_written() {
    let dir = scratch("filters_untouched");
    let dirs = gcodekit5_settings::migrate::ConfigDirs::new(dir.join("legacy"), dir.join("config"));
    let (settings, _problem) = SettingsManager::open(dirs);
    let state = AppState::new(settings, crate::test_smoke::native_decision())
        .with_tools_file(dir.join("custom_tools.json"));
    let mut h = app_harness(GcodeKitApp::from_state(state));
    h.run_steps(3);
    assert!(
        h.state()
            .state()
            .config()
            .ui
            .tools_manager_type_filter
            .is_empty(),
        "settings rewritten without a change"
    );
}

/// Quits through the menu, answering every message dialog with `reply`.
///
/// # Returns
/// The titles of the dialogs shown.
pub(crate) fn quit_answering(
    h: &mut Harness<'_, GcodeKitApp>,
    reply: craft_dialogs::MessageResult,
) -> Vec<String> {
    let shown = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = std::sync::Arc::clone(&shown);
    h.state_mut().dialogs_mut().set_override(Some(Box::new(
        move |request: &craft_dialogs::Request| match request {
            craft_dialogs::Request::Message(message) => {
                seen.lock().expect("shown").push(message.title.clone());
                craft_dialogs::Answer::Message(reply.clone())
            }
            craft_dialogs::Request::File(file) => panic!("unexpected file dialog {file:?}"),
        },
    )));
    h.state_mut()
        .request_quit(gcodekit5_ui_egui::QuitSource::MenuQuit);
    for _ in 0..4 {
        h.step();
        h.state_mut()
            .dialogs_mut()
            .end_frame(None::<&eframe::Frame>);
    }
    let titles = shown.lock().expect("shown").clone();
    titles
}

#[test]
fn test_tools_manager_quit_asks_and_saves_an_unsaved_tool() {
    let file = tools_file("quit_save", Some(GTK_TOOLS));
    let mut h = app_with(&file);
    search(&mut h, "router bit");
    click(&mut h, "My Router Bit");
    set_field(&mut h, "Name:", "Quit Saved Bit");
    // Closing the window (its title-bar button) keeps the edit; the quit asks about it.
    h.state_mut().tools_manager_mut().close();
    h.run_steps(2);
    assert!(!h.state().tools_manager().is_open());
    let shown = quit_answering(&mut h, craft_dialogs::MessageResult::Custom("Save".into()));
    assert_eq!(shown, ["Unsaved changes"]);
    assert_eq!(
        stored(&file, "my_router_bit").map(|t| t.name),
        Some("Quit Saved Bit".to_string())
    );
    assert!(h.state().quit_confirmed());
}

#[test]
fn test_tools_manager_quit_cancel_keeps_the_edit() {
    let file = tools_file("quit_cancel", Some(GTK_TOOLS));
    let mut h = app_with(&file);
    search(&mut h, "router bit");
    click(&mut h, "My Router Bit");
    set_field(&mut h, "Name:", "Unsaved Bit");
    let shown = quit_answering(&mut h, craft_dialogs::MessageResult::Cancel);
    assert_eq!(shown, ["Unsaved changes"]);
    assert!(!h.state().quit_confirmed());
    assert_eq!(
        h.get_by_role_and_label(Role::TextInput, "Name:")
            .value()
            .as_deref(),
        Some("Unsaved Bit"),
        "the edit is still open"
    );
    assert_eq!(
        stored(&file, "my_router_bit").map(|t| t.name),
        Some("My Router Bit".to_string())
    );
}
