//! The Device Manager window: the GTK build's `devices.json` loads and saves back unchanged,
//! profiles are added, edited, deleted and made active, the active profile cannot be deleted,
//! and the Firmware Settings window reads Repetier settings when the active profile says
//! Repetier and stores a retrieved GRBL dump in the active profile.

use std::path::{Path, PathBuf};
use std::time::Duration;

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit6_app::test_support::Wrote;
use gcodekit6_devicedb::{ControllerType, DeviceManager, DeviceProfile};
use gcodekit6_settings::SettingsManager;
use gcodekit6_settings::config::Config;
use gcodekit6_ui_egui::commands::Command;
use gcodekit6_ui_egui::workspace::Page;
use gcodekit6_ui_egui::{AppState, GcodeKitApp};

use crate::machine_support::{
    app_harness, connect, loopback_state, start_held_stream, step_until, wait_until,
};

/// A profiles file as the GTK build's `DeviceManager::save` writes it: two profiles, the
/// second active, the first with a stored `$$` dump.
pub(crate) const GTK_DEVICES: &str = r#"{
  "active_id": "b-laser",
  "profiles": [
    {
      "id": "a-router",
      "name": "Router 4040",
      "description": "Shop router",
      "device_type": "CncMill",
      "controller_type": "GrblHal",
      "x_axis": { "min": 0.0, "max": 400.0, "enabled": true },
      "y_axis": { "min": 0.0, "max": 400.0, "enabled": true },
      "z_axis": { "min": -80.0, "max": 0.0, "enabled": true },
      "a_axis": { "min": 0.0, "max": 360.0, "enabled": false },
      "b_axis": { "min": 0.0, "max": 360.0, "enabled": false },
      "c_axis": { "min": 0.0, "max": 360.0, "enabled": false },
      "num_axes": 3,
      "has_spindle": true,
      "has_laser": false,
      "has_coolant": true,
      "max_feed_rate": 3000.0,
      "max_s_value": 24000.0,
      "max_spindle_speed_rpm": 24000,
      "cnc_spindle_watts": 800.0,
      "laser_watts": 0.0,
      "connection_type": "Serial",
      "baud_rate": 115200,
      "port": "/dev/ttyUSB0",
      "tcp_host": "192.168.1.100",
      "tcp_port": 23,
      "timeout_ms": 5000,
      "auto_reconnect": false,
      "pen_mode": false,
      "grbl_settings": { "130": "400.000", "110": "3000.000" }
    },
    {
      "id": "b-laser",
      "name": "Diode Laser",
      "description": "",
      "device_type": "LaserCutter",
      "controller_type": "Grbl",
      "x_axis": { "min": 0.0, "max": 300.0, "enabled": true },
      "y_axis": { "min": 0.0, "max": 200.0, "enabled": true },
      "z_axis": { "min": 0.0, "max": 50.0, "enabled": true },
      "a_axis": { "min": 0.0, "max": 360.0, "enabled": false },
      "b_axis": { "min": 0.0, "max": 360.0, "enabled": false },
      "c_axis": { "min": 0.0, "max": 360.0, "enabled": false },
      "num_axes": 3,
      "has_spindle": false,
      "has_laser": true,
      "has_coolant": false,
      "max_feed_rate": 6000.0,
      "max_s_value": 1000.0,
      "max_spindle_speed_rpm": 0,
      "cnc_spindle_watts": 0.0,
      "laser_watts": 10.0,
      "connection_type": "TCP/IP",
      "baud_rate": 115200,
      "port": "Auto",
      "tcp_host": "laser.local",
      "tcp_port": 8080,
      "timeout_ms": 3000,
      "auto_reconnect": true,
      "pen_mode": false,
      "grbl_settings": {}
    }
  ]
}"#;

/// A fresh scratch folder `target/temp/egui_device_manager/<name>-<pid>`.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/egui_device_manager")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

/// The profiles file in a fresh scratch folder, holding `content` when given.
fn devices_file(name: &str, content: Option<&str>) -> PathBuf {
    let file = scratch(name).join("devices.json");
    if let Some(content) = content {
        std::fs::write(&file, content).expect("write devices.json");
    }
    file
}

/// The app, without a machine connection, keeping its profiles in `file`.
fn app_with(file: &Path) -> Harness<'static, GcodeKitApp> {
    let state = AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    )
    .with_device_file(file.to_path_buf());
    open_manager(GcodeKitApp::from_state(state))
}

