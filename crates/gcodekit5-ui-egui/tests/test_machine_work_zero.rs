//! Work zero in the Machine Control pane against the loopback controller: the per-axis Zero
//! buttons, the G54-G59 selector, Zero All Axes (confirmed) and Go to Work Zero, with the
//! active work coordinate system taken from the controller's `[GC:...]` parser state.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use gcodekit5_app::machine::MachineEvent;
use gcodekit5_app::test_support::{LoopbackShared, Wrote};
use gcodekit5_app::work_zero::Wcs;
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::machine::MachineStatus;

use crate::machine_support::{EVENTUALLY, step_until};
use crate::test_machine_pane::{pane_app, report_idle, report_state};

/// Every work zero control's accessible name.
const CONTROLS: [&str; 11] = [
    "Zero X",
    "Zero Y",
    "Zero Z",
    "G54",
    "G55",
    "G56",
    "G57",
    "G58",
    "G59",
    "Zero All Axes",
    "Go to Work Zero",
];

/// Whether the loopback saw the line `line`.
fn wrote(shared: &LoopbackShared, line: &str) -> bool {
    shared.log().contains(&Wrote::Line(line.to_string()))
}

/// How many `$G` lines were written.
fn count_dollar_g(shared: &LoopbackShared) -> usize {
    shared
        .log()
        .iter()
        .filter(|w| **w == Wrote::Line("$G".to_string()))
        .count()
}

/// Whether any line starting with `prefix` was written.
fn wrote_prefix(shared: &LoopbackShared, prefix: &str) -> bool {
    shared
        .log()
        .iter()
        .any(|w| matches!(w, Wrote::Line(l) if l.starts_with(prefix)))
}

/// Scrolls the node named `label` into view (the pane scrolls over a few frames).
fn scroll_to(harness: &mut Harness<'_, GcodeKitApp>, label: &str) {
    harness.get_by_label(label).scroll_to_me();
    for _ in 0..4 {
        harness.step();
    }
}

/// Whether the pane shows the notice `text` (at its top, so scrolled to first).
fn shows_notice(harness: &mut Harness<'_, GcodeKitApp>, text: &str) -> bool {
    scroll_to(harness, "Unlock");
    harness.query_by_label(text).is_some()
}

/// Scrolls the control named `label` into view and clicks it.
fn click(harness: &mut Harness<'_, GcodeKitApp>, label: &str) {
    scroll_to(harness, label);
    harness.get_by_label(label).click();
    harness.step();
    harness.step();
}

/// Whether the control named `label` is enabled.
fn enabled(harness: &Harness<'_, GcodeKitApp>, label: &str) -> bool {
    !harness.get_by_label(label).accesskit_node().is_disabled()
}

/// Steps until the first `$G` was answered and the zero buttons are enabled.
fn wait_zero_ready(harness: &mut Harness<'_, GcodeKitApp>, shared: &LoopbackShared) {
    assert!(
        step_until(harness, |_| wrote(shared, "$G")),
        "no $G after connect: {:?}",
        shared.log()
    );
    assert!(step_enabled(harness, "Zero X"), "Zero X never enabled");
}

/// Steps until the control named `label` is enabled or [`EVENTUALLY`] passes.
fn step_enabled(harness: &mut Harness<'_, GcodeKitApp>, label: &str) -> bool {
    let deadline = Instant::now() + EVENTUALLY;
    while Instant::now() < deadline {
        harness.step();
        if enabled(harness, label) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    false
}

/// Reports `wcs` as the controller's active system and steps until the app has it.
fn report_wcs(harness: &mut Harness<'_, GcodeKitApp>, shared: &LoopbackShared, wcs: Wcs) {
    shared.inject(&format!(
        "[GC:G0 {} G17 G21 G90 G94 M5 M9 T0 F0 S0]\n",
        wcs.code()
    ));
    assert!(
        step_until(harness, |app| app.state().machine.status().active_wcs
            == Some(wcs)),
        "{wcs:?} never reported"
    );
    harness.step();
    harness.step();
}

#[test]
fn test_work_zero_controls_disabled_when_disconnected() {
    let (harness, _shared) = pane_app(Duration::from_millis(1), false);
    for label in CONTROLS {
        assert!(!enabled(&harness, label), "{label} enabled while offline");
    }
}

#[test]
fn test_zero_x_uses_reported_g55() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    assert!(
        step_until(&mut harness, |_| wrote(&shared, "$G")),
        "no $G after connect"
    );
    report_wcs(&mut harness, &shared, Wcs::G55);
    report_idle(&mut harness, &shared);
    assert!(step_enabled(&mut harness, "Zero X"), "Zero X disabled");
    click(&mut harness, "Zero X");
    assert!(
        step_until(&mut harness, |_| wrote(&shared, "G10 L20 P2 X0")),
        "Zero X not written as P2: {:?}",
        shared.log()
    );
    assert!(!wrote(&shared, "G10 L20 P1 X0"));
}

