//! The tool and material manager rules shared by GTK and egui: the forms' validation and
//! the fields they keep, the list filters, and the library files (GTK's path, atomic
//! saves, an unreadable file backed up instead of wiped, a custom override giving way to
//! the standard entry).

use gcodekit5_app::material_form::{MaterialFilter, MaterialForm};
use gcodekit5_app::materials_manager_backend::{MATERIALS_FILE, MaterialsManagerBackend};
use gcodekit5_app::tool_form::{ShankChoice, ToolFilter, ToolForm, tool_contents_equal};
use gcodekit5_app::tools_manager_backend::{TOOLS_FILE, ToolsManagerBackend};
use gcodekit5_core::data::materials::{MaterialCategory, MaterialId};
use gcodekit5_core::data::tools::{ShankType, ToolId, ToolMaterial, ToolType};
use gcodekit5_settings::SettingsManager;
use std::path::PathBuf;

/// A fresh folder `target/temp/tool_material_library/<name>-<pid>`.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/tool_material_library")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn files_in(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("read dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn test_tool_form_validation_messages() {
    let mut form = ToolForm::default();
    assert_eq!(form.build(None).err(), Some("Tool ID is required"));
    form.id = "my_tool".into();
    assert_eq!(form.build(None).err(), Some("Tool name is required"));
    form.name = "My tool".into();
    form.diameter = "abc".into();
    assert_eq!(form.build(None).err(), Some("Invalid diameter"));
    form.diameter = "3.175".into();
    form.set_tool_type(ToolType::VBit);
    form.tip_angle = "x".into();
    assert_eq!(form.build(None).err(), Some("Invalid tip angle"));
    form.tip_angle = "60".into();
    let tool = form.build(None).expect("valid");
    assert!(tool.custom);
    assert_eq!(tool.tip_angle, Some(60.0));
    assert_eq!(
        tool.shank,
        ShankType::Straight(63),
        "shaft 6.35 mm -> 63 tenths"
    );
    // A type without a tip angle clears and ignores it.
    form.set_tool_type(ToolType::EndMillFlat);
    assert!(form.tip_angle.is_empty());
    assert_eq!(form.build(None).expect("valid").tip_angle, None);
}

#[test]
fn test_tool_form_round_trip_keeps_unedited_fields() {
    let backend = ToolsManagerBackend::with_storage_path(scratch("form_rt").join(TOOLS_FILE));
    let standard = backend.get_all_tools()[0].clone();
    let form = ToolForm::from_tool(&standard);
    assert!(
        !form.is_dirty(Some(&standard)),
        "an unchanged form is clean"
    );
    let mut edited = form.clone();
    edited.name.push_str(" (mine)");
    edited.shank = ShankChoice::Tapered;
    assert!(edited.is_dirty(Some(&standard)));
    let tool = edited.build(Some(&standard)).expect("valid");
    assert_eq!(tool.params.rpm, standard.params.rpm, "cutting params kept");
    assert_eq!(tool.shank, ShankType::Tapered);
    assert!(!tool_contents_equal(&tool, &standard));
}

#[test]
fn test_tool_filter_search_type_material_and_diameter() {
    let backend = ToolsManagerBackend::with_storage_path(scratch("filter").join(TOOLS_FILE));
    let all = backend.get_all_tools();
    let mut filter = ToolFilter {
        tool_type: Some(ToolType::VBit),
        ..ToolFilter::default()
    };
    let vbits = filter.apply(all.iter().copied());
    assert!(!vbits.is_empty());
    assert!(vbits.iter().all(|t| t.tool_type == ToolType::VBit));
    filter.tool_type = None;
    filter.material = Some(ToolMaterial::Diamond);
    assert!(
        filter
            .apply(all.iter().copied())
            .iter()
            .all(|t| t.material == ToolMaterial::Diamond)
    );
    filter.material = None;
    filter.dia_min = "3".into();
    filter.dia_max = "6".into();
    let sized = filter.apply(all.iter().copied());
    assert!(sized.iter().all(|t| (3.0..=6.0).contains(&t.diameter)));
    filter.dia_max = "six".into();
    assert!(filter.dia_max_invalid(), "GTK marks a bad bound");
    assert!(
        filter.apply(all.iter().copied()).len() >= sized.len(),
        "and ignores it"
    );
    let ids: Vec<&str> = sized.iter().map(|t| t.id.0.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted, "listed by id");
}

#[test]
fn test_material_form_validation_and_filter() {
    let mut form = MaterialForm::default();
    assert_eq!(form.build(None).err(), Some("Name is required"));
    form.name = "Walnut slab".into();
    assert_eq!(form.build(None).err(), Some("Subcategory is required"));
    form.subcategory = "Hardwood".into();
    form.tensile_strength = "strong".into();
    assert_eq!(
        form.build(None).err(),
        Some("Tensile strength must be a number")
    );
    form.tensile_strength = "100".into();
    assert_eq!(form.build(None).err(), Some("ID is required"));
    form.id = "walnut slab".into();
    assert_eq!(form.build(None).err(), Some("ID must not contain spaces"));
    form.id = "walnut_slab".into();
    let material = form.build(None).expect("valid");
    assert!(material.custom);
    assert_eq!(material.tensile_strength, Some(100.0));

    let backend =
        MaterialsManagerBackend::with_storage_path(scratch("mat_filter").join(MATERIALS_FILE));
    let filter = MaterialFilter {
        search: "oak".into(),
        category: Some(MaterialCategory::Wood),
    };
    let listed = filter.apply(backend.get_all_materials());
    assert!(!listed.is_empty());
    assert!(listed.iter().all(|m| m.category == MaterialCategory::Wood));
    // Editing a standard material keeps what the form does not show.
    let oak = listed[0].clone();
    let edited = MaterialForm::from_material(&oak)
        .build(Some(&oak))
        .expect("valid");
    assert_eq!(edited.cutting_params.len(), oak.cutting_params.len());
    assert_eq!(edited.required_ppe, oak.required_ppe);
}

