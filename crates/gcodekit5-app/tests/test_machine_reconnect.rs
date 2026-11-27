//! Tests for Auto Reconnect in `MachineController`.
//!
//! The controller runs against the in-memory loopback transport of
//! `gcodekit5_app::test_support`; setting `LoopbackShared::link_down` drops the link the
//! way a pulled cable or a rebooting board does. The first reconnect attempt comes one
//! second after the drop.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use gcodekit5_app::machine::{MachineController, MachineEvent, worker_text};
use gcodekit5_app::test_support::{LoopbackShared, Wrote, loopback_controller, tcp_settings};
use gcodekit5_settings::ConnectionSettings;

/// Upper bound for "eventually" waits; generous so a loaded machine is not flaky.
const EVENTUALLY: Duration = Duration::from_secs(10);

/// Longer than the first reconnect delay (1 s): a reconnect would have been tried by then.
const PAST_FIRST_ATTEMPT: Duration = Duration::from_millis(1600);

fn program(lines: usize) -> Arc<[String]> {
    (0..lines)
        .map(|i| format!("G1 X{i} F1000"))
        .collect::<Vec<_>>()
        .into()
}

/// Polls until an event matching `pred` arrives or [`EVENTUALLY`] passes, and returns every
/// event seen up to and including it.
fn events_until(
    controller: &mut MachineController,
    pred: impl Fn(&MachineEvent) -> bool,
) -> Vec<MachineEvent> {
    let deadline = Instant::now() + EVENTUALLY;
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        for event in controller.poll() {
            let found = pred(&event);
            seen.push(event);
            if found {
                return seen;
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("expected event did not arrive; saw {seen:?}");
}

/// Every event that arrives within `period`.
fn events_for(controller: &mut MachineController, period: Duration) -> Vec<MachineEvent> {
    let deadline = Instant::now() + period;
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        seen.extend(controller.poll());
        std::thread::sleep(Duration::from_millis(5));
    }
    seen
}

fn connect(controller: &mut MachineController, settings: &ConnectionSettings) {
    controller.connect(settings).expect("connect");
    events_until(controller, |e| *e == MachineEvent::Connected);
}

fn reconnect_settings(auto_reconnect: bool) -> ConnectionSettings {
    ConnectionSettings {
        auto_reconnect,
        ..tcp_settings()
    }
}

fn is_reconnecting(e: &MachineEvent) -> bool {
    matches!(e, MachineEvent::Reconnecting { .. })
}

#[test]
fn test_machine_reconnect_after_drop() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    connect(&mut controller, &reconnect_settings(true));

    shared.link_down.store(true, Ordering::SeqCst);
    let seen = events_until(&mut controller, is_reconnecting);
    let disconnected = seen
        .iter()
        .position(|e| *e == MachineEvent::Disconnected)
        .expect("the drop is reported as Disconnected");
    assert!(
        matches!(
            seen[disconnected + 1],
            MachineEvent::Reconnecting {
                attempt: 1,
                max_attempts
            } if max_attempts > 1
        ),
        "{seen:?}"
    );

    // The board comes back: the next attempt reopens the connection.
    shared.link_down.store(false, Ordering::SeqCst);
    events_until(&mut controller, |e| *e == MachineEvent::Connected);
    assert_eq!(shared.connects.load(Ordering::SeqCst), 2);

    // The reopened connection works.
    controller.send_line("G0 X1").expect("send after reconnect");
    let deadline = Instant::now() + EVENTUALLY;
    while !shared.log().contains(&Wrote::Line("G0 X1".to_string())) {
        assert!(
            Instant::now() < deadline,
            "line not written after reconnect"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    controller.disconnect();
}

#[test]
fn test_machine_reconnect_off_closes_on_drop() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    connect(&mut controller, &reconnect_settings(false));

    shared.link_down.store(true, Ordering::SeqCst);
    events_until(&mut controller, |e| *e == MachineEvent::Disconnected);
    shared.link_down.store(false, Ordering::SeqCst);
    let later = events_for(&mut controller, PAST_FIRST_ATTEMPT);
    assert!(!later.iter().any(is_reconnecting), "{later:?}");
    assert!(!later.contains(&MachineEvent::Connected), "{later:?}");
    assert_eq!(shared.connects.load(Ordering::SeqCst), 1);
}

#[test]
fn test_machine_reconnect_not_after_user_disconnect() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    connect(&mut controller, &reconnect_settings(true));

    controller.disconnect();
    let seen = events_for(&mut controller, PAST_FIRST_ATTEMPT);
    assert!(seen.contains(&MachineEvent::Disconnected), "{seen:?}");
    assert!(!seen.iter().any(is_reconnecting), "{seen:?}");
    assert!(!seen.contains(&MachineEvent::Connected), "{seen:?}");
    assert_eq!(shared.connects.load(Ordering::SeqCst), 1);
}