#[test]
fn test_zero_all_needs_confirmation() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    wait_zero_ready(&mut harness, &shared);
    report_idle(&mut harness, &shared);
    click(&mut harness, "Zero All Axes");
    assert_eq!(
        harness.state().dialogs().waiting(),
        1,
        "no confirmation asked"
    );
    assert!(!wrote_prefix(&shared, "G10"), "zeroed before the answer");

    // No: nothing is sent.
    harness
        .state_mut()
        .dialogs_mut()
        .set_override(Some(Box::new(|_| {
            craft_dialogs::Answer::Message(craft_dialogs::MessageResult::No)
        })));
    harness
        .state_mut()
        .dialogs_mut()
        .end_frame(None::<&eframe::Frame>);
    harness.step();
    harness.step();
    assert!(!wrote_prefix(&shared, "G10"), "zeroed after No");

    // Yes: X, Y and Z of the active system (G54 until the controller says otherwise).
    click(&mut harness, "Zero All Axes");
    harness
        .state_mut()
        .dialogs_mut()
        .set_override(Some(Box::new(|_| {
            craft_dialogs::Answer::Message(craft_dialogs::MessageResult::Yes)
        })));
    harness
        .state_mut()
        .dialogs_mut()
        .end_frame(None::<&eframe::Frame>);
    assert!(
        step_until(&mut harness, |_| wrote(&shared, "G10 L20 P1 X0 Y0 Z0")),
        "Zero All not written after Yes: {:?}",
        shared.log()
    );
}

#[test]
fn test_go_to_work_zero_refused_when_not_idle() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    wait_zero_ready(&mut harness, &shared);
    report_state(
        &mut harness,
        &shared,
        "<Alarm|MPos:0.000,0.000,0.000|FS:0,0>",
        "Alarm",
    );
    click(&mut harness, "Go to Work Zero");
    assert!(!wrote_prefix(&shared, "G90 G0"), "moved while not idle");
    assert!(shows_notice(
        &mut harness,
        "Go to Work Zero refused: the machine is not idle."
    ));

    report_idle(&mut harness, &shared);
    // Include Z stays off unless asked for.
    assert!(
        step_until(&mut harness, |app| app.state().machine.jog_allowed()),
        "report never fresh"
    );
    click(&mut harness, "Go to Work Zero");
    assert!(
        step_until(&mut harness, |_| wrote(&shared, "G90 G0 X0 Y0")),
        "Go to Work Zero not written when idle: {:?}",
        shared.log()
    );
    assert!(!wrote(&shared, "G90 G0 X0 Y0 Z0"));
}

#[test]
fn test_wcs_change_refused_during_stream() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    // The active system is settled before the job starts, so Zero X is enabled and only the
    // job guard can refuse it.
    wait_zero_ready(&mut harness, &shared);
    report_idle(&mut harness, &shared);
    shared.hold_acks.store(true, Ordering::SeqCst);
    let machine = &mut harness.state_mut().state_mut().machine;
    machine
        .set_program("G1 X1 F100\nG1 X2 F100\n")
        .expect("program");
    let program = machine.program().cloned().expect("program loaded");
    gcodekit5_ui_egui::machine::lock(machine.controller())
        .start_stream(program)
        .expect("start_stream");
    assert!(
        step_until(&mut harness, |app| app.state().machine.status().streaming),
        "stream never started"
    );
    harness.step();
    assert!(
        enabled(&harness, "Zero X"),
        "Zero X disabled during the job"
    );
    click(&mut harness, "Zero X");
    assert!(shows_notice(
        &mut harness,
        "Command ignored: a job is running."
    ));
    click(&mut harness, "G55");
    assert!(shows_notice(
        &mut harness,
        "Work coordinate change ignored: a job is running."
    ));
    assert!(!wrote(&shared, "G55"), "WCS changed during a stream");
    assert!(!wrote_prefix(&shared, "G10"), "zeroed during a stream");
}

