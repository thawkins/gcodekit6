//! A stream started in [`StreamMode::StopOnError`] (the probe routines' mode): an `error:`
//! reply makes the worker stop feeding lines at once, feed-hold (`!`), wait for the hold to
//! complete (`Hold:0` or an `Alarm`, polled with `?`), then soft-reset (0x18), and report why
//! with [`MachineEvent::StreamHalted`] before the stream's end. The wait is a second without
//! a fresh report, extended while the machine reports `Hold:1` (still decelerating) up to
//! [`HALT_MAX_WAIT`]. Cycle start is refused and no manual line is written while halting.
//! The loopback acks every line (acks are not held), so the worker is feeding when the error
//! arrives. A normal stream keeps streaming on an error.

use std::sync::Arc;
use std::time::{Duration, Instant};

use gcodekit5_app::machine::{
    HALT_MAX_WAIT, MachineController, MachineError, MachineEvent, RealtimeCommand, StreamMode,
};
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
    connected_over(Arc::new(LoopbackShared::default()))
}

/// A connected controller over the loopback `shared` that acks each line after 15 ms.
fn connected_over(shared: Arc<LoopbackShared>) -> (Arc<LoopbackShared>, MachineController) {
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
    // Without any report the short timeout applies, not the deceleration cap.
    assert!(
        waited < Duration::from_secs(3),
        "the reset waited for the cap: {waited:?}"
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

/// Polls until a soft reset was written, then until the stream ended; returns how long
/// after `from` the reset was seen.
fn until_reset(
    shared: &LoopbackShared,
    controller: &mut MachineController,
    events: &mut Vec<MachineEvent>,
    from: Instant,
    limit: Duration,
) -> Duration {
    let deadline = Instant::now() + limit;
    while realtime_at(&shared.log(), 0x18).is_none() {
        assert!(Instant::now() < deadline, "no reset within {limit:?}");
        events.extend(controller.poll());
        std::thread::sleep(Duration::from_millis(1));
    }
    let waited = from.elapsed();
    let deadline = Instant::now() + EVENTUALLY;
    while !events
        .iter()
        .any(|e| matches!(e, MachineEvent::StreamStopped { .. }))
        && Instant::now() < deadline
    {
        events.extend(controller.poll());
        std::thread::sleep(Duration::from_millis(2));
    }
    waited
}

/// Keeps polling `controller` for `period`, asserting no soft reset is written.
fn no_reset_for(
    shared: &LoopbackShared,
    controller: &mut MachineController,
    events: &mut Vec<MachineEvent>,
    period: Duration,
) {
    let end = Instant::now() + period;
    while Instant::now() < end {
        events.extend(controller.poll());
        assert_eq!(
            realtime_at(&shared.log(), 0x18),
            None,
            "reset while the machine still decelerates"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn test_probe_stream_halt_refuses_cycle_start() {
    let (shared, mut controller) = connected();
    let mut events = Vec::new();
    error_mid_stream(&shared, &mut controller, &mut events);
    // Before any hold report: the user presses Resume, and the global Cycle Start.
    controller.resume();
    controller.realtime(RealtimeCommand::CycleStart);
    let from = Instant::now();
    until_reset(&shared, &mut controller, &mut events, from, EVENTUALLY);
    let log = shared.log();
    assert_eq!(
        realtime_at(&log, b'~'),
        None,
        "cycle start written: {log:?}"
    );
    check_halt(&shared);
    let refused = events
        .iter()
        .filter(|e| matches!(e, MachineEvent::Error(m) if m.contains("cycle start ignored")))
        .count();
    assert_eq!(refused, 2, "{events:?}");
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, MachineEvent::Sent(s) if s.starts_with('~'))),
        "cycle start echoed as sent: {events:?}"
    );
    assert!(!controller.is_streaming());
}

#[test]
fn test_probe_stream_halt_lets_stop_through() {
    let (shared, mut controller) = connected();
    let mut events = Vec::new();
    error_mid_stream(&shared, &mut controller, &mut events);
    let from = Instant::now();
    controller.realtime(RealtimeCommand::FeedHold);
    controller.stop();
    let waited = until_reset(&shared, &mut controller, &mut events, from, EVENTUALLY);
    assert!(
        waited < Duration::from_millis(500),
        "Stop waited for the halt: {waited:?}"
    );
    let holds = shared
        .log()
        .iter()
        .filter(|w| **w == Wrote::Realtime(b'!'))
        .count();
    assert_eq!(holds, 2, "the user's feed hold was not written");
    assert!(!controller.is_streaming());
}

#[test]
fn test_probe_stream_halt_writes_no_manual_line_and_reports_it_dropped() {
    let (shared, mut controller) = connected();
    let mut events = Vec::new();
    error_mid_stream(&shared, &mut controller, &mut events);
    // The public path refuses a line while the stream (and its halt) runs.
    assert_eq!(
        controller.send_line("G0 X5"),
        Err(MachineError::StreamInProgress)
    );
    // A line that reached the worker's queue anyway is not written into the held controller.
    controller.queue_line_unchecked("G0 X7");
    let from = Instant::now();
    until_reset(&shared, &mut controller, &mut events, from, EVENTUALLY);
    let log = shared.log();
    assert!(
        !log.iter()
            .any(|w| matches!(w, Wrote::Line(l) if l == "G0 X7")),
        "the queued line was written while halting: {log:?}"
    );
    let dropped = events.iter().position(
        |e| matches!(e, MachineEvent::Error(m) if m.contains("dropped") && m.contains("G0 X7")),
    );
    let stopped = events
        .iter()
        .position(|e| matches!(e, MachineEvent::StreamStopped { .. }));
    assert!(dropped.is_some(), "no drop reported: {events:?}");
    assert!(dropped < stopped, "{events:?}");
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, MachineEvent::Sent(s) if s == "G0 X7")),
        "the dropped line was shown as sent: {events:?}"
    );
}

