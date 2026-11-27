//! The Firmware Settings window: the list filter (replaces the GTK crate's
//! `test_config_setting_filter.rs`), the values the controller reports, writing a setting
//! (exactly the `$n=v` line, through the shared controller), and refusing writes while a
//! job runs or while disconnected.

use std::sync::atomic::Ordering;
use std::time::Duration;

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit6_app::firmware_settings::ConfigSettingRow;
use gcodekit6_app::machine::MachineEvent;
use gcodekit6_app::test_support::{LoopbackShared, Wrote};
use gcodekit6_ui_egui::GcodeKitApp;
use gcodekit6_ui_egui::commands::Command;

use crate::machine_support::{
    app_harness, connect, loopback_state, start_held_stream, step_until, wait_until,
};

fn row(number: u16, name: &str, category: &str) -> ConfigSettingRow {
    ConfigSettingRow {
        number,
        name: name.to_string(),
        value: "0".to_string(),
        unit: String::new(),
        description: String::new(),
        category: category.to_string(),
        read_only: false,
    }
}

#[test]
fn test_firmware_settings_filter_matches_eeprom_id() {
    let eeprom = row(1100, "Max feedrate", "Repetier EEPROM");
    assert!(eeprom.matches_filter("epr:100", "All"));
    assert!(!eeprom.matches_filter("$1100", "All"));
}

#[test]
fn test_firmware_settings_filter_matches_grbl_id_and_category() {
    let grbl = row(110, "X max rate", "Max Rate");
    assert!(grbl.matches_filter("$110", "All"));
    assert!(grbl.matches_filter("x max", "Max Rate"));
    assert!(!grbl.matches_filter("", "Homing"));
    assert!(grbl.matches_filter("", "All"));
}

/// The app, connected to a loopback controller, with the Firmware Settings window open and
/// the controller's `$110` and `$22` reported.
fn open_window(
    shared_setup: impl FnOnce(&LoopbackShared),
) -> (
    Harness<'static, GcodeKitApp>,
    std::sync::Arc<LoopbackShared>,
) {
    let (mut state, shared) = loopback_state(Duration::from_millis(1));
    shared_setup(&shared);
    connect(&mut state);
    let mut h = app_harness(GcodeKitApp::from_state(state));
    h.state_mut().dispatch(Command::OpenFirmwareSettings);
    shared.inject("$22=1\n$110=500.000\n");
    assert!(
        step_until(&mut h, |app| app
            .firmware_settings()
            .visible_rows()
            .iter()
            .any(|r| r.number == 110 && r.value == "500.000")),
        "the reported $110 never showed"
    );
    (h, shared)
}

/// Replaces the text of the text field labelled `label` with `text`.
fn set_field(h: &mut Harness<'_, GcodeKitApp>, label: &str, text: &str) {
    h.get_by_role_and_label(Role::TextInput, label).focus();
    h.step();
    h.key_combination_modifiers(egui::Modifiers::COMMAND, &[egui::Key::A]);
    if text.is_empty() {
        h.key_press(egui::Key::Backspace);
    } else {
        h.event(egui::Event::Text(text.to_string()));
    }
    h.step();
}

fn ids(app: &GcodeKitApp) -> Vec<String> {
    app.firmware_settings()
        .visible_rows()
        .iter()
        .map(ConfigSettingRow::id)
        .collect()
}

#[test]
fn test_firmware_settings_search_and_category_filter_the_list() {
    let (mut h, _shared) = open_window(|_| {});
    assert!(ids(h.state()).len() > 50, "the GRBL definitions are listed");

    set_field(&mut h, "Filter:", "$110");
    h.run_steps(2);
    assert_eq!(ids(h.state()), ["$110"]);
    assert!(h.query_by_label("$110").is_some());
    assert!(h.query_by_label("$111").is_none());

    // The category narrows the search further.
    h.get_by_role_and_label(Role::ComboBox, "Category:").click();
    h.run_steps(2);
    h.get_by_label("Homing").click();
    h.run_steps(2);
    assert!(ids(h.state()).is_empty());
    h.get_by_role_and_label(Role::ComboBox, "Category:").click();
    h.run_steps(2);
    h.get_by_label("Max Rate").click();
    h.run_steps(2);
    assert_eq!(ids(h.state()), ["$110"]);
}