#[test]
fn test_zero_refused_when_not_idle() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    wait_zero_ready(&mut harness, &shared);
    report_state(
        &mut harness,
        &shared,
        "<Hold:0|MPos:0.000,0.000,0.000|FS:0,0>",
        "Hold:0",
    );
    click(&mut harness, "Zero X");
    assert!(shows_notice(
        &mut harness,
        "Zero refused: the machine is not idle."
    ));
    click(&mut harness, "Zero All Axes");
    assert_eq!(
        harness.state().dialogs().waiting(),
        0,
        "asked while not idle"
    );
    assert!(!wrote_prefix(&shared, "G10"), "zeroed while not idle");
}

#[test]
fn test_zero_all_refused_when_the_wcs_changed_before_yes() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    wait_zero_ready(&mut harness, &shared);
    report_idle(&mut harness, &shared);
    click(&mut harness, "Zero All Axes");
    assert_eq!(
        harness.state().dialogs().waiting(),
        1,
        "no confirmation asked"
    );
    // While the question (naming G54) is open, the console selects G55.
    gcodekit5_ui_egui::machine::lock(harness.state().state().machine.controller())
        .send_line("G55")
        .expect("send G55");
    assert!(
        step_until(&mut harness, |_| count_dollar_g(&shared) == 2),
        "no $G after G55"
    );
    assert!(step_enabled(&mut harness, "Zero X"), "G55 never settled");
    report_idle(&mut harness, &shared);
    assert!(
        step_until(&mut harness, |app| app.state().machine.jog_allowed()),
        "report never fresh after G55"
    );
    harness
        .state_mut()
        .dialogs_mut()
        .set_override(Some(Box::new(|_| {
            craft_dialogs::Answer::Message(craft_dialogs::MessageResult::Yes)
        })));
    harness
        .state_mut()
        .dialogs_mut()
        .end_frame(None::<&eframe::Frame>);
    harness.step();
    harness.step();
    assert!(shows_notice(
        &mut harness,
        "Zero All refused: the work coordinate system changed."
    ));
    assert!(!wrote_prefix(&shared, "G10"), "zeroed another system");
}

#[test]
fn test_wcs_outside_g54_g59_disables_zero() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    wait_zero_ready(&mut harness, &shared);
    report_idle(&mut harness, &shared);
    // grblHAL reports G59.1: no G10 L20 P1..P6 names it.
    shared.inject("[GC:G0 G59.1 G17 G21 G90 G94 M5 M9 T0 F0 S0]\n");
    assert!(
        step_until(&mut harness, |app| app.state().machine.status().wcs_outside),
        "G59.1 not taken"
    );
    harness.step();
    for label in ["Zero X", "Zero All Axes", "Go to Work Zero"] {
        assert!(!enabled(&harness, label), "{label} enabled in G59.1");
    }
    // The reply does not ask again: no endless $G.
    for _ in 0..20 {
        harness.step();
    }
    assert_eq!(count_dollar_g(&shared), 1, "{:?}", shared.log());
    assert!(!wrote_prefix(&shared, "G10"));

    // Typed in the console: G59.2 is unknown too, and `$G` is asked once for it.
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    wait_zero_ready(&mut harness, &shared);
    gcodekit5_ui_egui::machine::lock(harness.state().state().machine.controller())
        .send_line("G59.2")
        .expect("send G59.2");
    assert!(
        step_until(&mut harness, |app| app.state().machine.status().wcs_outside),
        "G59.2 not followed"
    );
    assert!(
        step_until(&mut harness, |_| count_dollar_g(&shared) == 2),
        "no $G after G59.2"
    );
    for _ in 0..20 {
        harness.step();
    }
    assert!(!enabled(&harness, "Zero X"), "Zero X enabled in G59.2");
    assert_eq!(count_dollar_g(&shared), 2);
}