#[test]
fn test_probe_stream_halt_waits_while_the_machine_decelerates() {
    let (shared, mut controller) = connected();
    let mut events = Vec::new();
    error_mid_stream(&shared, &mut controller, &mut events);
    // A slow machine: still decelerating well past the one-second timeout.
    shared.inject("<Hold:1|MPos:0.000,0.000,0.000|FS:500,0>\n");
    no_reset_for(
        &shared,
        &mut controller,
        &mut events,
        Duration::from_millis(1800),
    );
    shared.inject("<Hold:0|MPos:0.000,0.000,0.000|FS:0,0>\n");
    let from = Instant::now();
    let waited = until_reset(&shared, &mut controller, &mut events, from, EVENTUALLY);
    assert!(
        waited < Duration::from_millis(800),
        "the reset did not follow Hold:0: {waited:?}"
    );
    check_halt(&shared);
}

#[test]
fn test_probe_stream_halt_resets_at_the_cap_if_the_hold_never_completes() {
    let (shared, mut controller) = connected();
    let mut events = Vec::new();
    error_mid_stream(&shared, &mut controller, &mut events);
    let from = Instant::now();
    shared.inject("<Hold:1|MPos:0.000,0.000,0.000|FS:500,0>\n");
    let waited = until_reset(
        &shared,
        &mut controller,
        &mut events,
        from,
        HALT_MAX_WAIT + Duration::from_secs(5),
    );
    assert!(
        waited >= HALT_MAX_WAIT - Duration::from_millis(300),
        "reset before the cap: {waited:?}"
    );
    assert!(
        waited < HALT_MAX_WAIT + Duration::from_secs(2),
        "reset long after the cap: {waited:?}"
    );
    check_halt(&shared);
}