/// Finds and clicks the `$110` row and types `value` into the editor.
fn edit_110(h: &mut Harness<'_, GcodeKitApp>, value: &str) {
    // A window is laid out, invisibly, on its first frames; the search brings the row to
    // the top of the list, inside the window.
    h.run_steps(3);
    set_field(h, "Filter:", "$110");
    h.run_steps(2);
    h.get_by_label("$110").click();
    h.run_steps(2);
    set_field(h, "Value:", value);
}

#[test]
fn test_firmware_settings_write_sends_exactly_the_line() {
    let (mut h, shared) = open_window(|_| {});
    edit_110(&mut h, "5000");
    let before = shared.log().len();
    h.get_by_label("Apply").click();
    h.run_steps(2);
    // The first line written since `before` (the status poll's realtime `?` is no line).
    let first_line = || {
        shared.log()[before..]
            .iter()
            .find(|w| matches!(w, Wrote::Line(_)))
            .cloned()
    };
    assert!(wait_until(|| first_line().is_some()), "nothing was written");
    assert_eq!(
        first_line(),
        Some(Wrote::Line("$110=5000".to_string())),
        "the first line written is the setting"
    );

    // The controller's own report confirms the new value.
    assert!(wait_until(|| shared
        .log()
        .contains(&Wrote::Line("$$".to_string()))));
    shared.inject("$110=5000.000\n");
    assert!(
        step_until(&mut h, |app| app
            .firmware_settings()
            .visible_rows()
            .iter()
            .any(|r| r.number == 110 && r.value == "5000.000")),
        "the confirmed value never showed"
    );
    assert!(
        step_until(&mut h, |app| app
            .firmware_settings()
            .notice()
            .is_some_and(|n| n.contains("$110=5000"))),
        "no notice of the written setting"
    );
}

#[test]
fn test_firmware_settings_rejected_write_keeps_the_value() {
    let (mut h, shared) = open_window(|_| {});
    shared.hold_acks.store(true, Ordering::SeqCst);
    edit_110(&mut h, "5");
    h.get_by_label("Apply").click();
    h.run_steps(2);
    assert!(wait_until(|| shared
        .log()
        .contains(&Wrote::Line("$110=5".to_string()))));
    shared.inject("error:3\n");
    assert!(
        step_until(&mut h, |app| app.firmware_settings().notice().is_some_and(
            |n| n.contains("Setting not changed") && n.contains("error:3")
        )),
        "no rejection notice: {:?}",
        h.state().firmware_settings().notice()
    );
    assert!(
        h.state()
            .firmware_settings()
            .visible_rows()
            .iter()
            .any(|r| r.number == 110 && r.value == "500.000")
    );
}

#[test]
fn test_firmware_settings_refuses_writes_while_a_job_runs() {
    let (mut state, shared) = loopback_state(Duration::from_millis(1));
    start_held_stream(&mut state, &shared, 5);
    let mut h = app_harness(GcodeKitApp::from_state(state));
    h.state_mut().dispatch(Command::OpenFirmwareSettings);
    shared.inject("$110=500.000\n");
    assert!(step_until(&mut h, |app| app
        .firmware_settings()
        .visible_rows()
        .iter()
        .any(|r| r.number == 110)));

    edit_110(&mut h, "5000");
    h.get_by_label("Apply").click();
    h.run_steps(2);
    h.get_by_label("Retrieve").click();
    h.run_steps(2);
    std::thread::sleep(Duration::from_millis(50));
    h.run_steps(2);
    let written = shared.log();
    assert!(
        !written
            .iter()
            .any(|w| matches!(w, Wrote::Line(l) if l.starts_with('$'))),
        "a settings command was written during the job: {written:?}"
    );
    let notice = h.state().firmware_settings().notice().unwrap_or_default();
    assert!(notice.contains("a job is running"), "notice: {notice:?}");
    assert!(h.query_by_label_contains("a job is running").is_some());
}

