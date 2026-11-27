//! The quit prompt asks about the manager pages' unsaved forms in a fixed order: CNC Tools
//! before Materials (the app's `Managers::unsaved_request`). Cancel brings the page asked
//! about back, which shows which one was asked first.

use std::path::{Path, PathBuf};

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit6_app::materials_manager_backend::MaterialsManagerBackend;
use gcodekit6_app::tools_manager_backend::ToolsManagerBackend;
use gcodekit6_core::data::materials::{Material, MaterialCategory, MaterialId};
use gcodekit6_core::data::tools::{Tool, ToolId, ToolType};
use gcodekit6_settings::SettingsManager;
use gcodekit6_settings::config::Config;
use gcodekit6_ui_egui::commands::Command;
use gcodekit6_ui_egui::workspace::Page;
use gcodekit6_ui_egui::{AppState, GcodeKitApp};

use crate::machine_support::app_harness;

/// A fresh scratch folder `target/temp/egui_managers_quit_order-<pid>`.
fn scratch() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp")
        .join(format!("egui_managers_quit_order-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

/// Replaces the text of the field labelled `label` with `text`.
fn set_field(h: &mut Harness<'_, GcodeKitApp>, label: &str, text: &str) {
    h.get_by_role_and_label(Role::TextInput, label).focus();
    h.step();
    h.key_combination_modifiers(egui::Modifiers::COMMAND, &[egui::Key::A]);
    h.event(egui::Event::Text(text.to_string()));
    h.step();
}

/// Opens `command`'s page, selects the item `item` and renames it, leaving it unsaved,
/// then switches to the Designer.
fn leave_unsaved(h: &mut Harness<'_, GcodeKitApp>, command: Command, item: &str) {
    h.state_mut().dispatch(command);
    h.run_steps(3);
    h.get_by_label(item).click();
    h.run_steps(2);
    set_field(h, "Name:", "Never Saved");
    h.get_by_role_and_label(Role::Tab, "Designer").click();
    h.run_steps(2);
    assert_eq!(h.state().page(), Page::Designer);
}

#[test]
fn test_app_managers_quit_asks_tools_before_materials() {
    let dir = scratch();
    let tools_file = dir.join("custom_tools.json");
    ToolsManagerBackend::with_storage_path(tools_file.clone())
        .save_tool(
            Tool::new(
                ToolId("order_bit".into()),
                1,
                "Order Bit".into(),
                ToolType::EndMillFlat,
                3.0,
                38.0,
            )
            .with_custom(true),
        )
        .expect("write tools");
    let materials_file = dir.join("custom_materials.json");
    let mut material = Material::new(
        MaterialId("order_oak".into()),
        "Order Oak".into(),
        MaterialCategory::Wood,
        "Hardwood".into(),
    );
    material.custom = true;
    MaterialsManagerBackend::with_storage_path(materials_file.clone())
        .save_material(material)
        .expect("write materials");

    let state = AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    )
    .with_tools_file(tools_file)
    .with_materials_file(materials_file);
    let mut h = app_harness(GcodeKitApp::from_state(state));
    // Materials edited first, so the order asked is not the order edited.
    h.state_mut().materials_manager_mut().filter_mut().search = "order".into();
    leave_unsaved(&mut h, Command::OpenMaterialsManager, "Order Oak");
    h.state_mut().tools_manager_mut().filter_mut().search = "order".into();
    leave_unsaved(&mut h, Command::OpenToolsManager, "Order Bit");
    assert!(h.state().tools_manager().has_unsaved_changes());
    assert!(h.state().materials_manager().has_unsaved_changes());

    let shown =
        crate::test_tools_manager::quit_answering(&mut h, craft_dialogs::MessageResult::Cancel);
    assert_eq!(shown, ["Unsaved changes"]);
    assert!(!h.state().quit_confirmed());
    assert_eq!(h.state().page(), Page::CncTools, "CNC Tools asked first");

    let shown = crate::test_tools_manager::quit_answering(
        &mut h,
        craft_dialogs::MessageResult::Custom("Discard".into()),
    );
    assert_eq!(shown, ["Unsaved changes", "Unsaved changes"]);
    assert!(h.state().quit_confirmed());
}
