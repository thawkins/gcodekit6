//! The Device Info panel of the Device Config page: what the controller reports about
//! itself (firmware, version, build date, compile options, homing and laser mode), the
//! device type selector writing `$32`, and Refresh asking the controller again.

use std::time::Duration;

use egui_kittest::kittest::Queryable;
use gcodekit5_app::device_identity::{DeviceIdentity, DeviceType};
use gcodekit5_app::test_support::Wrote;
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::commands::Command;
use gcodekit5_ui_egui::workspace::Page;

use crate::machine_support::{app_harness, connect, loopback_state, step_until, wait_until};

#[test]
fn test_device_info_parses_version_build_and_options() {
    let mut id = DeviceIdentity::default();
    assert!(id.apply_line("Grbl 1.1h ['$' for help]"));
    assert!(id.apply_line("[VER:1.1h.20190825:Shop mill]"));
    assert!(id.apply_line("[OPT:VNMZL,15,128]"));
    assert!(id.apply_line("$32=1"));
    assert!(!id.apply_line("ok"));
    assert_eq!(id.firmware.as_deref(), Some("Grbl"));
    assert_eq!(id.version.as_deref(), Some("1.1h"));
    assert_eq!(id.build.as_deref(), Some("20190825"));
    assert_eq!(id.name.as_deref(), Some("Shop mill"));
    assert_eq!(id.options.as_deref(), Some("VNMZL,15,128"));
    assert_eq!(id.laser, Some(true));
    assert_eq!(id.homing, None);

    let mut repetier = DeviceIdentity::default();
    repetier.apply_line("FIRMWARE_NAME:Repetier_1.0.6 MACHINE_TYPE:Mendel EXTRUDER_COUNT:1");
    assert!(repetier.is_repetier());
    assert_eq!(repetier.version.as_deref(), Some("1.0.6"));
    assert_eq!(repetier.name.as_deref(), Some("Mendel"));
}

#[test]
fn test_device_info_type_follows_laser_mode() {
    use DeviceType::{Cnc, Laser, Other};
    assert_eq!(
        DeviceType::follow_laser_mode(Other, false, true),
        Some(Laser)
    );
    assert_eq!(
        DeviceType::follow_laser_mode(Other, false, false),
        Some(Cnc)
    );
    assert_eq!(DeviceType::follow_laser_mode(Other, true, false), None);
    assert_eq!(DeviceType::follow_laser_mode(Laser, true, false), Some(Cnc));
    assert_eq!(DeviceType::follow_laser_mode(Laser, true, true), None);
}

#[test]
fn test_device_info_shows_reported_firmware_build_and_options() {
    let (mut state, shared) = loopback_state(Duration::from_millis(1));
    connect(&mut state);
    let mut h = app_harness(GcodeKitApp::from_state(state));
    h.state_mut().dispatch(Command::OpenDeviceInfo);
    h.run_steps(2);
    // Machine > Device Info shows the Device Config page, whose left panel it is (GTK).
    assert_eq!(h.state().page(), Page::DeviceConfig);
    assert!(h.query_by_label("Device Type").is_some(), "panel not shown");

    shared.inject("Grbl 1.1h ['$' for help]\n[VER:1.1h.20190825:]\n[OPT:VNMZL,15,128]\n$22=1\n");
    assert!(
        step_until(&mut h, |app| app.device_info().identity().options.is_some()
            && app.device_info().identity().homing.is_some()),
        "the report never arrived"
    );
    h.run_steps(2);
    for shown in ["Grbl", "1.1h", "20190825", "VNMZL,15,128"] {
        assert!(h.query_by_label(shown).is_some(), "{shown} not shown");
    }
    h.get_by_label("Capabilities").click();
    h.run_steps(2);
    assert!(h.query_by_label("Enabled ($22=1)").is_some());
}

#[test]
fn test_device_info_laser_type_writes_32_and_refresh_asks_again() {
    let (mut state, shared) = loopback_state(Duration::from_millis(1));
    connect(&mut state);
    let mut h = app_harness(GcodeKitApp::from_state(state));
    h.state_mut().dispatch(Command::OpenDeviceInfo);
    h.run_steps(2);

    h.get_by_label("Laser").click();
    h.run_steps(2);
    assert!(
        wait_until(|| shared.log().contains(&Wrote::Line("$32=1".to_string()))),
        "$32=1 not written: {:?}",
        shared.log()
    );
    shared.inject("$32=1\n");
    assert!(step_until(&mut h, |app| app.device_info().device_type()
        == DeviceType::Laser
        && app.device_info().identity().laser == Some(true)));

    let before = shared.log().len();
    // The lines written since `before`: the status poll's realtime `?` can land among them
    // under load and is not part of the refresh.
    let lines = || -> Vec<Wrote> {
        shared.log()[before..]
            .iter()
            .filter(|w| matches!(w, Wrote::Line(_)))
            .cloned()
            .collect()
    };
    h.get_by_label("Refresh device info").click();
    h.run_steps(2);
    assert!(wait_until(|| lines().len() >= 2));
    assert_eq!(
        lines()[..2],
        [Wrote::Line("$I".to_string()), Wrote::Line("$$".to_string())]
    );
}

#[test]
fn test_device_info_refuses_laser_type_while_disconnected() {
    let (state, shared) = loopback_state(Duration::from_millis(1));
    let mut h = app_harness(GcodeKitApp::from_state(state));
    h.state_mut().dispatch(Command::OpenDeviceInfo);
    h.run_steps(2);
    h.get_by_label("Laser").click();
    h.run_steps(2);
    assert!(shared.log().is_empty());
    assert_eq!(h.state().device_info().device_type(), DeviceType::Other);
    assert!(h.query_by_label("No device").is_some());
}