#[test]
fn test_firmware_settings_refuses_writes_while_disconnected() {
    let (state, shared) = loopback_state(Duration::from_millis(1));
    let mut h = app_harness(GcodeKitApp::from_state(state));
    h.state_mut().dispatch(Command::OpenFirmwareSettings);
    h.state_mut().firmware_settings_mut().machine_events(
        &[MachineEvent::Received("$110=500.000".to_string())],
        &gcodekit6_app::i18n::Translator::new("en"),
    );
    h.run_steps(2);
    edit_110(&mut h, "5000");
    h.get_by_label("Apply").click();
    h.run_steps(2);
    assert!(shared.log().is_empty());
    let notice = h.state().firmware_settings().notice().unwrap_or_default();
    assert!(
        notice.contains("Setting not changed") && notice.contains("not connected"),
        "notice: {notice:?}"
    );
    assert!(h.query_by_label_contains("Setting not changed").is_some());

    h.get_by_label("Retrieve").click();
    h.run_steps(2);
    assert!(
        h.query_by_label_contains("Please connect the machine before retrieving parameters.")
            .is_some()
    );
}

#[test]
fn test_firmware_settings_retrieve_sends_dump_and_identity_queries() {
    let (mut h, shared) = open_window(|_| {});
    let before = shared.log().len();
    // The lines written since `before`: the status poll's realtime `?` can land among them
    // under load and is not part of the retrieve.
    let lines = || -> Vec<Wrote> {
        shared.log()[before..]
            .iter()
            .filter(|w| matches!(w, Wrote::Line(_)))
            .cloned()
            .collect()
    };
    h.get_by_label("Retrieve").click();
    h.run_steps(2);
    assert!(wait_until(|| lines().len() >= 2));
    assert_eq!(
        lines()[..2],
        [Wrote::Line("$$".to_string()), Wrote::Line("$I".to_string())]
    );
    shared.inject("$0=10\n$110=750.000\n[VER:1.1h.20190825:]\n");
    assert!(
        step_until(&mut h, |app| app
            .firmware_settings()
            .notice()
            .is_some_and(|n| n.contains("Settings retrieved from device: 2"))),
        "notice: {:?}",
        h.state().firmware_settings().notice()
    );
}

/// Whether `line` was written to the loopback.
fn wrote(shared: &LoopbackShared, line: &str) -> bool {
    shared.log().contains(&Wrote::Line(line.to_string()))
}

#[test]
fn test_firmware_settings_write_waits_for_an_earlier_retrieve() {
    let (mut h, shared) = open_window(|_| {});
    edit_110(&mut h, "5000");
    shared.hold_acks.store(true, Ordering::SeqCst);
    h.get_by_label("Retrieve").click();
    h.run_steps(2);
    assert!(wait_until(|| wrote(&shared, "$I")));

    // The dump is still coming: the write would take its lines as its own answer.
    h.get_by_label("Apply").click();
    h.run_steps(2);
    std::thread::sleep(Duration::from_millis(30));
    h.run_steps(2);
    assert!(!wrote(&shared, "$110=5000"), "written during the retrieve");

    // The dump ended, but `$$` and `$I` are still unanswered.
    shared.inject("$110=500.000\n[VER:1.1h.20190825:]\n");
    assert!(step_until(&mut h, |app| app
        .firmware_settings()
        .notice()
        .is_some_and(|n| n.contains("Settings retrieved"))));
    h.get_by_label("Apply").click();
    h.run_steps(2);
    std::thread::sleep(Duration::from_millis(30));
    h.run_steps(2);
    assert!(
        !wrote(&shared, "$110=5000"),
        "written before the retrieve was answered"
    );
    let notice = h.state().firmware_settings().notice().unwrap_or_default();
    assert!(
        notice.contains("waiting for the controller") && !notice.contains("Setting written"),
        "notice: {notice:?}"
    );

    // Once answered, the write goes out.
    shared.hold_acks.store(false, Ordering::SeqCst);
    edit_110(&mut h, "5000");
    assert!(step_until(&mut h, |app| {
        let machine = app.state().machine.controller();
        gcodekit6_ui_egui::machine::lock(machine).lines_answered()
    }));
    h.get_by_label("Apply").click();
    h.run_steps(2);
    assert!(wait_until(|| wrote(&shared, "$110=5000")));
}

