//! `MachineController::lines_answered`: whether every line sent with `send_line` has had
//! its reply (`ok` or `error:N`) or was dropped by a reset. A front end that infers a
//! setting write's answer from the order of replies (the controller reports no event for a
//! manual `ok`) may only start the write when nothing earlier is still unanswered.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use gcodekit5_app::machine::{MachineController, MachineEvent};
use gcodekit5_app::test_support::{LoopbackShared, Wrote, loopback_controller, tcp_settings};

const EVENTUALLY: Duration = Duration::from_secs(10);

/// Polls `controller` until `cond` holds or [`EVENTUALLY`] passes.
fn wait(controller: &mut MachineController, cond: impl Fn(&MachineController) -> bool) -> bool {
    let deadline = Instant::now() + EVENTUALLY;
    while Instant::now() < deadline {
        controller.poll();
        if cond(controller) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    false
}

fn connected() -> (MachineController, Arc<LoopbackShared>) {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    controller.connect(&tcp_settings()).expect("connect");
    let deadline = Instant::now() + EVENTUALLY;
    while !controller.poll().contains(&MachineEvent::Connected) {
        assert!(Instant::now() < deadline, "no Connected event");
        std::thread::sleep(Duration::from_millis(1));
    }
    (controller, shared)
}

fn written(shared: &LoopbackShared, line: &str) -> bool {
    shared.log().contains(&Wrote::Line(line.to_string()))
}

#[test]
fn test_lines_answered_waits_for_the_ok() {
    let (mut controller, shared) = connected();
    assert!(controller.lines_answered());
    shared.hold_acks.store(true, Ordering::SeqCst);
    controller.send_line("$$").expect("send");
    assert!(!controller.lines_answered(), "queued, not answered");
    assert!(wait(&mut controller, |_| written(&shared, "$$")));
    std::thread::sleep(Duration::from_millis(20));
    controller.poll();
    assert!(!controller.lines_answered(), "written, not answered");
    shared.hold_acks.store(false, Ordering::SeqCst);
    assert!(wait(&mut controller, MachineController::lines_answered));
}

#[test]
fn test_lines_answered_counts_an_error_reply() {
    let (mut controller, shared) = connected();
    shared.hold_acks.store(true, Ordering::SeqCst);
    controller.send_line("G4 P0").expect("send");
    controller.send_line("$I").expect("send");
    assert!(wait(&mut controller, |_| written(&shared, "$I")));
    shared.inject("error:20\n");
    std::thread::sleep(Duration::from_millis(20));
    controller.poll();
    assert!(
        !controller.lines_answered(),
        "only the first line is answered"
    );
    shared.inject("ok\n");
    assert!(wait(&mut controller, MachineController::lines_answered));
}

#[test]
fn test_lines_answered_after_a_reset_drops_them() {
    let (mut controller, shared) = connected();
    shared.hold_acks.store(true, Ordering::SeqCst);
    controller.send_line("$$").expect("send");
    assert!(wait(&mut controller, |_| written(&shared, "$$")));
    shared.inject("Grbl 1.1h ['$' for help]\n");
    assert!(wait(&mut controller, MachineController::lines_answered));
}

fn jogs_written(shared: &LoopbackShared) -> usize {
    shared
        .log()
        .iter()
        .filter(|w| matches!(w, Wrote::Line(l) if l.starts_with("$J=")))
        .count()
}

#[test]
fn test_lines_answered_waits_for_a_jogs_reply() {
    use gcodekit5_app::machine::Axis;
    let (mut controller, shared) = connected();
    shared.hold_acks.store(true, Ordering::SeqCst);
    controller.jog(Axis::X, 5.0, 1000.0);
    assert!(!controller.lines_answered(), "a queued jog is unanswered");
    assert!(wait(&mut controller, |_| jogs_written(&shared) == 1));
    std::thread::sleep(Duration::from_millis(20));
    controller.poll();
    assert!(!controller.lines_answered(), "a written jog is unanswered");
    // A soft-limit refusal answers it: a setting write sent next can own the next reply.
    shared.inject("error:15\n");
    assert!(wait(&mut controller, MachineController::lines_answered));
}

#[test]
fn test_lines_answered_after_a_jog_cancel_drops_queued_jogs() {
    use gcodekit5_app::machine::{Axis, RealtimeCommand};
    let (mut controller, shared) = connected();
    shared.hold_acks.store(true, Ordering::SeqCst);
    // More jogs than fit the controller's buffer: some wait in the worker's queue.
    for _ in 0..12 {
        controller.jog(Axis::X, 1.0, 1000.0);
    }
    assert!(wait(&mut controller, |_| jogs_written(&shared) > 0));
    std::thread::sleep(Duration::from_millis(20));
    controller.poll();
    assert!(jogs_written(&shared) < 12, "every jog was written");
    controller.realtime(RealtimeCommand::JogCancel);
    // The jogs in flight are acknowledged; the queued ones never run.
    shared.hold_acks.store(false, Ordering::SeqCst);
    assert!(wait(&mut controller, MachineController::lines_answered));
    assert!(
        jogs_written(&shared) < 12,
        "a queued jog ran after the cancel"
    );
}

#[test]
fn test_lines_answered_after_a_reset_drops_jogs() {
    use gcodekit5_app::machine::Axis;
    let (mut controller, shared) = connected();
    shared.hold_acks.store(true, Ordering::SeqCst);
    for _ in 0..12 {
        controller.jog(Axis::Y, 1.0, 1000.0);
    }
    assert!(wait(&mut controller, |_| jogs_written(&shared) > 0));
    // Some jogs may still be in the worker's channel when the banner is read: they were
    // sent before the reset, so they are dropped too. One written after the reset would,
    // with acks held, never be answered.
    shared.inject("Grbl 1.1h ['$' for help]\n");
    assert!(wait(&mut controller, MachineController::lines_answered));
}

/// A jog sent before the controller reset itself, but still in the worker's channel when
/// the reset banner was read, is dropped rather than written after the reset.
#[test]
fn test_jog_sent_before_a_reset_never_runs_after_it() {
    use gcodekit5_app::machine::Axis;
    let (mut controller, shared) = connected();
    shared.hold_acks.store(true, Ordering::SeqCst);
    let before_reset = Instant::now();
    shared.inject("Grbl 1.1h ['$' for help]\n");
    assert!(
        wait_event(&mut controller, |e| matches!(
            e,
            MachineEvent::Received(line) if line.starts_with("Grbl")
        )),
        "banner never read"
    );
    controller.jog_sent_earlier(Axis::Y, 1.0, 1000.0, before_reset);
    assert!(wait(&mut controller, MachineController::lines_answered));
    assert_eq!(jogs_written(&shared), 0, "{:?}", shared.log());
}

/// A line sent in answer to the reset banner (the UI's `$G`) is written, never dropped as
/// sent before the reset: the worker stamps the reset before it reports the banner. The
/// repaint hook, called by the worker right after each event, holds the worker for a while
/// so the line is sent before the worker could stamp the reset afterwards.
#[test]
fn test_line_sent_in_answer_to_a_reset_banner_is_written() {
    use std::sync::atomic::AtomicBool;
    let (mut controller, shared) = connected();
    let stall = Arc::new(AtomicBool::new(false));
    let hook_stall = Arc::clone(&stall);
    controller.set_repaint_hook(Box::new(move || {
        if hook_stall.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(200));
        }
    }));
    stall.store(true, Ordering::SeqCst);
    shared.inject("Grbl 1.1h ['$' for help]\n");
    assert!(
        wait_event(&mut controller, |e| matches!(
            e,
            MachineEvent::Received(line) if line.starts_with("Grbl")
        )),
        "banner never read"
    );
    controller.send_query("$G").expect("send $G");
    stall.store(false, Ordering::SeqCst);
    assert!(wait(&mut controller, MachineController::lines_answered));
    assert!(written(&shared, "$G"), "$G dropped: {:?}", shared.log());
}

