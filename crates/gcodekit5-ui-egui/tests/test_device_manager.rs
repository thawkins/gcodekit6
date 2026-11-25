//! The Device Manager window: the GTK build's `devices.json` loads and saves back unchanged,
//! profiles are added, edited, deleted and made active, the active profile cannot be deleted,
//! and the Firmware Settings window reads Repetier settings when the active profile says
//! Repetier and stores a retrieved GRBL dump in the active profile.

use std::path::{Path, PathBuf};
use std::time::Duration;

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_app::test_support::Wrote;
use gcodekit5_devicedb::{ControllerType, DeviceManager, DeviceProfile};
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::commands::Command;
use gcodekit5_ui_egui::{AppState, GcodeKitApp};

use crate::machine_support::{
    app_harness, connect, loopback_state, start_held_stream, step_until, wait_until,
};

/// A profiles file as the GTK build's `DeviceManager::save` writes it: two profiles, the
/// second active, the first with a stored `$$` dump.
const GTK_DEVICES: &str = r#"{
  "active_id": "b-laser",
  "profiles": [
    {
      "id": "a-router",
      "name": "Router 4040",
      "description": "Shop router",
      "device_type": "CncMill",
      "controller_type": "Grbl",
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
    assert!(
        h.query_by_label("Device Manager").is_some(),
        "window not open"
    );
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
        assert!(!notice(&h).contains("Invalid"), "refused: {}", notice(&h));
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
    std::sync::Arc<gcodekit5_app::test_support::LoopbackShared>,
    PathBuf,
) {
    let file = devices_file(name, None);
    let manager = DeviceManager::new(file.clone());
    let profile = DeviceProfile {
        name: "Bench Mill".to_string(),
        controller_type: controller,
        ..DeviceProfile::default()
    };
    manager.save_profile(profile.clone()).expect("save");
    manager.set_active_profile(&profile.id).expect("active");
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
    h.state_mut().firmware_settings_mut().close();
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
fn test_device_manager_unreadable_file_is_reported_and_kept() {
    let file = devices_file("unreadable", Some("{ not json"));
    let h = app_with(&file);
    assert!(notice(&h).contains("could not be read"), "{}", notice(&h));
    assert_eq!(
        std::fs::read_to_string(&file).expect("read"),
        "{ not json",
        "the file was replaced"
    );
}