#[test]
fn test_firmware_settings_earlier_error_is_not_the_writes_answer() {
    let (mut h, shared) = open_window(|_| {});
    shared.hold_acks.store(true, Ordering::SeqCst);
    gcodekit6_ui_egui::machine::lock(h.state().state().machine.controller())
        .send_line("G4 P0")
        .expect("send");
    assert!(wait_until(|| wrote(&shared, "G4 P0")));
    edit_110(&mut h, "5000");
    h.get_by_label("Apply").click();
    h.run_steps(2);
    assert!(
        !wrote(&shared, "$110=5000"),
        "written behind an unanswered line"
    );

    // G4's own error answers it; it is not the write's.
    shared.inject("error:20\n");
    assert!(step_until(&mut h, |app| gcodekit6_ui_egui::machine::lock(
        app.state().machine.controller()
    )
    .lines_answered()));
    edit_110(&mut h, "5000");
    h.get_by_label("Apply").click();
    h.run_steps(2);
    assert!(wait_until(|| wrote(&shared, "$110=5000")));
    shared.hold_acks.store(false, Ordering::SeqCst);
    assert!(wait_until(|| wrote(&shared, "$$")));
    shared.inject("$110=5000.000\n");
    assert!(
        step_until(&mut h, |app| app
            .firmware_settings()
            .notice()
            .is_some_and(|n| n.contains("Setting written"))),
        "notice: {:?}",
        h.state().firmware_settings().notice()
    );
}

#[test]
fn test_firmware_settings_repetier_float_slot_written_as_float() {
    let (mut h, shared) = open_window(|_| {});
    shared.inject("FIRMWARE_NAME:Repetier_1.0.6 MACHINE_TYPE:Mendel EXTRUDER_COUNT:1\n");
    assert!(step_until(&mut h, |app| app
        .device_info()
        .identity()
        .is_repetier()));
    h.get_by_label("Retrieve").click();
    h.run_steps(2);
    assert!(wait_until(|| wrote(&shared, "M115")));
    shared
        .inject("EPR:3 11 80.0000 X-axis steps per mm [steps/mm]\nFIRMWARE_NAME:Repetier_1.0.6\n");
    assert!(step_until(&mut h, |app| app
        .firmware_settings()
        .notice()
        .is_some_and(|n| n.contains("Settings retrieved"))));
    assert!(step_until(&mut h, |app| gcodekit6_ui_egui::machine::lock(
        app.state().machine.controller()
    )
    .lines_answered()));

    h.run_steps(3);
    set_field(&mut h, "Filter:", "epr:11");
    h.run_steps(2);
    h.get_by_label("EPR:11").click();
    h.run_steps(2);
    set_field(&mut h, "Value:", "80");
    h.get_by_label("Apply").click();
    h.run_steps(2);
    assert!(
        wait_until(|| wrote(&shared, "M206 T3 P11 X80")),
        "written: {:?}",
        shared.log()
    );
}

