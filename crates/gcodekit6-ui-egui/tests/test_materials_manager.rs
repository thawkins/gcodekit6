//! The Materials Manager window: the GTK build's `custom_materials.json` loads and saves
//! back, materials are created, edited, validated and deleted (custom ones only), the speeds
//! and feeds calculator sees every change without a restart, and an unreadable library is
//! backed up rather than wiped.

use std::path::{Path, PathBuf};

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use gcodekit6_app::materials_manager_backend::MaterialsManagerBackend;
use gcodekit6_core::data::materials::{Material, MaterialId};
use gcodekit6_settings::SettingsManager;
use gcodekit6_settings::config::Config;
use gcodekit6_ui_egui::commands::Command;
use gcodekit6_ui_egui::workspace::Page;
use gcodekit6_ui_egui::{AppState, GcodeKitApp};

use crate::machine_support::app_harness;

/// A custom materials file as the GTK build's `MaterialsManagerBackend` writes it.
const GTK_MATERIALS: &str = r#"[
  {
    "id": "bog_oak",
    "name": "Bog Oak",
    "category": "Wood",
    "subcategory": "Hardwood",
    "description": "Very dense, dark",
    "density": 1100.0,
    "machinability_rating": 5,
    "tensile_strength": 90.0,
    "melting_point": null,
    "brinell_hardness": 4.5,
    "rockwell_hardness": null,
    "thermal_conductivity": null,
    "specific_cutting_force": null,
    "chip_type": "Segmented",
    "heat_sensitivity": "Moderate",
    "abrasiveness": "High",
    "surface_finish": "Fair",
    "dust_hazard": "High",
    "fume_hazard": "Minimal",
    "required_ppe": ["EyeProtection", "Respiratory"],
    "coolant_required": false,
    "cutting_params": {},
    "custom": true,
    "notes": "Sharp tools only"
  }
]"#;

/// A fresh scratch folder `target/temp/egui_materials_manager/<name>-<pid>`.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/egui_materials_manager")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

/// The materials file in a fresh scratch folder, holding `content` when given.
fn materials_file(name: &str, content: Option<&str>) -> PathBuf {
    let file = scratch(name).join("custom_materials.json");
    if let Some(content) = content {
        std::fs::write(&file, content).expect("write custom_materials.json");
    }
    file
}

/// The app keeping its custom materials in `file`, with the Materials Manager open.
fn app_with(file: &Path) -> Harness<'static, GcodeKitApp> {
    let state = AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    )
    .with_materials_file(file.to_path_buf());
    let mut h = app_harness(GcodeKitApp::from_state(state));
    h.state_mut().dispatch(Command::OpenMaterialsManager);
    h.run_steps(3);
    assert_eq!(h.state().page(), Page::Materials, "page not shown");
    h
}

/// The custom material `id` as the GTK build reads it back from `file`.
fn stored(file: &Path, id: &str) -> Option<Material> {
    MaterialsManagerBackend::with_storage_path(file.to_path_buf())
        .get_material(&MaterialId(id.to_string()))
        .filter(|m| m.custom)
        .cloned()
}

/// Replaces the text of the input labelled `label` (of `role`) with `text`.
fn set_input(h: &mut Harness<'_, GcodeKitApp>, role: Role, label: &str, text: &str) {
    h.get_by_role_and_label(role, label).focus();
    h.step();
    h.key_combination_modifiers(egui::Modifiers::COMMAND, &[egui::Key::A]);
    if text.is_empty() {
        h.key_press(egui::Key::Backspace);
    } else {
        h.event(egui::Event::Text(text.to_string()));
    }
    h.step();
}

fn set_field(h: &mut Harness<'_, GcodeKitApp>, label: &str, text: &str) {
    set_input(h, Role::TextInput, label, text);
}

fn click(h: &mut Harness<'_, GcodeKitApp>, label: &str) {
    h.get_by_label(label).click();
    h.run_steps(2);
}

/// Clicks the last widget labelled `label` (the window is drawn after the panes, so a
/// label a pane also uses, such as the Properties pane's, is the window's last).
fn click_last(h: &mut Harness<'_, GcodeKitApp>, label: &str) {
    h.get_all_by_label(label).last().expect(label).click();
    h.run_steps(2);
}