#[test]
fn test_library_files_are_the_gtk_build_files() {
    let legacy = SettingsManager::config_directory().expect("config folder");
    assert_eq!(
        ToolsManagerBackend::default_storage_path(),
        legacy.join("custom_tools.json")
    );
    assert_eq!(
        MaterialsManagerBackend::default_storage_path(),
        legacy.join("custom_materials.json")
    );
}

#[test]
fn test_unreadable_tool_file_is_backed_up_not_wiped() {
    let dir = scratch("tools_backup");
    let file = dir.join(TOOLS_FILE);
    std::fs::write(&file, "[{ not json").expect("write");
    let mut backend = ToolsManagerBackend::with_storage_path(file.clone());
    assert!(backend.load_error().is_some());
    let mut form = ToolForm {
        id: "new_one".into(),
        name: "New".into(),
        ..ToolForm::default()
    };
    form.notes = "n".into();
    backend
        .save_tool(form.build(None).expect("valid"))
        .expect("save");
    let backup = backend.last_backup().expect("backed up").to_path_buf();
    assert_eq!(
        std::fs::read_to_string(&backup).expect("backup"),
        "[{ not json"
    );
    let reloaded = ToolsManagerBackend::with_storage_path(file);
    assert!(reloaded.get_tool(&ToolId("new_one".into())).is_some());
    // No temporary file is left behind.
    assert!(
        files_in(&dir).iter().all(|n| !n.ends_with(".tmp")),
        "{:?}",
        files_in(&dir)
    );
}

#[test]
fn test_unreadable_material_file_is_backed_up_not_wiped() {
    let dir = scratch("materials_backup");
    let file = dir.join(MATERIALS_FILE);
    std::fs::write(&file, "garbage").expect("write");
    let mut backend = MaterialsManagerBackend::with_storage_path(file.clone());
    assert!(backend.load_error().is_some());
    let form = MaterialForm {
        id: "m1".into(),
        name: "M1".into(),
        subcategory: "S".into(),
        ..MaterialForm::default()
    };
    backend
        .save_material(form.build(None).expect("valid"))
        .expect("save");
    let backup = backend.last_backup().expect("backed up");
    assert_eq!(std::fs::read_to_string(backup).expect("backup"), "garbage");
    assert!(files_in(&dir).iter().all(|n| !n.ends_with(".tmp")));
}

#[test]
fn test_removing_an_override_restores_the_standard_entry() {
    let dir = scratch("override");
    let mut tools = ToolsManagerBackend::with_storage_path(dir.join(TOOLS_FILE));
    let standard = tools.get_all_tools()[0].clone();
    let mut form = ToolForm::from_tool(&standard);
    form.name = "Override".into();
    tools
        .save_tool(form.build(Some(&standard)).expect("valid"))
        .expect("save");
    assert_eq!(
        tools.get_tool(&standard.id).map(|t| t.name.as_str()),
        Some("Override")
    );
    tools.remove_tool(&standard.id).expect("remove");
    let back = tools.get_tool(&standard.id).expect("standard back");
    assert_eq!(back.name, standard.name);
    assert!(!back.custom);

    let mut materials = MaterialsManagerBackend::with_storage_path(dir.join(MATERIALS_FILE));
    let oak = materials.get_all_materials()[0].clone();
    let mut form = MaterialForm::from_material(&oak);
    form.name = "Mine".into();
    materials
        .save_material(form.build(Some(&oak)).expect("valid"))
        .expect("save");
    materials.remove_material(&oak.id).expect("remove");
    assert_eq!(
        materials.get_material(&oak.id).map(|m| m.name.clone()),
        Some(oak.name.clone())
    );
    assert!(materials.get_material(&MaterialId("nope".into())).is_none());
}

#[test]
fn test_reset_custom_tools_removes_file_and_overrides() {
    let dir = scratch("reset");
    let file = dir.join(TOOLS_FILE);
    let mut tools = ToolsManagerBackend::with_storage_path(file.clone());
    let form = ToolForm {
        id: "c1".into(),
        name: "C1".into(),
        ..ToolForm::default()
    };
    tools
        .save_tool(form.build(None).expect("valid"))
        .expect("save");
    assert!(file.exists());
    tools.reset_custom_tools().expect("reset");
    assert!(!file.exists());
    assert!(tools.get_tool(&ToolId("c1".into())).is_none());
    assert!(tools.get_all_tools().iter().all(|t| !t.custom));
}
