//! Auto Reconnect and the WebSocket connection type in the front end: the status bar and the
//! Machine Control pane show "Reconnecting… (n/m)" while a dropped connection is retried, the
//! user can still close it, and the active device profile's network connection and Auto
//! Reconnect setting reach the next connect.

use std::sync::atomic::Ordering;
use std::time::Duration;

use egui_kittest::kittest::Queryable;
use gcodekit6_app::i18n::Translator;
use gcodekit6_core::units::MeasurementSystem;
use gcodekit6_devicedb::DeviceProfile;
use gcodekit6_settings::ConnectionType;
use gcodekit6_ui_egui::machine::{MachineStatus, lock, reconnecting_text};
use gcodekit6_ui_egui::status_bar::summary;

use crate::machine_support::step_until;
use crate::test_machine_pane::pane_app;

#[test]
fn test_reconnect_ui_status_bar_text() {
    let tr = Translator::new("en");
    let status = MachineStatus {
        reconnecting: Some((2, 10)),
        ..MachineStatus::default()
    };
    assert_eq!(
        summary(&status, MeasurementSystem::Metric, &tr).connection,
        "Reconnecting… (2/10)"
    );
    assert_eq!(reconnecting_text(&MachineStatus::default(), &tr), None);
    let de = Translator::new("de");
    assert_eq!(
        reconnecting_text(&status, &de).as_deref(),
        Some("Verbinde erneut… (2/10)")
    );
}

#[test]
fn test_reconnect_ui_shows_state_and_disconnect_cancels() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    shared.link_down.store(true, Ordering::SeqCst);
    assert!(
        step_until(&mut harness, |app| app
            .state()
            .machine
            .status()
            .reconnecting
            .is_some()),
        "the app never showed the reconnect"
    );
    harness.step();
    assert!(!harness.state().state().machine.status().connected);
    let shown = harness
        .query_all_by_label_contains("Reconnecting… (1/")
        .count();
    assert!(
        shown >= 2,
        "status bar and pane both show it, found {shown}"
    );

    // The connection is still the user's to close: Disconnect ends the retries.
    harness.get_by_label("Disconnect").click();
    assert!(
        step_until(&mut harness, |app| app
            .state()
            .machine
            .status()
            .reconnecting
            .is_none()),
        "Disconnect did not end the reconnect"
    );
    shared.link_down.store(false, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(1600));
    harness.step();
    assert_eq!(shared.connects.load(Ordering::SeqCst), 1);
    assert!(!harness.state().state().machine.status().connected);
    let _ = harness.get_by_label("Connect");
}

#[test]
fn test_reconnect_ui_profile_connection_reaches_connect() {
    let (mut harness, _shared) = pane_app(Duration::from_millis(1), false);
    let profile = DeviceProfile {
        connection_type: "WebSocket".to_string(),
        tcp_host: "fluidnc.local".to_string(),
        tcp_port: 81,
        auto_reconnect: false,
        ..DeviceProfile::default()
    };
    harness
        .state_mut()
        .workspace_mut()
        .set_device(Some(&profile));
    harness.step();
    harness.get_by_label("Connect").click();
    assert!(
        step_until(&mut harness, |app| app.state().machine.status().connected),
        "never connected"
    );
    let used = lock(harness.state().state().machine.controller())
        .last_connection()
        .cloned()
        .expect("a connection was started");
    assert_eq!(used.connection_type, ConnectionType::WebSocket);
    assert_eq!(used.port, "fluidnc.local");
    assert_eq!(used.tcp_port, 81);
    assert!(!used.auto_reconnect);
}

#[test]
fn test_reconnect_ui_serial_profile_keeps_configured_connection() {
    let (mut harness, _shared) = pane_app(Duration::from_millis(1), false);
    let network = DeviceProfile {
        connection_type: "TCP/IP".to_string(),
        tcp_host: "10.0.0.7".to_string(),
        tcp_port: 23,
        ..DeviceProfile::default()
    };
    let serial = DeviceProfile {
        auto_reconnect: true,
        ..DeviceProfile::default()
    };
    harness
        .state_mut()
        .workspace_mut()
        .set_device(Some(&network));
    harness
        .state_mut()
        .workspace_mut()
        .set_device(Some(&serial));
    harness.step();
    harness.get_by_label("Connect").click();
    assert!(
        step_until(&mut harness, |app| app.state().machine.status().connected),
        "never connected"
    );
    let used = lock(harness.state().state().machine.controller())
        .last_connection()
        .cloned()
        .expect("a connection was started");
    // The configured (loopback) connection is back; the profile's Auto Reconnect applies.
    assert_eq!(used.connection_type, ConnectionType::Tcp);
    assert_eq!(used.port, "loopback");
    assert!(used.auto_reconnect);
}