#[test]
fn test_probe_stream_halt_skips_every_report_owed_before_the_hold() {
    // Every `?` is owed an `Idle` (the machine before the hold), but no reply is read yet.
    let shared = Arc::new(LoopbackShared::default());
    *shared.reply_budget.lock().expect("budget") = Some(0);
    shared.inject("<Idle|MPos:0.000,0.000,0.000|FS:0,0>\n");
    let (shared, mut controller) = connected_over(shared);
    let mut events = Vec::new();
    // More `?`s outstanding than the worker tracks freshness tags for.
    until(&mut controller, &mut events, "5 status queries", || {
        shared.status_queries() >= 5
    });
    error_mid_stream(&shared, &mut controller, &mut events);
    let log = shared.log();
    let hold = realtime_at(&log, b'!').expect("a feed hold");
    let stale = log[..hold]
        .iter()
        .filter(|w| **w == Wrote::Realtime(b'?'))
        .count();
    assert!(stale > 3, "only {stale} queries before the hold");
    // Release only the stale replies: all `Idle` from before the hold.
    *shared.reply_budget.lock().expect("budget") = Some(stale);
    no_reset_for(
        &shared,
        &mut controller,
        &mut events,
        Duration::from_millis(400),
    );
    assert_eq!(
        *shared.reply_budget.lock().expect("budget"),
        Some(0),
        "the stale replies were not all read"
    );
    // A fresh report after the hold completes it: from Idle, `!` puts GRBL in `Hold:0` at
    // once. (Replies to `?`s written during the halt still say `Idle`: GRBL never reports
    // that after a hold, so they cannot complete it.)
    shared.inject("<Hold:0|MPos:0.000,0.000,0.000|FS:0,0>\n");
    *shared.reply_budget.lock().expect("budget") = None;
    let from = Instant::now();
    let waited = until_reset(&shared, &mut controller, &mut events, from, EVENTUALLY);
    assert!(
        waited < Duration::from_millis(500),
        "the fresh report did not reset: {waited:?}"
    );
    check_halt(&shared);
}

#[test]
fn test_probe_stream_halt_never_resets_on_a_stale_hold_complete() {
    // Every `?` so far is owed `Hold:0`, from an earlier user pause since resumed; no reply
    // is read yet.
    let shared = Arc::new(LoopbackShared::default());
    *shared.reply_budget.lock().expect("budget") = Some(0);
    shared.inject("<Hold:0|MPos:0.000,0.000,0.000|FS:0,0>\n");
    let (shared, mut controller) = connected_over(shared);
    let mut events = Vec::new();
    until(&mut controller, &mut events, "5 status queries", || {
        shared.status_queries() >= 5
    });
    // The machine runs again: every later `?` is owed a report of it decelerating.
    shared.inject("<Hold:1|MPos:0.000,0.000,0.000|FS:500,0>\n");
    until(&mut controller, &mut events, "a query owed Hold:1", || {
        shared.status_queries() >= 6
    });
    error_mid_stream(&shared, &mut controller, &mut events);
    let log = shared.log();
    let hold = realtime_at(&log, b'!').expect("a feed hold");
    let stale = log[..hold]
        .iter()
        .filter(|w| **w == Wrote::Realtime(b'?'))
        .count();
    assert!(stale > 5, "only {stale} queries before the hold");
    // Release every reply: the stale `Hold:0`s first, then `Hold:1`s. A stale `Hold:0` must
    // not reset the machine, which is still moving.
    *shared.reply_budget.lock().expect("budget") = None;
    no_reset_for(
        &shared,
        &mut controller,
        &mut events,
        Duration::from_millis(600),
    );
    // A fresh `Hold:0` completes the hold.
    shared.inject("<Hold:0|MPos:0.000,0.000,0.000|FS:0,0>\n");
    let from = Instant::now();
    let waited = until_reset(&shared, &mut controller, &mut events, from, EVENTUALLY);
    assert!(
        waited < Duration::from_millis(800),
        "the fresh Hold:0 did not reset: {waited:?}"
    );
    check_halt(&shared);
}

#[test]
fn test_probe_stream_halt_forgets_status_replies_that_never_came() {
    // No report injected yet: every `?` written is lost (never answered).
    let (shared, mut controller) = connected();
    let mut events = Vec::new();
    until(
        &mut controller,
        &mut events,
        "25 lost status queries",
        || shared.status_queries() >= 25,
    );
    error_mid_stream(&shared, &mut controller, &mut events);
    // From now on every `?` is answered: the machine is still decelerating. Lost replies
    // must not make the halt skip these fresh reports until its one-second timeout.
    shared.inject("<Hold:1|MPos:0.000,0.000,0.000|FS:500,0>\n");
    no_reset_for(
        &shared,
        &mut controller,
        &mut events,
        Duration::from_millis(1800),
    );
    shared.inject("<Hold:0|MPos:0.000,0.000,0.000|FS:0,0>\n");
    let from = Instant::now();
    let waited = until_reset(&shared, &mut controller, &mut events, from, EVENTUALLY);
    assert!(
        waited < Duration::from_millis(800),
        "the reset did not follow Hold:0: {waited:?}"
    );
    check_halt(&shared);
}