#[test]
fn test_machine_reconnect_cancelled_by_user_disconnect() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    connect(&mut controller, &reconnect_settings(true));

    shared.link_down.store(true, Ordering::SeqCst);
    events_until(&mut controller, is_reconnecting);

    // A program started while the link is down is refused, never queued for later.
    controller
        .start_stream(program(3))
        .expect("the worker still runs");
    let seen = events_until(&mut controller, |e| {
        matches!(e, MachineEvent::StreamStopped { .. })
    });
    assert!(
        seen.contains(&MachineEvent::StreamStopped { acked: 0, total: 3 }),
        "{seen:?}"
    );
    assert!(
        seen.contains(&MachineEvent::Error(worker_text::NOT_CONNECTED.to_string())),
        "{seen:?}"
    );
    assert!(!controller.is_streaming());

    // Disconnect while waiting to retry: reported at once, and no attempt follows.
    controller.disconnect();
    assert!(controller.poll().contains(&MachineEvent::Disconnected));
    shared.link_down.store(false, Ordering::SeqCst);
    std::thread::sleep(PAST_FIRST_ATTEMPT);
    assert_eq!(shared.connects.load(Ordering::SeqCst), 1);
    assert!(controller.poll().is_empty());
}

#[test]
fn test_machine_reconnect_drop_mid_stream_stops_job() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(20));
    connect(&mut controller, &reconnect_settings(true));

    let lines = program(200);
    controller
        .start_stream(lines.clone())
        .expect("start_stream");
    events_until(
        &mut controller,
        |e| matches!(e, MachineEvent::StreamProgress { acked, .. } if *acked > 0),
    );

    shared.link_down.store(true, Ordering::SeqCst);
    // Nothing is written while the link is down, so later writes come after the reconnect.
    let written_before = shared.log().len();
    let seen = events_until(&mut controller, is_reconnecting);
    let job_stopped = seen
        .iter()
        .position(|e| {
            *e == MachineEvent::Error(worker_text::JOB_STOPPED_CONNECTION_LOST.to_string())
        })
        .expect("the user is told the job stopped");
    let stopped = seen
        .iter()
        .position(|e| matches!(e, MachineEvent::StreamStopped { total: 200, .. }))
        .expect("the stream is reported stopped");
    let disconnected = seen
        .iter()
        .position(|e| *e == MachineEvent::Disconnected)
        .expect("the drop is reported");
    assert!(job_stopped < stopped && stopped < disconnected, "{seen:?}");
    assert!(!controller.is_streaming());

    shared.link_down.store(false, Ordering::SeqCst);
    events_until(&mut controller, |e| *e == MachineEvent::Connected);
    let after = events_for(&mut controller, Duration::from_millis(500));
    assert!(
        !after.iter().any(|e| matches!(
            e,
            MachineEvent::StreamProgress { .. } | MachineEvent::StreamFinished { .. }
        )),
        "the job resumed: {after:?}"
    );
    let resumed: Vec<Wrote> = shared.log()[written_before..]
        .iter()
        .filter(|w| matches!(w, Wrote::Line(l) if lines.contains(l)))
        .cloned()
        .collect();
    assert!(
        resumed.is_empty(),
        "program lines written after the reconnect: {resumed:?}"
    );
    controller.disconnect();
}
