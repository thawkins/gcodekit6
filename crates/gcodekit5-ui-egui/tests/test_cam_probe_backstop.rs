//! The probe run's backstop: after an `error:` reply the controller halts the routine
//! itself (feed hold, then a reset once the machine has stopped, up to `HALT_MAX_WAIT`).
//! The run waits the long [`BACKSTOP_DELAY`] only once the controller has confirmed the halt
//! (its feed hold echoed as sent); without that it resets the controller itself after the
//! short [`UNCONFIRMED_BACKSTOP_DELAY`]. The backstop fires even when no machine event
//! arrives: the app hands the run an empty batch when [`ProbeRun::backstop_in`] says it is
//! due, and asks for a repaint then.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use gcodekit5_app::machine::{MachineEvent, realtime_label};
use gcodekit5_app::test_support::LoopbackShared;
use gcodekit5_ui_egui::AppState;
use gcodekit5_ui_egui::panes::cam_tools::probe_routines::ProbeParams;
use gcodekit5_ui_egui::panes::cam_tools::probe_routines::run::{
    BACKSTOP_DELAY, ProbeRun, UNCONFIRMED_BACKSTOP_DELAY,
};

use crate::machine_support::{EVENTUALLY, connect, loopback_state, wait_machine, wait_until};
use crate::test_cam_probe_run::{IDLE, reply, reset_written};

/// A probe run started on a connected, Idle loopback whose acks are held.
fn started_run() -> (ProbeRun, AppState, Arc<LoopbackShared>) {
    let (mut state, shared) = loopback_state(Duration::from_millis(1));
    shared.hold_acks.store(true, Ordering::SeqCst);
    reply(&shared, IDLE);
    connect(&mut state);
    assert!(
        wait_machine(&mut state.machine, |s| s.report.is_some()),
        "no status report"
    );
    let deadline = Instant::now() + EVENTUALLY;
    let mut ready = false;
    while !ready && Instant::now() < deadline {
        state.machine.poll();
        ready = state.machine.jog_allowed();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(ready, "the report never became fresh");
    let (run, _) = ProbeRun::start(
        Some(&state.machine),
        &ProbeParams::default(),
        &state.translator,
    )
    .expect("start");
    (run, state, shared)
}

fn refused() -> MachineEvent {
    MachineEvent::Error("error:9 on program line 1".to_string())
}

#[test]
fn test_cam_probe_backstop_resets_soon_without_a_confirmed_halt() {
    let (mut run, state, shared) = started_run();
    run.handle(&[refused()], &state.translator);
    let due = run.backstop_in().expect("a backstop is armed");
    assert!(due <= UNCONFIRMED_BACKSTOP_DELAY, "{due:?}");
    std::thread::sleep(UNCONFIRMED_BACKSTOP_DELAY + Duration::from_millis(100));
    assert_eq!(run.backstop_in(), Some(Duration::ZERO));
    // No machine event at all (the controller went quiet): the frame's empty batch fires it.
    run.handle(&[], &state.translator);
    assert!(
        wait_until(|| reset_written(&shared)),
        "the backstop did not reset an unconfirmed halt"
    );
}

#[test]
fn test_cam_probe_backstop_waits_while_the_controller_halts() {
    let (mut run, state, shared) = started_run();
    let hold = realtime_label(b'!').expect("feed hold label").to_string();
    run.handle(&[refused(), MachineEvent::Sent(hold)], &state.translator);
    let due = run.backstop_in().expect("a backstop is armed");
    assert!(due > UNCONFIRMED_BACKSTOP_DELAY, "{due:?}");
    std::thread::sleep(UNCONFIRMED_BACKSTOP_DELAY + Duration::from_millis(100));
    run.handle(&[], &state.translator);
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        !reset_written(&shared),
        "the backstop reset a machine the controller is still halting"
    );
    assert!(BACKSTOP_DELAY > UNCONFIRMED_BACKSTOP_DELAY);
}

#[test]
fn test_cam_probe_backstop_fires_in_the_app_with_no_machine_events() {
    use crate::test_cam_drill_press::click;
    use crate::test_cam_probe_run::{lines, probe_app, settle};

    let (mut h, shared) = probe_app(true, IDLE);
    click(&mut h, "Probe");
    crate::test_cam_probe_run::until(&mut h, "the routine", |_| !lines(&shared).is_empty());
    // The controller goes quiet: no status reply and no ack from now on.
    *shared.reply_budget.lock().expect("budget") = Some(0);
    settle(&mut h, 300);
    // A refused line the controller does not halt for (no feed hold echoed).
    h.state_mut().state_mut().machine.absorb(vec![refused()]);
    let deadline = std::time::Instant::now() + UNCONFIRMED_BACKSTOP_DELAY * 3;
    while !reset_written(&shared) && std::time::Instant::now() < deadline {
        h.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(reset_written(&shared), "the backstop never fired");
}