fn open_manager(app: GcodeKitApp) -> Harness<'static, GcodeKitApp> {
    let mut h = app_harness(app);
    h.state_mut().dispatch(Command::OpenDeviceManager);
    h.run_steps(3);
    assert_eq!(h.state().page(), Page::DeviceManager, "page not shown");
    h
}

/// The profiles in `file`, read the way the GTK build reads them.
fn stored(file: &Path) -> (Vec<DeviceProfile>, Option<String>) {
    let manager = DeviceManager::new(file.to_path_buf());
    manager.load().expect("devices.json loads");
    let mut profiles = manager.get_all_profiles();
    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    (profiles, manager.get_active_profile().map(|p| p.id))
}

/// Replaces the text of the text field labelled `label` with `text`.
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
        .device_manager()
        .notice()
        .unwrap_or_default()
        .to_string()
}

#[test]
fn test_device_manager_gtk_file_loads_and_round_trips() {
    let file = devices_file("round_trip", Some(GTK_DEVICES));
    let before: serde_json::Value = serde_json::from_str(GTK_DEVICES).expect("fixture");
    let mut h = app_with(&file);
    for shown in ["Router 4040", "Diode Laser", "Active"] {
        assert!(h.query_by_label(shown).is_some(), "{shown} not listed");
    }
    // Save both profiles unchanged through the form.
    for name in ["Router 4040", "Diode Laser"] {
        click(&mut h, name);
        click(&mut h, "Save");
        assert!(
            notice(&h).contains("Device saved"),
            "refused: {}",
            notice(&h)
        );
    }
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).expect("read")).expect("json");
    assert_eq!(after["active_id"], before["active_id"]);
    let by_id = |v: &serde_json::Value, id: &str| {
        v["profiles"]
            .as_array()
            .and_then(|a| a.iter().find(|p| p["id"] == id).cloned())
    };
    for id in ["a-router", "b-laser"] {
        assert_eq!(by_id(&after, id), by_id(&before, id), "{id} changed");
    }
}

#[test]
fn test_device_manager_add_edit_delete() {
    let file = devices_file("crud", None);
    let mut h = app_with(&file);
    // Without a file the GTK build's default profile is created and made active.
    let (profiles, active) = stored(&file);
    assert_eq!(profiles.len(), 1);
    assert_eq!(active.as_deref(), Some(profiles[0].id.as_str()));

    click(&mut h, "Add Device");
    assert_eq!(stored(&file).0.len(), 2);
    set_field(&mut h, "Name:", "Laser 2");
    click(&mut h, "Save");
    let (profiles, _) = stored(&file);
    assert!(profiles.iter().any(|p| p.name == "Laser 2"), "{profiles:?}");

    click(&mut h, "Laser 2");
    click(&mut h, "Capabilities");
    set_field(&mut h, "No of Axes:", "9");
    click(&mut h, "Save");
    assert!(notice(&h).contains("Invalid No of Axes"), "{}", notice(&h));
    assert!(stored(&file).0.iter().all(|p| p.num_axes == 3));
    click(&mut h, "Cancel");

    click(&mut h, "Laser 2");
    click(&mut h, "Delete");
    assert!(h.query_by_label("Delete device?").is_some(), "not asked");
    click(&mut h, "Yes");
    let (profiles, active) = stored(&file);
    assert_eq!(profiles.len(), 1);
    assert!(active.is_some());
}

#[test]
fn test_device_manager_refuses_deleting_the_active_profile() {
    let file = devices_file("delete_active", Some(GTK_DEVICES));
    let mut h = app_with(&file);
    click(&mut h, "Diode Laser");
    click(&mut h, "Delete");
    assert!(h.query_by_label("Delete device?").is_none());
    assert!(notice(&h).contains("active device"), "{}", notice(&h));
    assert_eq!(stored(&file).0.len(), 2);
}

#[test]
fn test_device_manager_set_active() {
    let file = devices_file("set_active", Some(GTK_DEVICES));
    let mut h = app_with(&file);
    click(&mut h, "Router 4040");
    click(&mut h, "Set Active");
    assert_eq!(stored(&file).1.as_deref(), Some("a-router"));
    assert_eq!(
        h.state().active_device().map(|p| p.name),
        Some("Router 4040".to_string())
    );
}

