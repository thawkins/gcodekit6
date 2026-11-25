//! A stream started in [`StreamMode::StopOnError`] (the probe routines' mode): an `error:`
//! reply makes the worker stop feeding lines at once, feed-hold (`!`), wait for the hold to
//! complete (`Hold:0` or `Idle`, polled with `?`) for up to a second, then soft-reset
//! (0x18), and report why with [`MachineEvent::StreamHalted`] before the stream's end. The
//! loopback acks every line (acks are not held), so the worker is feeding when the error
//! arrives. A normal stream keeps streaming on an error.

use std::sync::Arc;
use std::time::{Duration, Instant};

use gcodekit5_app::machine::{MachineController, MachineEvent, StreamMode};
use gcodekit5_app::test_support::{LoopbackShared, Wrote, loopback_controller, tcp_settings};

const EVENTUALLY: Duration = Duration::from_secs(10);

/// A 60-line program with a `G10 L20` at line 40, like a probe routine's WCS write.
fn program() -> Arc<[String]> {
    (0..60)
        .map(|i| {
            if i == 40 {
                "G10 L20 P1 Z0".to_string()
            } else {
                format!("G1 X{i} F1000")
            }
        })
        .collect::<Vec<_>>()
        .into()
}

/// A connected controller over a loopback that acks each line after 15 ms.
fn connected() -> (Arc<LoopbackShared>, MachineController) {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(15));
    controller.connect(&tcp_settings()).expect("connect");
    let deadline = Instant::now() + EVENTUALLY;
    while Instant::now() < deadline {
        if controller.poll().contains(&MachineEvent::Connected) {
            return (shared, controller);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("no Connected event");
}

/// Polls `controller` into `events` until `cond` holds on the loopback's log.
fn until(
    controller: &mut MachineController,
    events: &mut Vec<MachineEvent>,
    what: &str,
    cond: impl Fn() -> bool,
) {
    let deadline = Instant::now() + EVENTUALLY;
    while Instant::now() < deadline {
        events.extend(controller.poll());
        if cond() {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("timed out waiting for {what}");
}

fn lines(shared: &LoopbackShared) -> usize {
    shared.lines_written()
}

/// The position of the first `byte` written in real time.
fn realtime_at(log: &[Wrote], byte: u8) -> Option<usize> {
    log.iter().position(|w| *w == Wrote::Realtime(byte))
}

/// Starts the program in stop-on-error mode and injects `error:9` once 5 lines are out.
fn error_mid_stream(
    shared: &LoopbackShared,
    controller: &mut MachineController,
    events: &mut Vec<MachineEvent>,
) {
    controller
        .start_stream_with(program(), StreamMode::StopOnError)
        .expect("start");
    until(controller, events, "5 lines", || lines(shared) >= 5);
    shared.inject("error:9\n");
    until(controller, events, "the feed hold", || {
        realtime_at(&shared.log(), b'!').is_some()
    });
}

/// No program line follows the hold, the hold precedes the reset, and no G10 was sent.
fn check_halt(shared: &LoopbackShared) {
    let log = shared.log();
    let hold = realtime_at(&log, b'!').expect("a feed hold");
    let reset = realtime_at(&log, 0x18).expect("a soft reset");
    assert!(hold < reset, "reset before hold: {log:?}");
    assert!(
        !log[hold..].iter().any(|w| matches!(w, Wrote::Line(_))),
        "a line was written after the hold: {:?}",
        &log[hold..]
    );
    assert!(
        !log.iter()
            .any(|w| matches!(w, Wrote::Line(l) if l.starts_with("G10"))),
        "the G10 was sent"
    );
}

#[test]
fn test_probe_stream_error_holds_then_resets_once_held() {
    let (shared, mut controller) = connected();
    let mut events = Vec::new();
    error_mid_stream(&shared, &mut controller, &mut events);
    let held = Instant::now();
    // The controller reports the hold complete.
    shared.inject("<Hold:0|MPos:0.000,0.000,0.000|FS:0,0>\n");
    until(&mut controller, &mut events, "the reset", || {
        realtime_at(&shared.log(), 0x18).is_some()
    });
    assert!(
        held.elapsed() < Duration::from_millis(800),
        "the reset waited for the timeout: {:?}",
        held.elapsed()
    );
    let deadline = Instant::now() + EVENTUALLY;
    while !events
        .iter()
        .any(|e| matches!(e, MachineEvent::StreamStopped { .. }))
        && Instant::now() < deadline
    {
        events.extend(controller.poll());
        std::thread::sleep(Duration::from_millis(2));
    }
    check_halt(&shared);
    let halted = events.iter().position(
        |e| matches!(e, MachineEvent::StreamHalted { reason } if reason.contains("error:9")),
    );
    let stopped = events
        .iter()
        .position(|e| matches!(e, MachineEvent::StreamStopped { .. }));
    assert!(halted.is_some(), "no StreamHalted: {events:?}");
    assert!(halted < stopped, "{events:?}");
    assert!(!controller.is_streaming());
    // Nothing more is written once the acks of the lines in flight arrive.
    let written = lines(&shared);
    std::thread::sleep(Duration::from_millis(200));
    controller.poll();
    assert_eq!(lines(&shared), written);
}

#[test]
fn test_probe_stream_error_resets_after_the_timeout_without_a_hold_report() {
    let (shared, mut controller) = connected();
    let mut events = Vec::new();
    error_mid_stream(&shared, &mut controller, &mut events);
    let held = Instant::now();
    until(&mut controller, &mut events, "the reset", || {
        realtime_at(&shared.log(), 0x18).is_some()
    });
    let waited = held.elapsed();
    assert!(
        waited >= Duration::from_millis(800),
        "reset before the timeout: {waited:?}"
    );
    check_halt(&shared);
    // The worker asked for the hold's status while waiting.
    let log = shared.log();
    let hold = realtime_at(&log, b'!').unwrap_or_default();
    assert!(log[hold..].contains(&Wrote::Realtime(b'?')));
}

#[test]
fn test_normal_stream_streams_on_after_an_error() {
    let (shared, mut controller) = connected();
    let mut events = Vec::new();
    controller.start_stream(program()).expect("start");
    until(&mut controller, &mut events, "5 lines", || {
        lines(&shared) >= 5
    });
    shared.inject("error:9\n");
    let deadline = Instant::now() + EVENTUALLY;
    while controller.is_streaming() && Instant::now() < deadline {
        events.extend(controller.poll());
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!controller.is_streaming(), "the stream did not finish");
    let log = shared.log();
    assert_eq!(realtime_at(&log, b'!'), None);
    assert_eq!(realtime_at(&log, 0x18), None);
    assert_eq!(lines(&shared), 60);
}