#[test]
fn test_firmware_settings_invalid_value_is_not_sent() {
    let (mut h, shared) = open_window(|_| {});
    for bad in ["5,000", "abc", ""] {
        edit_110(&mut h, bad);
        h.get_by_label("Apply").click();
        h.run_steps(2);
        let notice = h.state().firmware_settings().notice().unwrap_or_default();
        assert!(
            notice.contains("Setting not changed")
                && (notice.contains("Enter a number") || notice.contains("Enter a value")),
            "{bad:?}: {notice:?}"
        );
    }
    std::thread::sleep(Duration::from_millis(30));
    h.run_steps(2);
    assert!(
        !shared
            .log()
            .iter()
            .any(|w| matches!(w, Wrote::Line(l) if l.starts_with("$110"))),
        "{:?}",
        shared.log()
    );
}

#[test]
fn test_firmware_settings_unreported_rows_are_not_editable() {
    let (mut h, shared) = open_window(|_| {});
    // `$111` is listed from the definitions but the controller never reported it.
    h.run_steps(3);
    set_field(&mut h, "Filter:", "$111");
    h.run_steps(2);
    h.get_by_label("$111").click();
    h.run_steps(2);
    assert!(
        h.query_by_role_and_label(Role::TextInput, "Value:")
            .is_none(),
        "the placeholder row opened an editor"
    );

    // A Retrieve starts from placeholders: `$110` is editable again only once reported.
    shared.hold_acks.store(true, Ordering::SeqCst);
    h.get_by_label("Retrieve").click();
    h.run_steps(2);
    set_field(&mut h, "Filter:", "$110");
    h.run_steps(2);
    h.get_by_label("$110").click();
    h.run_steps(2);
    assert!(
        h.query_by_role_and_label(Role::TextInput, "Value:")
            .is_none()
    );
    shared.inject("$110=500.000\n");
    assert!(step_until(&mut h, |app| app
        .firmware_settings()
        .visible_rows()
        .iter()
        .any(|r| r.number == 110 && r.value == "500.000")));
    h.run_steps(2);
    h.get_by_label("$110").click();
    h.run_steps(2);
    assert!(
        h.query_by_role_and_label(Role::TextInput, "Value:")
            .is_some()
    );
}

#[test]
fn test_firmware_settings_disconnect_keeps_an_unread_result() {
    let (mut h, shared) = open_window(|_| {});
    edit_110(&mut h, "5000");
    h.get_by_label("Apply").click();
    h.run_steps(2);
    assert!(wait_until(|| wrote(&shared, "$$")));
    shared.inject("$110=5000.000\n");
    assert!(step_until(&mut h, |app| app
        .firmware_settings()
        .notice()
        .is_some_and(|n| n.contains("Setting written"))));

    h.state_mut().state_mut().machine.disconnect();
    h.run_steps(3);
    let notice = h.state().firmware_settings().notice().unwrap_or_default();
    assert!(notice.contains("Setting written"), "notice: {notice:?}");
    assert!(
        h.query_by_label("Not connected").is_some(),
        "the connection state shows"
    );
}

#[test]
fn test_firmware_settings_disconnect_ends_a_retrieve_with_a_notice() {
    let (mut h, shared) = open_window(|_| {});
    shared.hold_acks.store(true, Ordering::SeqCst);
    h.get_by_label("Retrieve").click();
    h.run_steps(2);
    assert!(wait_until(|| wrote(&shared, "$I")));
    h.state_mut().state_mut().machine.disconnect();
    h.run_steps(3);
    let notice = h.state().firmware_settings().notice().unwrap_or_default();
    assert_eq!(notice, "Not connected");
}