#[test]
fn test_device_manager_refuses_changing_the_active_profile_during_a_job() {
    let file = devices_file("busy", Some(GTK_DEVICES));
    let (mut state, shared) = loopback_state(Duration::from_millis(1));
    start_held_stream(&mut state, &shared, 20);
    let mut h = open_manager(GcodeKitApp::from_state(
        state.with_device_file(file.clone()),
    ));
    click(&mut h, "Router 4040");
    click(&mut h, "Set Active");
    assert!(notice(&h).contains("a job is running"), "{}", notice(&h));
    assert_eq!(stored(&file).1.as_deref(), Some("b-laser"));
    shared
        .hold_acks
        .store(false, std::sync::atomic::Ordering::SeqCst);
}

/// A connected app whose active profile uses `controller`, with Firmware Settings open.
fn firmware_with_profile(
    name: &str,
    controller: ControllerType,
) -> (
    Harness<'static, GcodeKitApp>,
    std::sync::Arc<gcodekit6_app::test_support::LoopbackShared>,
    PathBuf,
) {
    let profile = DeviceProfile {
        name: "Bench Mill".to_string(),
        controller_type: controller,
        ..DeviceProfile::default()
    };
    let active = profile.id.clone();
    firmware_with_profiles(name, vec![profile], &active)
}

/// A connected app (loopback host `loopback:23`) over `profiles` with `active` active, with
/// Firmware Settings open.
fn firmware_with_profiles(
    name: &str,
    profiles: Vec<DeviceProfile>,
    active: &str,
) -> (
    Harness<'static, GcodeKitApp>,
    std::sync::Arc<gcodekit6_app::test_support::LoopbackShared>,
    PathBuf,
) {
    let file = devices_file(name, None);
    let manager = DeviceManager::new(file.clone());
    for profile in profiles {
        manager.save_profile(profile).expect("save");
    }
    manager.set_active_profile(active).expect("active");
    let (mut state, shared) = loopback_state(Duration::from_millis(1));
    connect(&mut state);
    let mut h = app_harness(GcodeKitApp::from_state(
        state.with_device_file(file.clone()),
    ));
    h.state_mut().dispatch(Command::OpenFirmwareSettings);
    h.run_steps(3);
    (h, shared, file)
}

#[test]
fn test_device_manager_repetier_profile_selects_repetier_retrieve() {
    let (mut h, shared, _file) = firmware_with_profile("repetier", ControllerType::Repetier);
    click(&mut h, "Retrieve");
    assert!(
        wait_until(|| shared.log().contains(&Wrote::Line("M115".to_string()))),
        "written: {:?}",
        shared.log()
    );
    let log = shared.log();
    assert!(log.contains(&Wrote::Line("M205".to_string())));
    assert!(!log.contains(&Wrote::Line("$$".to_string())));
}

#[test]
fn test_device_manager_protocol_mismatch_hints_at_the_profile() {
    let (mut h, shared, _file) = firmware_with_profile("mismatch", ControllerType::Grbl);
    click(&mut h, "Retrieve");
    assert!(wait_until(|| shared
        .log()
        .contains(&Wrote::Line("$I".to_string()))));
    shared.inject("[VER:1.0.6:]\n");
    assert!(step_until(&mut h, |app| app
        .firmware_settings()
        .notice()
        .is_some_and(|n| n.contains("Protocol mismatch"))));
    let notice = h.state().firmware_settings().notice().unwrap_or_default();
    assert!(notice.contains("Device Manager"), "{notice}");
}

#[test]
fn test_device_manager_retrieve_stores_settings_and_syncs_the_form() {
    let (mut h, shared, file) = firmware_with_profile("store", ControllerType::Grbl);
    click(&mut h, "Retrieve");
    assert!(wait_until(|| shared
        .log()
        .contains(&Wrote::Line("$I".to_string()))));
    let dump: String = [
        "$0=10",
        "$1=25",
        "$2=0",
        "$3=0",
        "$4=0",
        "$5=0",
        "$6=0",
        "$30=1000",
        "$32=1",
        "$110=5000.000",
        "$130=410.000",
        "$131=290.000",
        "$132=60.000",
    ]
    .iter()
    .map(|l| format!("{l}\n"))
    .collect();
    shared.inject(&format!("{dump}[VER:1.1h.20190825:]\n"));
    assert!(step_until(&mut h, |app| app
        .firmware_settings()
        .notice()
        .is_some_and(|n| n.contains("Settings retrieved"))));
    h.run_steps(2);
    let (profiles, _) = stored(&file);
    assert_eq!(profiles[0].grbl_settings.len(), 13, "{:?}", profiles[0]);
    assert_eq!(profiles[0].max_s_value, 1000.0);
    assert!(profiles[0].has_laser);

    // Sync from Device fills the form from the same dump; Save keeps it.
    h.state_mut().dispatch(Command::OpenDeviceManager);
    h.run_steps(3);
    click(&mut h, "Bench Mill");
    click(&mut h, "Sync from Device");
    click(&mut h, "Save");
    let (profiles, _) = stored(&file);
    assert_eq!(profiles[0].x_axis.max, 410.0);
    assert_eq!(profiles[0].z_axis.max, 60.0);
    assert_eq!(profiles[0].max_spindle_speed_rpm, 1000);
}