/// Narrows the list to `text` (the search field).
fn search(h: &mut Harness<'_, GcodeKitApp>, text: &str) {
    h.state_mut().materials_manager_mut().filter_mut().search = text.to_string();
    h.run_steps(2);
}

fn notice(h: &Harness<'_, GcodeKitApp>) -> String {
    h.state()
        .materials_manager()
        .notice()
        .unwrap_or_default()
        .to_string()
}

/// The speeds and feeds calculator's material `id`, if it lists one.
fn calculator_material(h: &Harness<'_, GcodeKitApp>, id: &str) -> Option<Material> {
    h.state()
        .cam_tools()
        .borrow_mut()
        .speeds_feeds_mut()
        .materials
        .iter()
        .find(|m| m.id.0 == id)
        .cloned()
}

#[test]
fn test_materials_manager_gtk_file_loads_and_saves_back() {
    let file = materials_file("gtk_file", Some(GTK_MATERIALS));
    let before = stored(&file, "bog_oak").expect("fixture loads");
    let mut h = app_with(&file);
    assert!(
        calculator_material(&h, "bog_oak").is_some(),
        "startup library"
    );
    search(&mut h, "bog");
    click(&mut h, "Bog Oak");
    click_last(&mut h, "Properties");
    assert_eq!(
        h.get_by_role_and_label(Role::TextInput, "Tensile Strength (MPa):")
            .value()
            .as_deref(),
        Some("90")
    );
    set_field(&mut h, "Tensile Strength (MPa):", "95");
    click(&mut h, "Save");
    assert!(notice(&h).contains("Material saved"), "{}", notice(&h));
    let after = stored(&file, "bog_oak").expect("saved");
    let mut expected = before.clone();
    expected.tensile_strength = Some(95.0);
    assert_eq!(
        serde_json::to_value(&after).expect("json"),
        serde_json::to_value(&expected).expect("json"),
        "only the edited field changed"
    );
    assert_eq!(
        calculator_material(&h, "bog_oak").and_then(|m| m.tensile_strength),
        Some(95.0),
        "calculator sees the edit"
    );
}

#[test]
fn test_materials_manager_create_edit_delete() {
    let file = materials_file("crud", None);
    let mut h = app_with(&file);
    click(&mut h, "New Material");
    set_field(&mut h, "ID:", "my_mdf");
    set_field(&mut h, "Name:", "My MDF");
    set_field(&mut h, "Subcategory:", "Fiberboard");
    click(&mut h, "Save");
    assert!(notice(&h).contains("Material saved"), "{}", notice(&h));
    assert_eq!(
        stored(&file, "my_mdf").map(|m| m.name),
        Some("My MDF".to_string())
    );
    assert!(
        calculator_material(&h, "my_mdf").is_some(),
        "calculator sees it"
    );

    click(&mut h, "Notes");
    set_input(
        &mut h,
        Role::MultilineTextInput,
        "Additional Notes and Tips:",
        "Dusty",
    );
    click(&mut h, "Save");
    assert_eq!(
        stored(&file, "my_mdf").map(|m| m.notes),
        Some("Dusty".to_string())
    );

    click(&mut h, "Delete");
    assert!(h.query_by_label("Delete material?").is_some(), "not asked");
    click(&mut h, "Yes");
    assert!(stored(&file, "my_mdf").is_none());
    assert!(
        calculator_material(&h, "my_mdf").is_none(),
        "calculator sees the delete"
    );
}

#[test]
fn test_materials_manager_validation_refuses_bad_values() {
    let file = materials_file("validation", Some(GTK_MATERIALS));
    let mut h = app_with(&file);
    click(&mut h, "New Material");
    set_field(&mut h, "ID:", "with space");
    set_field(&mut h, "Name:", "Spacey");
    set_field(&mut h, "Subcategory:", "Test");
    click(&mut h, "Save");
    assert!(
        notice(&h).contains("must not contain spaces"),
        "{}",
        notice(&h)
    );
    set_field(&mut h, "ID:", "bog_oak");
    click(&mut h, "Save");
    assert!(notice(&h).contains("already exists"), "{}", notice(&h));
    set_field(&mut h, "Subcategory:", "");
    click(&mut h, "Save");
    assert!(
        notice(&h).contains("Subcategory is required"),
        "{}",
        notice(&h)
    );
    assert_eq!(
        stored(&file, "bog_oak").map(|m| m.name),
        Some("Bog Oak".to_string()),
        "nothing replaced"
    );
}