#[test]
fn test_work_zero_disabled_for_non_grbl_controller() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), false);
    harness
        .state_mut()
        .workspace_mut()
        .set_controller(Some(&gcodekit5_devicedb::ControllerType::Repetier));
    gcodekit5_ui_egui::machine::lock(harness.state().state().machine.controller())
        .connect(&gcodekit5_app::test_support::tcp_settings())
        .expect("connect");
    assert!(
        step_until(&mut harness, |app| app.state().machine.status().connected),
        "never connected"
    );
    for _ in 0..20 {
        harness.step();
    }
    for label in CONTROLS {
        assert!(!enabled(&harness, label), "{label} enabled for Repetier");
    }
    assert!(
        !wrote(&shared, "$G"),
        "$G sent to Repetier: {:?}",
        shared.log()
    );
}

#[test]
fn test_status_tracks_active_wcs() {
    let mut status = MachineStatus::default();
    status.apply(&[MachineEvent::Connected]);
    assert_eq!(status.active_wcs, None);
    // A selection sent before any report is the fallback; a stream end keeps it.
    status.apply(&[
        MachineEvent::Sent("G56".to_string()),
        MachineEvent::StreamFinished { lines: 1 },
    ]);
    assert_eq!(status.active_wcs, Some(Wcs::G56));
    assert!(!status.wcs_reported);
    status.apply(&[MachineEvent::Received("[GC:G0 G57 G17 G21]".to_string())]);
    assert_eq!(status.active_wcs, Some(Wcs::G57));
    assert!(status.wcs_reported);
    // Once reported, a reset makes it unknown until the next report.
    status.apply(&[MachineEvent::Received(
        "Grbl 1.1h ['$' for help]".to_string(),
    )]);
    assert_eq!(status.active_wcs, None);
    status.apply(&[MachineEvent::Sent("G0 G59 X0".to_string())]);
    assert_eq!(status.active_wcs, Some(Wcs::G59));
    // Outside G54-G59, reported or sent: unknown to the zero commands.
    status.apply(&[MachineEvent::Received("[GC:G0 G59.1 G17]".to_string())]);
    assert_eq!(status.active_wcs, None);
    assert!(status.wcs_outside);
    status.apply(&[MachineEvent::Sent("G55".to_string())]);
    assert_eq!(status.active_wcs, Some(Wcs::G55));
    assert!(!status.wcs_outside);
    status.apply(&[MachineEvent::Sent("G59.2".to_string())]);
    assert_eq!(status.active_wcs, None);
    assert!(status.wcs_outside);
    status.apply(&[MachineEvent::Disconnected]);
    assert_eq!(status.active_wcs, None);
    assert!(!status.wcs_reported);
    assert!(!status.wcs_outside);
}

#[test]
fn test_console_wcs_change_is_followed_by_zero() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    wait_zero_ready(&mut harness, &shared);
    // Typed in the console, not chosen in the pane: the zero buttons follow it and ask `$G`.
    gcodekit5_ui_egui::machine::lock(harness.state().state().machine.controller())
        .send_line("G56")
        .expect("send G56");
    assert!(
        step_until(&mut harness, |app| app.state().machine.status().active_wcs
            == Some(Wcs::G56)),
        "G56 not followed"
    );
    assert!(
        step_until(&mut harness, |_| shared
            .log()
            .iter()
            .filter(|w| **w == Wrote::Line("$G".to_string()))
            .count()
            == 2),
        "no $G after G56: {:?}",
        shared.log()
    );
    report_idle(&mut harness, &shared);
    assert!(
        step_until(&mut harness, |app| app.state().machine.jog_allowed()),
        "report never fresh after G56"
    );
    assert!(step_enabled(&mut harness, "Zero Y"), "Zero Y disabled");
    click(&mut harness, "Zero Y");
    assert!(
        step_until(&mut harness, |_| wrote(&shared, "G10 L20 P3 Y0")),
        "Zero Y not written as P3: {:?}",
        shared.log()
    );
}

#[test]
fn test_stream_selecting_a_wcs_is_followed_by_a_parser_state_query() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    wait_zero_ready(&mut harness, &shared);
    let machine = &mut harness.state_mut().state_mut().machine;
    machine.set_program("G55\nG0 X1\n").expect("program");
    let program = machine.program().cloned().expect("program loaded");
    gcodekit5_ui_egui::machine::lock(machine.controller())
        .start_stream(program)
        .expect("start_stream");
    assert!(
        step_until(&mut harness, |app| app
            .state()
            .machine
            .status()
            .stream_end
            .is_some()),
        "stream never ended"
    );
    assert!(
        step_until(&mut harness, |_| shared
            .log()
            .iter()
            .filter(|w| **w == Wrote::Line("$G".to_string()))
            .count()
            == 2),
        "no $G after the stream: {:?}",
        shared.log()
    );
}