#[test]
fn test_device_manager_unreadable_file_is_backed_up_and_named() {
    let file = devices_file("unreadable", Some("{ not json"));
    let h = app_with(&file);
    let text = notice(&h);
    assert!(
        text.contains("could not be read") && text.contains(".bak-"),
        "{text}"
    );
    let backups: Vec<_> = std::fs::read_dir(file.parent().expect("dir"))
        .expect("dir")
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().contains(".bak-"))
        .collect();
    assert_eq!(backups.len(), 1, "one backup");
    assert_eq!(
        std::fs::read_to_string(backups[0].path()).expect("read"),
        "{ not json"
    );
    // A valid active profile replaces the unreadable file.
    assert!(h.state().active_device().is_some());
}

/// The app over `file` without opening any window.
fn app_only(file: &Path) -> GcodeKitApp {
    let state = AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    )
    .with_device_file(file.to_path_buf());
    GcodeKitApp::from_state(state)
}

#[test]
fn test_device_manager_startup_without_a_file_creates_the_active_default() {
    let file = devices_file("startup_none", None);
    let app = app_only(&file);
    assert!(file.exists(), "devices.json not created at startup");
    let (profiles, active) = stored(&file);
    assert_eq!(profiles.len(), 1);
    assert_eq!(active.as_deref(), Some(profiles[0].id.as_str()));
    assert_eq!(app.active_device().map(|p| p.id), active);
}

#[test]
fn test_device_manager_startup_repairs_a_missing_or_dangling_active_id() {
    for (name, active) in [("null_active", "null"), ("dangling_active", "\"gone\"")] {
        let content = GTK_DEVICES.replace(
            "\"active_id\": \"b-laser\"",
            &format!("\"active_id\": {active}"),
        );
        let file = devices_file(name, Some(&content));
        let app = app_only(&file);
        // The first profile by name becomes active, in memory and on disk.
        assert_eq!(
            app.active_device().map(|p| p.name).as_deref(),
            Some("Diode Laser")
        );
        assert_eq!(stored(&file).1.as_deref(), Some("b-laser"));
    }
}

#[test]
fn test_device_manager_save_of_active_profile_refused_during_a_job() {
    let file = devices_file("busy_save", Some(GTK_DEVICES));
    let (mut state, shared) = loopback_state(Duration::from_millis(1));
    start_held_stream(&mut state, &shared, 20);
    let mut h = open_manager(GcodeKitApp::from_state(
        state.with_device_file(file.clone()),
    ));
    click(&mut h, "Diode Laser");
    set_field(&mut h, "Name:", "Renamed");
    click(&mut h, "Save");
    assert!(notice(&h).contains("a job is running"), "{}", notice(&h));
    assert!(stored(&file).0.iter().all(|p| p.name != "Renamed"));
    // A profile that is not active can still be saved.
    click(&mut h, "Router 4040");
    set_field(&mut h, "Name:", "Router Renamed");
    click(&mut h, "Save");
    assert!(stored(&file).0.iter().any(|p| p.name == "Router Renamed"));
    shared
        .hold_acks
        .store(false, std::sync::atomic::Ordering::SeqCst);
}

/// A GRBL dump of 13 settings, then the identity reply that ends it.
fn grbl_dump() -> String {
    let dump: String = [
        "$0=10",
        "$1=25",
        "$2=0",
        "$3=0",
        "$4=0",
        "$5=0",
        "$6=0",
        "$30=1000",
        "$32=1",
        "$110=5000.000",
        "$130=410.000",
        "$131=290.000",
        "$132=60.000",
    ]
    .iter()
    .map(|l| format!("{l}\n"))
    .collect();
    format!("{dump}[VER:1.1h.20190825:]\n")
}