/// Polls `controller` until an event matching `cond` arrives or [`EVENTUALLY`] passes.
fn wait_event(controller: &mut MachineController, cond: impl Fn(&MachineEvent) -> bool) -> bool {
    let deadline = Instant::now() + EVENTUALLY;
    while Instant::now() < deadline {
        if controller.poll().iter().any(&cond) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    false
}

#[test]
fn test_query_line_is_answered_but_keeps_the_report_fresh() {
    let (mut controller, shared) = connected();
    shared.hold_acks.store(true, Ordering::SeqCst);
    controller.send_query("$G").expect("query");
    // `$G` changes nothing on the machine: the last report still describes it.
    assert!(controller.report_fresh());
    assert!(!controller.lines_answered(), "queued, not answered");
    assert!(wait(&mut controller, |_| written(&shared, "$G")));
    shared.hold_acks.store(false, Ordering::SeqCst);
    assert!(wait(&mut controller, MachineController::lines_answered));
    assert!(controller.report_fresh());
}

#[test]
fn test_query_line_has_send_line_checks() {
    let mut offline = MachineController::new();
    assert_eq!(
        offline.send_query("$G"),
        Err(gcodekit5_app::machine::MachineError::NotConnected)
    );
    let (mut controller, _shared) = connected();
    assert_eq!(
        controller.send_query(""),
        Err(gcodekit5_app::machine::MachineError::EmptyLine)
    );
    assert_eq!(
        controller.send_query("$G?"),
        Err(gcodekit5_app::machine::MachineError::RealtimeCharacter('?'))
    );
}
