//! Running a probe routine on the machine (the loopback transport of
//! `gcodekit5_app::test_support`): GTK's guards (connected, Idle, no job, one run at a time)
//! plus the probe input, the program streamed through the controller without blocking,
//! `[PRB:...]` reports read in mm whatever `$13` says, Apply to WCS sending the exact
//! `G10 L2` line, and a run that stops (alarm, an `error:` reply, the user's stop, a
//! disconnect) sending nothing more of the routine and restoring G90 once Idle.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_app::test_support::{LoopbackShared, Wrote};
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::machine::lock;
use gcodekit5_ui_egui::panes::cam_tools::CamTool;
use gcodekit5_ui_egui::panes::cam_tools::probe_routines::RoutineKind;

use crate::machine_support::{connect, loopback_state, start_held_stream, step_until};
use crate::test_cam_drill_press::{click, fixture, pane};
use crate::test_cam_probe_routines::gtk_run_lines;
use crate::test_cam_tools::cam_app;
use crate::test_designer_files_app::frames;

pub const IDLE: &str = "<Idle|MPos:1.000,2.000,3.000|FS:0,0>";
pub const ALARM: &str = "<Alarm|MPos:1.000,2.000,3.000|FS:0,0>";

/// The lines written to the loopback so far.
pub fn lines(shared: &LoopbackShared) -> Vec<String> {
    shared
        .log()
        .into_iter()
        .filter_map(|w| match w {
            Wrote::Line(line) => Some(line),
            Wrote::Realtime(_) => None,
        })
        .collect()
}

/// Has the loopback answer with `line` (a status report answers the next `?`).
pub fn reply(shared: &LoopbackShared, line: &str) {
    shared.inject(&format!("{line}\n"));
}

/// Whether a soft reset (0x18) was written.
pub fn reset_written(shared: &LoopbackShared) -> bool {
    shared.log().contains(&Wrote::Realtime(0x18))
}

/// The machine state of the app's latest status report.
pub fn machine_state(app: &GcodeKitApp) -> Option<String> {
    app.state()
        .machine
        .status()
        .report
        .as_ref()
        .and_then(|r| r.machine_state.clone())
}

/// The Probe Tools page open in a tall window, the loopback connected (when `connected`)
/// with `report` as its status. Acks are held so a run keeps streaming until released.
pub fn probe_app(
    connected: bool,
    report: &str,
) -> (Harness<'static, GcodeKitApp>, Arc<LoopbackShared>) {
    probe_app_with(connected, report, Duration::from_millis(1), true)
}

/// [`probe_app`] with acks after `ok_delay`, held or not.
pub fn probe_app_with(
    connected: bool,
    report: &str,
    ok_delay: Duration,
    hold: bool,
) -> (Harness<'static, GcodeKitApp>, Arc<LoopbackShared>) {
    let (mut state, shared) = loopback_state(ok_delay);
    shared.hold_acks.store(hold, Ordering::SeqCst);
    reply(&shared, report);
    if connected {
        connect(&mut state);
    }
    let (mut h, _script) = cam_app(state);
    h.get_by_label("Probe Tools").click();
    frames(&mut h, 1);
    h.get_by_label("Open").click();
    frames(&mut h, 2);
    assert_eq!(pane(&h).open_tool(), Some(CamTool::ProbeRoutines));
    h.set_size(egui::vec2(1280.0, 2400.0));
    frames(&mut h, 2);
    if connected {
        let state = report
            .trim_start_matches('<')
            .split('|')
            .next()
            .unwrap_or_default()
            .to_string();
        assert!(
            step_until(&mut h, |app| machine_state(app).as_deref() == Some(&state)),
            "no {state} report"
        );
        frames(&mut h, 2);
    }
    (h, shared)
}

/// Steps until `cond` holds on the CAM pane.
pub fn until(
    h: &mut Harness<'static, GcodeKitApp>,
    what: &str,
    cond: impl Fn(&GcodeKitApp) -> bool,
) {
    assert!(step_until(h, cond), "timed out waiting for {what}");
}

/// The probe page's result text.
pub fn result(app: &GcodeKitApp) -> String {
    app.cam_tools()
        .borrow()
        .probe_result_text()
        .unwrap_or_default()
        .to_string()
}