/// The app with rotary calibration A = 12.5 degrees, connected, its first `$G` answered.
fn rotary_app() -> (
    Harness<'static, GcodeKitApp>,
    std::sync::Arc<LoopbackShared>,
) {
    let mut config = crate::machine_support::loopback_config();
    config
        .machine
        .rotary_calibration
        .insert("A".to_string(), 12.5);
    let (mut harness, shared) =
        crate::test_machine_pane::pane_app_with(config, Duration::from_millis(1), true);
    wait_zero_ready(&mut harness, &shared);
    (harness, shared)
}

/// The DRO shows a reported rotary axis in degrees, with its own Zero button; an axis the
/// machine does not report has neither.
#[test]
fn test_rotary_axis_shown_in_degrees() {
    let (mut harness, shared) = rotary_app();
    report_state(
        &mut harness,
        &shared,
        "<Hold:0|MPos:0.000,0.000,0.000|FS:0,0>",
        "Hold:0",
    );
    assert!(harness.query_by_label("WA").is_none(), "A shown unreported");
    assert!(harness.query_by_label("Zero A").is_none());
    report_state(
        &mut harness,
        &shared,
        "<Idle|MPos:0.000,0.000,0.000,90.000|WCO:0.000,0.000,0.000,10.000|FS:0,0>",
        "Idle",
    );
    assert!(harness.query_by_label("WA").is_some(), "no WA row");
    assert!(harness.query_by_label("MA:").is_some(), "no MA readout");
    assert!(
        harness.query_by_label("90.00\u{b0}").is_some(),
        "no MA value"
    );
    assert!(
        harness.query_by_label("80.00\u{b0}").is_some(),
        "no WA value"
    );
    assert!(harness.query_by_label("Zero A").is_some());
    assert!(harness.query_by_label("Zero B").is_none());
}

/// Review Focus 2: Zero A sends G10 L20 with the active system's P number (here G55, P2)
/// and the rotary calibration value from Preferences.
#[test]
fn test_rotary_zero_uses_active_wcs_and_calibration() {
    let (mut harness, shared) = rotary_app();
    report_wcs(&mut harness, &shared, Wcs::G55);
    report_state(
        &mut harness,
        &shared,
        "<Idle|MPos:0.000,0.000,0.000,90.000|FS:0,0>",
        "Idle",
    );
    assert!(step_enabled(&mut harness, "Zero A"), "Zero A disabled");
    click(&mut harness, "Zero A");
    assert!(
        step_until(&mut harness, |_| wrote(&shared, "G10 L20 P2 A12.5")),
        "rotary zero not written: {:?}",
        shared.log()
    );
}

/// Review Focus 2: a rotary Zero follows the same rules as X/Y/Z: refused without a fresh
/// Idle report and while a job runs.
#[test]
fn test_rotary_zero_refused_when_not_idle_or_job_running() {
    let (mut harness, shared) = rotary_app();
    report_state(
        &mut harness,
        &shared,
        "<Hold:0|MPos:0.000,0.000,0.000,90.000|FS:0,0>",
        "Hold:0",
    );
    click(&mut harness, "Zero A");
    assert!(shows_notice(
        &mut harness,
        "Zero refused: the machine is not idle."
    ));
    shared.hold_acks.store(true, Ordering::SeqCst);
    let program: String = (0..20).map(|i| format!("G1 X{i} F1000\n")).collect();
    harness
        .state_mut()
        .state_mut()
        .machine
        .set_program(&program)
        .expect("program");
    harness
        .state_mut()
        .dispatch(gcodekit5_ui_egui::commands::Command::StreamStart);
    assert!(
        step_until(&mut harness, |app| app.state().machine.job_running()),
        "job never started"
    );
    report_state(
        &mut harness,
        &shared,
        "<Idle|MPos:0.000,0.000,0.000,90.000|FS:0,0>",
        "Idle",
    );
    click(&mut harness, "Zero A");
    assert!(
        !wrote_prefix(&shared, "G10"),
        "rotary zeroed: {:?}",
        shared.log()
    );
}