/// Polls until the stream ended, at most `limit`; returns how long that took.
fn until_stopped(
    controller: &mut MachineController,
    events: &mut Vec<MachineEvent>,
    limit: Duration,
) -> Duration {
    let from = Instant::now();
    while !events
        .iter()
        .any(|e| matches!(e, MachineEvent::StreamStopped { .. }))
    {
        assert!(from.elapsed() < limit, "no StreamStopped within {limit:?}");
        events.extend(controller.poll());
        std::thread::sleep(Duration::from_millis(1));
    }
    from.elapsed()
}

#[test]
fn test_probe_stream_halt_ends_on_a_user_soft_reset() {
    let (shared, mut controller) = connected();
    let mut events = Vec::new();
    error_mid_stream(&shared, &mut controller, &mut events);
    controller.realtime(RealtimeCommand::SoftReset);
    let waited = until_stopped(&mut controller, &mut events, EVENTUALLY);
    assert!(
        waited < Duration::from_millis(500),
        "the soft reset waited for the halt: {waited:?}"
    );
    assert!(!controller.is_streaming());
    // The halt is over: it does not reset a second time or report itself later.
    std::thread::sleep(Duration::from_millis(1500));
    events.extend(controller.poll());
    let resets = shared
        .log()
        .iter()
        .filter(|w| **w == Wrote::Realtime(0x18))
        .count();
    assert_eq!(resets, 1, "{:?}", shared.log());
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, MachineEvent::StreamHalted { .. })),
        "{events:?}"
    );
    check_halt(&shared);
}

#[test]
fn test_stream_ends_on_a_user_soft_reset() {
    let (shared, mut controller) = connected();
    let mut events = Vec::new();
    controller.start_stream(program()).expect("start");
    until(&mut controller, &mut events, "5 lines", || {
        lines(&shared) >= 5
    });
    controller.realtime(RealtimeCommand::SoftReset);
    until_stopped(&mut controller, &mut events, EVENTUALLY);
    assert!(!controller.is_streaming());
    let log = shared.log();
    let reset = realtime_at(&log, 0x18).expect("a soft reset");
    assert!(
        !log[reset..].iter().any(|w| matches!(w, Wrote::Line(_))),
        "a line was written after the reset: {:?}",
        &log[reset..]
    );
    let written = lines(&shared);
    std::thread::sleep(Duration::from_millis(200));
    controller.poll();
    assert_eq!(lines(&shared), written);
}

#[test]
fn test_probe_stream_halt_never_resets_on_a_late_stale_idle_over_a_slow_link() {
    // A slow link (a Wi-Fi or telnet bridge): every status reply arrives 2.6 s after its `?`.
    let shared = Arc::new(LoopbackShared::default());
    *shared.reply_delay.lock().expect("delay") = Duration::from_millis(2600);
    shared.inject("<Idle|MPos:0.000,0.000,0.000|FS:0,0>\n");
    let (shared, mut controller) = connected_over(shared);
    let mut events = Vec::new();
    // Replies are flowing, each owed `Idle` from before any hold.
    let settled = Instant::now() + Duration::from_millis(3500);
    until(&mut controller, &mut events, "replies flowing", || {
        Instant::now() >= settled
    });
    error_mid_stream(&shared, &mut controller, &mut events);
    // After the hold the machine decelerates; the late replies still say `Idle`.
    shared.inject("<Hold:1|MPos:0.000,0.000,0.000|FS:500,0>\n");
    no_reset_for(
        &shared,
        &mut controller,
        &mut events,
        Duration::from_millis(3500),
    );
    shared.inject("<Hold:0|MPos:0.000,0.000,0.000|FS:0,0>\n");
    let from = Instant::now();
    let waited = until_reset(&shared, &mut controller, &mut events, from, EVENTUALLY);
    assert!(
        waited < Duration::from_secs(4),
        "the reset did not follow Hold:0: {waited:?}"
    );
    check_halt(&shared);
}