/// A GRBL write whose read-back holds another value (GRBL 1.1 stores `$3` in a byte, so
/// `$3=257` becomes `$3=1`) is a mismatch naming the stored value, never accepted.
#[test]
fn test_firmware_settings_write_read_back_with_another_value_is_a_mismatch() {
    use gcodekit6_app::i18n::Translator;
    use gcodekit6_ui_egui::windows::setting_write::{PendingWrite, WriteOutcome};

    let (mut state, _shared) = loopback_state(Duration::from_millis(1));
    connect(&mut state);
    let tr = Translator::new("en");
    let mut write = PendingWrite::send(&state.machine, "$3=257", true, &tr).expect("send");
    assert_eq!(
        write.observe(&MachineEvent::Sent("$3=257".to_string())),
        None
    );
    // Earlier settings of the dump say the write's `ok` came first, not what `$3` holds.
    assert_eq!(
        write.observe(&MachineEvent::Received("$0=10".to_string())),
        None
    );
    assert_eq!(
        write.observe(&MachineEvent::Received("$3=1".to_string())),
        Some(WriteOutcome::Mismatch("$3=1".to_string()))
    );

    // The same value, printed with the controller's decimals, is accepted.
    let mut write = PendingWrite::send(&state.machine, "$110=5000", true, &tr).expect("send");
    write.observe(&MachineEvent::Sent("$110=5000".to_string()));
    assert_eq!(
        write.observe(&MachineEvent::Received("$110=5000.000".to_string())),
        Some(WriteOutcome::Accepted)
    );
    // A float is compared to the precision the controller prints.
    let mut write = PendingWrite::send(&state.machine, "$11=0.0104", true, &tr).expect("send");
    write.observe(&MachineEvent::Sent("$11=0.0104".to_string()));
    assert_eq!(
        write.observe(&MachineEvent::Received("$11=0.010".to_string())),
        Some(WriteOutcome::Accepted)
    );
    // An integer reported without decimals is compared exactly.
    let mut write = PendingWrite::send(&state.machine, "$26=25.5", true, &tr).expect("send");
    write.observe(&MachineEvent::Sent("$26=25.5".to_string()));
    assert_eq!(
        write.observe(&MachineEvent::Received("$26=25".to_string())),
        Some(WriteOutcome::Mismatch("$26=25".to_string()))
    );
}

/// A read-back drops only an old GRBL's trailing `(comment)`: a text value with spaces is
/// compared whole, so a value cut after its first word is a mismatch.
#[test]
fn test_firmware_settings_read_back_keeps_a_multi_word_value() {
    use gcodekit6_app::i18n::Translator;
    use gcodekit6_ui_egui::windows::setting_write::{PendingWrite, WriteOutcome};

    let (mut state, _shared) = loopback_state(Duration::from_millis(1));
    connect(&mut state);
    let tr = Translator::new("en");
    let read_back = |written: &str, stored: &str| {
        let mut write = PendingWrite::send(&state.machine, written, true, &tr).expect("send");
        write.observe(&MachineEvent::Sent(written.to_string()));
        write.observe(&MachineEvent::Received(stored.to_string()))
    };
    assert_eq!(
        read_back("$70=my shop cnc", "$70=my shop cnc"),
        Some(WriteOutcome::Accepted)
    );
    assert_eq!(
        read_back("$70=my shop cnc", "$70=my"),
        Some(WriteOutcome::Mismatch("$70=my".to_string()))
    );
    assert_eq!(
        read_back("$3=1", "$3=1 (dir invert mask)"),
        Some(WriteOutcome::Accepted)
    );
    assert_eq!(
        read_back("$3=257", "$3=1 (dir invert mask)"),
        Some(WriteOutcome::Mismatch("$3=1".to_string()))
    );
}

/// The Firmware Settings window reports a stored value that differs from the written one
/// as a failure naming both, never as "Setting written".
#[test]
fn test_firmware_settings_mismatched_read_back_is_reported() {
    let (mut h, shared) = open_window(|_| {});
    edit_110(&mut h, "5000");
    h.get_by_label("Apply").click();
    h.run_steps(2);
    assert!(wait_until(|| wrote(&shared, "$$")));
    shared.inject("$22=1\n$110=4000.000\n");
    assert!(
        step_until(&mut h, |app| app.firmware_settings().notice().is_some_and(
            |n| n.contains("$110=5000") && n.contains("$110=4000.000")
        )),
        "notice: {:?}",
        h.state().firmware_settings().notice()
    );
    let notice = h.state().firmware_settings().notice().unwrap_or_default();
    assert!(!notice.contains("Setting written"), "{notice:?}");
    // The setting did change, to the stored value: not "Setting not changed".
    assert!(
        notice.starts_with("Setting stored with another value: $110=5000"),
        "{notice:?}"
    );
    assert!(!notice.contains("Setting not changed"), "{notice:?}");
}

