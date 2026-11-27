//! The manager pages' shared notice: the load error of an unreadable library file is shown
//! when the page opens, and the backup a save made of it is named once, not on every save.

use std::path::{Path, PathBuf};

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit6_settings::SettingsManager;
use gcodekit6_settings::config::Config;
use gcodekit6_ui_egui::commands::Command;
use gcodekit6_ui_egui::workspace::Page;
use gcodekit6_ui_egui::{AppState, GcodeKitApp};

use crate::machine_support::app_harness;

/// An unreadable `custom_materials.json` in a fresh `target/temp` folder.
fn broken_materials_file(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/egui_library_notice")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch folder");
    let file = dir.join("custom_materials.json");
    std::fs::write(&file, "not json").expect("write custom_materials.json");
    file
}

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

fn set_field(h: &mut Harness<'_, GcodeKitApp>, label: &str, text: &str) {
    h.get_by_role_and_label(Role::TextInput, label).focus();
    h.step();
    h.key_combination_modifiers(egui::Modifiers::COMMAND, &[egui::Key::A]);
    h.event(egui::Event::Text(text.to_string()));
    h.step();
}

fn click(h: &mut Harness<'_, GcodeKitApp>, label: &str) {
    h.get_by_label(label).click();
    h.run_steps(2);
}

fn notice(h: &Harness<'_, GcodeKitApp>) -> String {
    h.state()
        .materials_manager()
        .notice()
        .unwrap_or_default()
        .to_string()
}

#[test]
fn test_library_notice_names_the_backup_once() {
    let file = broken_materials_file("once");
    let mut h = app_with(&file);
    let opened = notice(&h);
    assert!(
        opened.starts_with("The custom materials file could not be read: "),
        "{opened}"
    );
    click(&mut h, "New Material");
    set_field(&mut h, "ID:", "first");
    set_field(&mut h, "Name:", "First");
    set_field(&mut h, "Subcategory:", "Test");
    click(&mut h, "Save");
    let first = notice(&h);
    assert!(
        first.contains(" A copy of the unreadable custom materials file was kept in "),
        "{first}"
    );
    assert!(first.contains(".bak-"), "{first}");
    click(&mut h, "New Material");
    set_field(&mut h, "ID:", "second");
    set_field(&mut h, "Name:", "Second");
    set_field(&mut h, "Subcategory:", "Test");
    click(&mut h, "Save");
    let second = notice(&h);
    assert!(!second.is_empty(), "the save reports");
    assert!(!second.contains(".bak-"), "{second}");
}
