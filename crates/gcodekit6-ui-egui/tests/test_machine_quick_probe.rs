//! Quick probes from Machine Control (GTK's Probe Z and Edge buttons): the Probe Routines
//! program with GTK's values (Z 20 mm down, edges 10 mm, 100 mm/min) streamed through the
//! same run as the CAM page, its result shown in the pane, and the same guards.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use gcodekit6_app::test_support::LoopbackShared;
use gcodekit6_ui_egui::GcodeKitApp;

use crate::machine_support::{start_held_stream, step_until};
use crate::test_cam_probe_run::{lines, reply};
use crate::test_machine_pane::{pane_app, report_state};

/// The Machine Control page, connected and Idle, with acks held so a run keeps streaming.
fn idle_app() -> (Harness<'static, GcodeKitApp>, Arc<LoopbackShared>) {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    shared.hold_acks.store(true, Ordering::SeqCst);
    report_state(
        &mut harness,
        &shared,
        "<Idle|MPos:1.000,2.000,3.000|FS:0,0>",
        "Idle",
    );
    (harness, shared)
}

/// Scrolls `label` into view and clicks it.
fn click(harness: &mut Harness<'_, GcodeKitApp>, label: &str) {
    harness.get_by_label(label).scroll_to_me();
    harness.step();
    harness.step();
    harness.get_by_label(label).click();
    harness.step();
    harness.step();
}

/// Whether the control named `label` is enabled.
fn enabled(harness: &Harness<'_, GcodeKitApp>, label: &str) -> bool {
    !harness.get_by_label(label).accesskit_node().is_disabled()
}

/// Waits until a line starting with `prefix` was written; returns the lines from the first
/// `G21` on.
fn wait_program(
    harness: &mut Harness<'_, GcodeKitApp>,
    shared: &LoopbackShared,
    prefix: &str,
) -> Vec<String> {
    assert!(
        step_until(harness, |_| lines(shared)
            .iter()
            .any(|l| l.starts_with(prefix))),
        "no {prefix} line: {:?}",
        lines(shared)
    );
    let all = lines(shared);
    let start = all.iter().position(|l| l == "G21").expect("G21");
    all[start..].to_vec()
}

#[test]
fn test_quick_probe_z_streams_gtk_values_and_shows_the_result() {
    let (mut harness, shared) = idle_app();
    click(&mut harness, "Probe Z");
    let program = wait_program(&mut harness, &shared, "G38.2");
    assert_eq!(program[..3], ["G21", "G91", "G38.2 Z-20.000 F100.0"]);
    assert!(
        !lines(&shared).iter().any(|l| l.starts_with("G10")),
        "a quick probe wrote a work offset"
    );
    assert!(!enabled(&harness, "Probe Z"), "a second probe could start");
    reply(&shared, "[PRB:1.000,2.000,-7.500:1]");
    shared.hold_acks.store(false, Ordering::SeqCst);
    assert!(
        step_until(&mut harness, |_| lines(&shared).last().map(String::as_str)
            == Some("G90")),
        "G90 not restored: {:?}",
        lines(&shared)
    );
    assert!(
        step_until(&mut harness, |app| !app.state().machine.job_running()),
        "run never ended"
    );
    harness.step();
    harness.step();
    assert!(
        harness
            .query_all_by_label_contains("Surface at machine Z-7.500")
            .next()
            .is_some(),
        "result not shown"
    );
}

#[test]
fn test_quick_probe_edges_use_gtk_distance_and_direction() {
    for (label, line) in [
        ("Probe X- edge", "G38.2 X-10.000 F100.0"),
        ("Probe X+ edge", "G38.2 X10.000 F100.0"),
        ("Probe Y- edge", "G38.2 Y-10.000 F100.0"),
        ("Probe Y+ edge", "G38.2 Y10.000 F100.0"),
    ] {
        let (mut harness, shared) = idle_app();
        click(&mut harness, label);
        let program = wait_program(&mut harness, &shared, "G38.2");
        // No descent before the edge probe (GTK probed at the current height).
        assert_eq!(program[..3], ["G21", "G91", line], "{label}");
    }
}

#[test]
fn test_quick_probe_refused_unless_idle() {
    let (mut harness, shared) = idle_app();
    report_state(
        &mut harness,
        &shared,
        "<Alarm|MPos:1.000,2.000,3.000|FS:0,0>",
        "Alarm",
    );
    let before = lines(&shared).len();
    click(&mut harness, "Probe Z");
    harness.step();
    crate::test_machine_keyboard_jog::settle(&harness, &shared);
    let sent: Vec<String> = lines(&shared)[before..]
        .iter()
        .filter(|l| l.as_str() != "$I")
        .cloned()
        .collect();
    assert!(sent.is_empty(), "lines were sent in Alarm: {sent:?}");
    harness.get_by_label("Unlock").scroll_to_me();
    harness.step();
    harness.step();
    assert!(
        harness
            .query_by_label("The machine must be Idle (Alarm)")
            .is_some(),
        "no refusal shown"
    );
}

#[test]
fn test_quick_probe_disabled_offline_and_during_a_job() {
    let (harness, _shared) = pane_app(Duration::from_millis(1), false);
    for label in ["Probe Z", "Probe X- edge", "Probe Y+ edge"] {
        assert!(!enabled(&harness, label), "{label} enabled offline");
    }
    let (mut state, shared) = crate::machine_support::loopback_state(Duration::from_millis(1));
    start_held_stream(&mut state, &shared, 20);
    let mut harness = crate::machine_support::app_harness(GcodeKitApp::from_state(state));
    harness.step();
    harness.step();
    for label in ["Probe Z", "Probe X+ edge"] {
        assert!(!enabled(&harness, label), "{label} enabled during a job");
    }
}

/// The quick probe's backstop fires through `Pane::wake_in` when the controller goes quiet
/// after refusing a line without halting: the app hands Machine Control an empty batch of
/// events once it is due, and the run resets the controller.
#[test]
fn test_quick_probe_backstop_fires_with_no_machine_events() {
    use gcodekit6_ui_egui::panes::cam_tools::probe_routines::run::UNCONFIRMED_BACKSTOP_DELAY;
    let (mut harness, shared) = idle_app();
    click(&mut harness, "Probe Z");
    wait_program(&mut harness, &shared, "G38.2");
    // The controller goes quiet: no status reply and no ack from now on.
    *shared.reply_budget.lock().expect("budget") = Some(0);
    for _ in 0..10 {
        harness.step();
    }
    // A refused line the controller does not halt for (no feed hold echoed).
    harness.state_mut().state_mut().machine.absorb(vec![
        gcodekit6_app::machine::MachineEvent::Error("error:9 on program line 1".to_string()),
    ]);
    let deadline = std::time::Instant::now() + UNCONFIRMED_BACKSTOP_DELAY * 3;
    while !crate::test_cam_probe_run::reset_written(&shared) && std::time::Instant::now() < deadline
    {
        harness.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        crate::test_cam_probe_run::reset_written(&shared),
        "the backstop never fired"
    );
}
