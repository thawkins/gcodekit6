//! The Machine Control pane with a Repetier device profile, against the loopback controller:
//! Connect speaks the profile's protocol, the controls Repetier cannot serve (overrides,
//! continuous jog, probing) are disabled with their reason, E-STOP writes `M112`, and a
//! controller whose firmware differs from the profile's is disconnected with GTK's notice.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::test_support::{LoopbackShared, Wrote};
use gcodekit5_communication::firmware::ControllerProtocol;
use gcodekit5_devicedb::ControllerType;
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::machine::{lock, protocol_mismatch_text};

use crate::machine_support::step_until;
use crate::test_machine_pane::pane_app;

/// Makes `controller` the active profile's and connects through the pane's Connect button.
pub(crate) fn connect_as(harness: &mut Harness<'_, GcodeKitApp>, controller: ControllerType) {
    harness
        .state_mut()
        .workspace_mut()
        .set_controller(Some(&controller));
    harness.step();
    harness
        .get_by_role_and_label(Role::Button, "Connect")
        .click();
    assert!(
        step_until(harness, |app| app.state().machine.status().connected),
        "never connected"
    );
}

/// Whether the control named `label` is enabled.
fn enabled(harness: &Harness<'_, GcodeKitApp>, label: &str) -> bool {
    !harness.get_by_label(label).accesskit_node().is_disabled()
}

/// Asserts that no GRBL real-time byte or `$` command reached the controller.
fn assert_no_grbl_writes(shared: &LoopbackShared) {
    for wrote in shared.log() {
        match wrote {
            Wrote::Realtime(byte) => panic!("GRBL real-time byte 0x{byte:02X} written"),
            Wrote::Line(line) => assert!(!line.contains('$'), "`$` command written: {line}"),
        }
    }
}

#[test]
fn test_repetier_profile_streams_repetier_and_disables_grbl_controls() {
    let (mut harness, shared) = pane_app(std::time::Duration::from_millis(1), false);
    connect_as(&mut harness, ControllerType::Repetier);
    assert!(lock(harness.state().state().machine.controller()).is_repetier());
    assert!(
        step_until(&mut harness, |_| shared
            .log()
            .iter()
            .any(|w| matches!(w, Wrote::Line(l) if l.starts_with("N1 M110 ")))),
        "no numbered M110 at connect: {:?}",
        shared.log()
    );
    for _ in 0..10 {
        harness.step();
    }
    for label in [
        "Increase feed rate by 10%",
        "Decrease feed rate by 1%",
        "Probe Z",
        "Continuous",
    ] {
        assert!(!enabled(&harness, label), "{label} enabled on Repetier");
    }
    for reason in [
        "Overrides need GRBL's real-time commands; Repetier firmware has none.",
        "Probing is not available on Repetier firmware.",
    ] {
        assert!(
            harness.query_by_label(reason).is_some(),
            "no reason: {reason}"
        );
    }
    // The emergency stop is M112, never GRBL's soft reset.
    let estop = harness.get_by_label("E-STOP");
    estop.scroll_to_me();
    harness.step();
    harness.get_by_label("E-STOP").click();
    assert!(
        step_until(&mut harness, |_| shared
            .log()
            .contains(&Wrote::Line("M112".to_string()))),
        "E-STOP did not write M112: {:?}",
        shared.log()
    );
    // The board is killed until it restarts: motion is disabled and the pane says so.
    let reset = "Emergency stop (M112): reset the controller or reconnect to continue.";
    assert!(
        step_until(&mut harness, |app| app.state().machine.status().estopped),
        "E-STOP not recorded"
    );
    harness.get_by_label("Unlock").scroll_to_me();
    for _ in 0..4 {
        harness.step();
    }
    assert!(harness.query_by_label(reset).is_some(), "no reset notice");
    for label in ["Home", "Jog X+", "Zero X", "Send"] {
        assert!(!enabled(&harness, label), "{label} enabled after M112");
    }
    // The firmware restarted: motion is possible again.
    shared.inject("start\n");
    assert!(
        step_until(&mut harness, |app| !app.state().machine.status().estopped),
        "restart did not clear the E-STOP"
    );
    assert_no_grbl_writes(&shared);
}

#[test]
fn test_grbl_profile_still_allows_grbl_controls() {
    let (mut harness, _shared) = pane_app(std::time::Duration::from_millis(1), false);
    connect_as(&mut harness, ControllerType::Grbl);
    for _ in 0..10 {
        harness.step();
    }
    assert!(!lock(harness.state().state().machine.controller()).is_repetier());
    assert!(enabled(&harness, "Increase feed rate by 10%"));
    assert!(enabled(&harness, "Continuous"));
}

/// Connects with the profile `controller`, lets the controller say `banner`, and checks the
/// connection is closed with GTK's notice naming both protocols.
fn assert_rejected(
    controller: ControllerType,
    banner: &str,
    configured: ControllerProtocol,
    detected: ControllerProtocol,
) {
    let (mut harness, shared) = pane_app(std::time::Duration::from_millis(1), false);
    connect_as(&mut harness, controller);
    shared.inject(banner);
    assert!(
        step_until(&mut harness, |app| !app.state().machine.status().connected),
        "the mismatch did not disconnect"
    );
    let notice = protocol_mismatch_text(configured, detected, &Translator::new("en"));
    assert!(
        notice.contains(configured.as_str()) && notice.contains(detected.as_str()),
        "{notice}"
    );
    harness.get_by_label("Unlock").scroll_to_me();
    for _ in 0..4 {
        harness.step();
    }
    // In the pane and in the console log.
    let shown = harness
        .query_all(egui_kittest::kittest::By::new().label(&notice))
        .count();
    assert!(shown >= 2, "notice shown {shown} times: {notice}");
    if configured == ControllerProtocol::Repetier {
        assert_no_grbl_writes(&shared);
    }
}

#[test]
fn test_grbl_profile_rejects_repetier_firmware() {
    assert_rejected(
        ControllerType::Grbl,
        "FIRMWARE_NAME:Repetier_1.0.6 FIRMWARE_URL:https://github.com/repetier\n",
        ControllerProtocol::Grbl,
        ControllerProtocol::Repetier,
    );
}

#[test]
fn test_repetier_profile_rejects_grbl_firmware() {
    assert_rejected(
        ControllerType::Repetier,
        "Grbl 1.1h ['$' for help]\n",
        ControllerProtocol::Repetier,
        ControllerProtocol::Grbl,
    );
}
