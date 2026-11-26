//! Work zero in the Machine Control pane against the loopback controller: the per-axis Zero
//! buttons, the G54-G59 selector, Zero All Axes (confirmed) and Go to Work Zero, with the
//! active work coordinate system taken from the controller's `[GC:...]` parser state.

use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use gcodekit5_app::machine::MachineEvent;
use gcodekit5_app::test_support::{LoopbackShared, Wrote};
use gcodekit5_app::work_zero::Wcs;
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::machine::MachineStatus;

use crate::machine_support::{
    EVENTUALLY, app_harness, loopback_state, start_held_stream, step_until,
};
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
    let (mut state, shared) = loopback_state(Duration::from_millis(1));
    start_held_stream(&mut state, &shared, 50);
    let mut harness = app_harness(GcodeKitApp::from_state(state));
    harness.step();
    harness.step();
    click(&mut harness, "G55");
    assert!(shows_notice(
        &mut harness,
        "Work coordinate change ignored: a job is running."
    ));
    click(&mut harness, "Zero X");
    assert!(!wrote(&shared, "G55"), "WCS changed during a stream");
    assert!(!wrote_prefix(&shared, "G10"), "zeroed during a stream");
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
    status.apply(&[MachineEvent::Disconnected]);
    assert_eq!(status.active_wcs, None);
    assert!(!status.wcs_reported);
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