/// Shows the Device Config page, clicks Retrieve and answers it with [`grbl_dump`].
fn retrieve_dump(
    h: &mut Harness<'_, GcodeKitApp>,
    shared: &gcodekit6_app::test_support::LoopbackShared,
) {
    h.state_mut().dispatch(Command::OpenFirmwareSettings);
    h.run_steps(2);
    click(h, "Retrieve");
    assert!(wait_until(|| shared
        .log()
        .contains(&Wrote::Line("$I".to_string()))));
    shared.inject(&grbl_dump());
}

#[test]
fn test_device_manager_dump_goes_to_the_profile_on_the_connected_port() {
    // The active profile is on another port; the loopback host matches "Laser".
    let router = DeviceProfile {
        name: "Router".to_string(),
        port: "/dev/ttyUSB0".to_string(),
        ..DeviceProfile::default()
    };
    let laser = DeviceProfile {
        name: "Laser".to_string(),
        port: "/dev/ttyACM0".to_string(),
        tcp_host: "loopback".to_string(),
        tcp_port: 23,
        ..DeviceProfile::default()
    };
    let active = router.id.clone();
    let (mut h, shared, file) = firmware_with_profiles("port_match", vec![router, laser], &active);
    retrieve_dump(&mut h, &shared);
    assert!(step_until(&mut h, |app| app
        .firmware_settings()
        .notice()
        .is_some_and(|n| n.contains("Laser"))));
    let (profiles, _) = stored(&file);
    let by_name = |n: &str| profiles.iter().find(|p| p.name == n).cloned().expect(n);
    assert_eq!(by_name("Laser").grbl_settings.len(), 13);
    assert!(
        by_name("Router").grbl_settings.is_empty(),
        "the router was overwritten"
    );
    assert!(!by_name("Router").has_laser);
}

#[test]
fn test_device_manager_dump_without_a_matching_profile_is_reported() {
    let router = DeviceProfile {
        name: "Router".to_string(),
        port: "/dev/ttyUSB0".to_string(),
        ..DeviceProfile::default()
    };
    let active = router.id.clone();
    let (mut h, shared, file) = firmware_with_profiles("port_none", vec![router], &active);
    retrieve_dump(&mut h, &shared);
    assert!(step_until(&mut h, |app| app
        .firmware_settings()
        .notice()
        .is_some_and(|n| n.contains("not stored"))));
    assert!(stored(&file).0[0].grbl_settings.is_empty());
}

#[test]
fn test_device_manager_dump_completed_before_a_disconnect_is_stored() {
    let (mut h, shared, file) = firmware_with_profile("disconnect", ControllerType::Grbl);
    click(&mut h, "Retrieve");
    assert!(wait_until(|| shared
        .log()
        .contains(&Wrote::Line("$I".to_string()))));
    shared.inject(&grbl_dump());
    // Let the worker read the reply, then disconnect before the app sees any of it.
    std::thread::sleep(Duration::from_millis(300));
    gcodekit6_ui_egui::machine::lock(h.state().state().machine.controller()).disconnect();
    h.run_steps(3);
    assert_eq!(stored(&file).0[0].grbl_settings.len(), 13);
}

#[test]
fn test_device_manager_stored_dump_refreshes_the_open_form() {
    let (mut h, shared, file) = firmware_with_profile("stale_form", ControllerType::Grbl);
    h.state_mut().dispatch(Command::OpenDeviceManager);
    h.run_steps(3);
    click(&mut h, "Bench Mill");
    retrieve_dump(&mut h, &shared);
    assert!(step_until(&mut h, |app| app
        .firmware_settings()
        .notice()
        .is_some_and(|n| n.contains("Settings retrieved"))));
    h.run_steps(2);
    h.state_mut().dispatch(Command::OpenDeviceManager);
    h.run_steps(2);
    click(&mut h, "Save");
    let (profiles, _) = stored(&file);
    assert_eq!(profiles[0].max_s_value, 1000.0);
    assert!(profiles[0].has_laser);
    assert_eq!(profiles[0].grbl_settings.len(), 13);
}