#[test]
fn test_materials_manager_standard_material_edits_as_override() {
    let file = materials_file("standard", None);
    let standard = MaterialsManagerBackend::with_storage_path(file.clone())
        .get_all_materials()
        .into_iter()
        .find(|m| !m.custom)
        .cloned()
        .expect("standard material");
    let mut h = app_with(&file);
    search(&mut h, &standard.name);
    click(&mut h, &standard.name);
    assert!(
        h.get_by_label("Delete").accesskit_node().is_disabled(),
        "standard materials cannot be deleted"
    );
    set_field(&mut h, "Name:", "My Override");
    click(&mut h, "Save");
    let saved = stored(&file, &standard.id.0).expect("override saved");
    assert_eq!(saved.name, "My Override");
    assert_eq!(
        saved.cutting_params.len(),
        standard.cutting_params.len(),
        "cutting parameters kept"
    );
    // Deleting the override brings the standard material back.
    click(&mut h, "Delete");
    click(&mut h, "Yes");
    assert!(stored(&file, &standard.id.0).is_none());
    assert_eq!(
        calculator_material(&h, &standard.id.0).map(|m| m.name),
        Some(standard.name.clone())
    );
}

#[test]
fn test_materials_manager_switching_away_asks_about_unsaved_changes() {
    let file = materials_file("unsaved", Some(GTK_MATERIALS));
    let mut h = app_with(&file);
    search(&mut h, "bog");
    click(&mut h, "Bog Oak");
    set_field(&mut h, "Name:", "Renamed Oak");
    click(&mut h, "New Material");
    assert!(h.query_by_label("Unsaved changes").is_some(), "not asked");
    // The question's Save comes before the (disabled) action bar's.
    h.get_all_by_label("Save").next().expect("Save").click();
    h.run_steps(2);
    assert_eq!(
        stored(&file, "bog_oak").map(|m| m.name),
        Some("Renamed Oak".to_string()),
        "Save kept the edit"
    );
    assert_eq!(
        h.get_by_role_and_label(Role::TextInput, "ID:")
            .value()
            .as_deref(),
        Some(""),
        "then the new material form opened"
    );
}

#[test]
fn test_materials_manager_unreadable_file_is_backed_up() {
    let file = materials_file("unreadable", Some("not json"));
    let mut h = app_with(&file);
    assert!(notice(&h).contains("could not be read"), "{}", notice(&h));
    click(&mut h, "New Material");
    set_field(&mut h, "ID:", "after_broken");
    set_field(&mut h, "Name:", "After Broken");
    set_field(&mut h, "Subcategory:", "Test");
    click(&mut h, "Save");
    assert!(notice(&h).contains(".bak-"), "{}", notice(&h));
    let backup = std::fs::read_dir(file.parent().expect("dir"))
        .expect("dir")
        .filter_map(Result::ok)
        .find(|e| e.file_name().to_string_lossy().contains(".bak-"))
        .expect("backup file");
    assert_eq!(
        std::fs::read_to_string(backup.path()).expect("read"),
        "not json"
    );
    assert!(stored(&file, "after_broken").is_some());
}

#[test]
fn test_materials_manager_quit_discard_drops_the_edit() {
    let file = materials_file("quit_discard", Some(GTK_MATERIALS));
    let mut h = app_with(&file);
    search(&mut h, "bog");
    click(&mut h, "Bog Oak");
    set_field(&mut h, "Name:", "Never Saved");
    // Cancel from another page brings the Materials page back with the edit.
    h.get_by_role_and_label(Role::Tab, "CAM Tools").click();
    h.run_steps(2);
    let shown =
        crate::test_tools_manager::quit_answering(&mut h, craft_dialogs::MessageResult::Cancel);
    assert_eq!(shown, ["Unsaved changes"]);
    assert!(!h.state().quit_confirmed());
    assert_eq!(h.state().page(), Page::Materials);
    assert!(h.state().materials_manager().has_unsaved_changes());
    let shown = crate::test_tools_manager::quit_answering(
        &mut h,
        craft_dialogs::MessageResult::Custom("Discard".into()),
    );
    assert_eq!(shown, ["Unsaved changes"]);
    assert!(h.state().quit_confirmed());
    assert_eq!(
        stored(&file, "bog_oak").map(|m| m.name),
        Some("Bog Oak".to_string())
    );
}