/// A value GRBL would refuse (negative) or truncate (a byte mask over 255, a fraction) is
/// refused with a notice and never sent.
#[test]
fn test_firmware_settings_value_grbl_would_truncate_is_not_sent() {
    let (mut h, shared) = open_window(|_| {});
    shared.inject("$3=0\n");
    assert!(step_until(&mut h, |app| app
        .firmware_settings()
        .visible_rows()
        .iter()
        .any(|r| r.number == 3)));
    h.run_steps(3);
    for (bad, reason) in [
        ("257", "out of range"),
        ("1.5", "whole number"),
        ("-1", "0 or more"),
    ] {
        set_field(&mut h, "Filter:", "$3");
        h.run_steps(2);
        h.get_by_label("$3").click();
        h.run_steps(2);
        set_field(&mut h, "Value:", bad);
        h.get_by_label("Apply").click();
        h.run_steps(2);
        let notice = h.state().firmware_settings().notice().unwrap_or_default();
        assert!(
            notice.contains("Setting not changed") && notice.contains(reason),
            "{bad:?}: {notice:?}"
        );
    }
    std::thread::sleep(Duration::from_millis(30));
    h.run_steps(2);
    assert!(
        !shared
            .log()
            .iter()
            .any(|w| matches!(w, Wrote::Line(l) if l.starts_with("$3="))),
        "{:?}",
        shared.log()
    );
}

/// On a controller that said it runs grblHAL, a mask below `$100` is not held to GRBL
/// 1.1's byte: the window sends the wider value.
#[test]
fn test_firmware_settings_grblhal_mask_above_255_is_sent() {
    let (mut h, shared) = open_window(|_| {});
    shared.inject("[FIRMWARE:grblHAL]\n$3=0\n");
    assert!(step_until(&mut h, |app| app
        .firmware_settings()
        .visible_rows()
        .iter()
        .any(|r| r.number == 3)));
    h.run_steps(3);
    set_field(&mut h, "Filter:", "$3");
    h.run_steps(2);
    h.get_by_label("$3").click();
    h.run_steps(2);
    set_field(&mut h, "Value:", "300");
    h.get_by_label("Apply").click();
    h.run_steps(2);
    assert!(
        wait_until(|| wrote(&shared, "$3=300")),
        "notice: {:?}",
        h.state().firmware_settings().notice()
    );
}

/// A write waits for an earlier jog's reply, so the jog's `error:15` is never taken for the
/// write's answer.
#[test]
fn test_firmware_settings_write_waits_for_an_earlier_jog() {
    let (mut h, shared) = open_window(|_| {});
    shared.hold_acks.store(true, Ordering::SeqCst);
    gcodekit6_ui_egui::machine::lock(h.state().state().machine.controller()).jog(
        gcodekit6_app::machine::Axis::X,
        500.0,
        1000.0,
    );
    assert!(wait_until(|| shared
        .log()
        .iter()
        .any(|w| matches!(w, Wrote::Line(l) if l.starts_with("$J=")))));
    edit_110(&mut h, "5000");
    h.get_by_label("Apply").click();
    h.run_steps(2);
    assert!(!wrote(&shared, "$110=5000"), "written behind the jog");
    let notice = h.state().firmware_settings().notice().unwrap_or_default();
    assert!(notice.contains("waiting for the controller"), "{notice:?}");
    shared.inject("error:15\n");
    assert!(step_until(&mut h, |app| gcodekit6_ui_egui::machine::lock(
        app.state().machine.controller()
    )
    .lines_answered()));
}