#[test]
fn test_device_manager_quit_asks_and_saves_an_unsaved_profile() {
    let file = devices_file("quit_save", Some(GTK_DEVICES));
    let mut h = app_with(&file);
    click(&mut h, "Router 4040");
    set_field(&mut h, "Name:", "Quit Saved Router");
    let shown = crate::test_tools_manager::quit_answering(
        &mut h,
        craft_dialogs::MessageResult::Custom("Save".into()),
    );
    assert_eq!(shown, ["Unsaved changes"]);
    assert!(
        stored(&file)
            .0
            .iter()
            .any(|p| p.name == "Quit Saved Router"),
        "{:?}",
        stored(&file).0
    );
    assert!(h.state().quit_confirmed());
}

#[test]
fn test_device_manager_quit_cancel_keeps_the_edit_and_discard_quits() {
    let file = devices_file("quit_cancel", Some(GTK_DEVICES));
    let mut h = app_with(&file);
    click(&mut h, "Router 4040");
    set_field(&mut h, "Name:", "Unsaved Router");
    // Asked about from another page; Cancel brings the Device Manager page back.
    h.get_by_role_and_label(Role::Tab, "Visualizer").click();
    h.run_steps(2);
    let shown =
        crate::test_tools_manager::quit_answering(&mut h, craft_dialogs::MessageResult::Cancel);
    assert_eq!(shown, ["Unsaved changes"]);
    assert!(!h.state().quit_confirmed());
    assert_eq!(h.state().page(), Page::DeviceManager);
    assert!(h.state().device_manager().has_unsaved_changes());
    assert!(stored(&file).0.iter().all(|p| p.name != "Unsaved Router"));

    let shown = crate::test_tools_manager::quit_answering(
        &mut h,
        craft_dialogs::MessageResult::Custom("Discard".into()),
    );
    assert_eq!(shown, ["Unsaved changes"]);
    assert!(h.state().quit_confirmed());
    assert!(stored(&file).0.iter().all(|p| p.name != "Unsaved Router"));
}

#[test]
fn test_device_manager_unchanged_form_is_not_asked_about() {
    let file = devices_file("quit_unchanged", Some(GTK_DEVICES));
    let mut h = app_with(&file);
    click(&mut h, "Router 4040");
    assert!(!h.state().device_manager().has_unsaved_changes());
    let shown =
        crate::test_tools_manager::quit_answering(&mut h, craft_dialogs::MessageResult::Cancel);
    assert!(shown.is_empty(), "{shown:?}");
    assert!(h.state().quit_confirmed());
}

/// Review Focus 1: a quit asked from Machine Control whose unsaved device profile prompt is
/// cancelled shows the Device Manager page with no pointer press. That page change ends a
/// held keyboard jog at once, and the jog keys no longer reach the hidden pane.
#[test]
fn test_device_manager_quit_cancel_ends_keyboard_jog() {
    use crate::test_machine_keyboard_jog::{cancels, jogs, key, settled_jogs, tap};
    let file = devices_file("quit_cancel_jog", Some(GTK_DEVICES));
    let (state, shared) = crate::machine_support::loopback_state(Duration::from_millis(1));
    let mut state = state.with_device_file(file.clone());
    crate::machine_support::connect(&mut state);
    let mut h = open_manager(GcodeKitApp::from_state(state));
    click(&mut h, "Router 4040");
    set_field(&mut h, "Name:", "Unsaved Router");
    h.get_by_role_and_label(Role::Tab, "Machine Control")
        .click();
    h.run_steps(2);
    crate::test_machine_pane::report_state(
        &mut h,
        &shared,
        "<Idle|MPos:-100.000,-100.000,-50.000|FS:0,0>",
        "Idle",
    );
    h.get_by_label("Continuous").click();
    h.step();
    h.get_by_label("Machine State").click();
    h.run_steps(2);
    key(&h, egui::Key::Num6, true);
    h.run_steps(2);
    assert!(
        wait_until(|| jogs(&shared).len() == 1),
        "{:?}",
        shared.log()
    );

    let shown =
        crate::test_tools_manager::quit_answering(&mut h, craft_dialogs::MessageResult::Cancel);
    assert_eq!(shown, ["Unsaved changes"]);
    assert_eq!(h.state().page(), Page::DeviceManager);
    assert!(wait_until(|| cancels(&shared) == 1), "{:?}", shared.log());

    key(&h, egui::Key::Num6, false);
    h.step();
    tap(&mut h, egui::Key::Num6);
    tap(&mut h, egui::Key::Num8);
    assert_eq!(settled_jogs(&h, &shared).len(), 1, "{:?}", shared.log());
    assert_eq!(cancels(&shared), 1);
}