/// Steps frames for `ms` milliseconds.
pub fn settle(h: &mut Harness<'static, GcodeKitApp>, ms: u64) {
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        h.step();
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Runs a Z-touch whose two probe reports are `prb` and returns the app after the result.
pub fn z_touch_with(
    h: &mut Harness<'static, GcodeKitApp>,
    shared: &LoopbackShared,
    prb: &str,
) -> Vec<String> {
    let before = lines(shared).len();
    click(h, "Probe");
    let program = gtk_run_lines(&fixture("probe_z_touch.gcode"));
    until(h, "the program", |_| {
        lines(shared).len() >= before + program.len()
    });
    assert_eq!(lines(shared)[before..], program[..], "streamed lines");
    reply(shared, prb);
    reply(shared, prb);
    shared.hold_acks.store(false, Ordering::SeqCst);
    until(h, "the result", |app| {
        app.cam_tools().borrow().probe_last_result().is_some()
    });
    lines(shared)[before..].to_vec()
}

#[test]
fn test_cam_probe_z_touch_reports_and_applies_in_mm() {
    let (mut h, shared) = probe_app(true, IDLE);
    z_touch_with(&mut h, &shared, "[PRB:1.000,2.000,-12.700:1]");
    let text = result(h.state());
    assert!(text.contains("Surface at machine Z-12.700"), "{text}");
    assert!(text.contains("2 probe report(s) received."), "{text}");
    until(&mut h, "Idle", |app| !app.state().machine.job_running());
    frames(&mut h, 2);
    click(&mut h, "Apply to WCS");
    until(&mut h, "the WCS line", |_| {
        lines(&shared).last().map(String::as_str) == Some("G10 L2 P1 Z-12.700")
    });
}

#[test]
fn test_cam_probe_z_touch_reads_inch_reports_in_mm() {
    let (mut h, shared) = probe_app(true, IDLE);
    reply(&shared, "$13=1");
    until(&mut h, "$13=1", |app| {
        app.state().machine.status().report_inches
    });
    // The DRO waits for the next report after a $13 change.
    until(&mut h, "a report", |app| {
        machine_state(app).as_deref() == Some("Idle")
    });
    frames(&mut h, 2);
    z_touch_with(&mut h, &shared, "[PRB:0.0394,0.0787,-0.5000:1]");
    let text = result(h.state());
    assert!(text.contains("Surface at machine Z-12.700"), "{text}");
    until(&mut h, "Idle", |app| !app.state().machine.job_running());
    frames(&mut h, 2);
    click(&mut h, "Apply to WCS");
    until(&mut h, "the WCS line", |_| {
        lines(&shared).last().map(String::as_str) == Some("G10 L2 P1 Z-12.700")
    });
}

#[test]
fn test_cam_probe_follows_a_report_inches_change_during_the_run() {
    let (mut h, shared) = probe_app(true, IDLE);
    // `$13=1` arrives after the run started, before its probe reports.
    z_touch_with(&mut h, &shared, "$13=1\n[PRB:0.0394,0.0787,-0.5000:1]");
    let text = result(h.state());
    assert!(text.contains("Surface at machine Z-12.700"), "{text}");
}

#[test]
fn test_cam_probe_is_refused_unless_the_machine_is_ready() {
    let cases = [
        (false, IDLE, "The machine is not connected."),
        (true, ALARM, "The machine must be Idle (Alarm)"),
        (
            true,
            "<Idle|MPos:1.000,2.000,3.000|FS:0,0|Pn:P>",
            "The probe input is already triggered.",
        ),
    ];
    for (connected, report, reason) in cases {
        let (mut h, shared) = probe_app(connected, report);
        assert!(h.query_by_label(reason).is_some(), "{reason} not shown");
        let before = lines(&shared).len();
        click(&mut h, "Probe");
        settle(&mut h, 100);
        assert_eq!(lines(&shared).len(), before, "{reason}: lines were sent");
        assert!(!pane(&h).probe_running(), "{reason}: a run started");
    }
}

#[test]
fn test_cam_probe_is_refused_while_a_job_runs() {
    let (mut state, shared) = loopback_state(Duration::from_millis(1));
    reply(&shared, IDLE);
    start_held_stream(&mut state, &shared, 50);
    let (mut h, _script) = cam_app(state);
    h.get_by_label("Probe Tools").click();
    frames(&mut h, 1);
    h.get_by_label("Open").click();
    h.set_size(egui::vec2(1280.0, 2400.0));
    frames(&mut h, 3);
    assert!(h.query_by_label("A job is running.").is_some());
    let before = lines(&shared).len();
    click(&mut h, "Probe");
    settle(&mut h, 100);
    assert_eq!(lines(&shared).len(), before);
    assert!(!pane(&h).probe_running());
}

#[test]
fn test_cam_probe_apply_waits_for_a_successful_run() {
    let (mut h, shared) = probe_app(true, IDLE);
    let before = lines(&shared).len();
    click(&mut h, "Apply to WCS");
    settle(&mut h, 100);
    assert_eq!(lines(&shared).len(), before);
}

/// Starts the boss-centre routine (longer than the controller's buffer) and waits until
/// the first lines are written; returns how many lines were written before the click and
/// the routine's lines.
fn start_boss(
    h: &mut Harness<'static, GcodeKitApp>,
    shared: &LoopbackShared,
) -> (usize, Vec<String>) {
    pane(h).probe_mut().routine = RoutineKind::BossCenter;
    frames(h, 2);
    let before = lines(shared).len();
    click(h, "Probe");
    until(h, "the first lines", |_| lines(shared).len() > before);
    settle(h, 50);
    assert!(pane(h).probe_running());
    let program = gtk_run_lines(&fixture("probe_boss_center.gcode"));
    let written = lines(shared).len() - before;
    assert!(written < program.len(), "the whole routine fit the buffer");
    (before, program)
}

/// After a run stopped: releasing the acks sends no more of the routine, nothing is
/// restored while the machine is not Idle, and G90 is sent once it is.
fn check_stopped_then_restored(
    h: &mut Harness<'static, GcodeKitApp>,
    shared: &LoopbackShared,
    before: usize,
    program: &[String],
) {
    until(h, "the failure", |app| {
        !app.cam_tools().borrow().probe_running_stream()
    });
    let written = lines(shared).len();
    shared.hold_acks.store(false, Ordering::SeqCst);
    settle(h, 300);
    let after: Vec<String> = lines(shared)[before..].to_vec();
    assert_eq!(
        after.len(),
        written - before,
        "more of the routine was sent: {after:?}"
    );
    assert!(after.iter().all(|l| program.contains(l)), "{after:?}");
    assert!(
        result(h.state()).contains("Probe failed"),
        "{}",
        result(h.state())
    );
    reply(shared, IDLE);
    until(h, "G90", |_| {
        lines(shared).last().map(String::as_str) == Some("G90")
    });
    until(h, "the run's end", |app| {
        !app.cam_tools().borrow().probe_running()
    });
}

#[test]
fn test_cam_probe_alarm_stops_the_routine_and_restores_g90_when_idle() {
    let (mut h, shared) = probe_app(true, IDLE);
    let (before, program) = start_boss(&mut h, &shared);
    reply(&shared, ALARM);
    reply(&shared, "ALARM:5");
    check_stopped_then_restored(&mut h, &shared, before, &program);
}

#[test]
fn test_cam_probe_user_stop_mid_routine_stops_sending() {
    let (mut h, shared) = probe_app(true, IDLE);
    let (before, program) = start_boss(&mut h, &shared);
    reply(&shared, ALARM);
    // The Machine Control pane's Stop.
    lock(h.state().state().machine.controller()).stop();
    check_stopped_then_restored(&mut h, &shared, before, &program);
    assert!(reset_written(&shared));
}

#[test]
fn test_cam_probe_stop_button_resets_the_controller() {
    let (mut h, shared) = probe_app(true, IDLE);
    let (before, program) = start_boss(&mut h, &shared);
    reply(&shared, ALARM);
    click(&mut h, "Stop Probe");
    assert!(reset_written(&shared), "no soft reset");
    check_stopped_then_restored(&mut h, &shared, before, &program);
}

#[test]
fn test_cam_probe_error_reply_stops_the_routine() {
    let (mut h, shared) = probe_app(true, IDLE);
    let (before, program) = start_boss(&mut h, &shared);
    reply(&shared, ALARM);
    reply(&shared, "error:9");
    until(&mut h, "the soft reset", |_| reset_written(&shared));
    check_stopped_then_restored(&mut h, &shared, before, &program);
}

#[test]
fn test_cam_probe_disconnect_aborts_the_run() {
    let (mut h, shared) = probe_app(true, IDLE);
    let (before, _program) = start_boss(&mut h, &shared);
    lock(h.state().state().machine.controller()).disconnect();
    until(&mut h, "the abort", |app| {
        !app.cam_tools().borrow().probe_running()
    });
    assert!(
        result(h.state()).contains("Probe aborted: the machine was disconnected."),
        "{}",
        result(h.state())
    );
    assert!(!lines(&shared)[before..].iter().any(|l| l == "G90"));
}
