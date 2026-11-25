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
