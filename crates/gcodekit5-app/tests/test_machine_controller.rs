//! Tests for the non-blocking `MachineController`.
//!
//! The controller runs against the in-memory loopback transport of
//! `gcodekit5_app::test_support`: it records every write in order and answers each complete
//! line with `ok` after a fixed delay, like a GRBL controller executing a short move.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use gcodekit5_app::machine::grbl_stream::MAX_LINE_LENGTH;
use gcodekit5_app::machine::{MachineController, MachineError, MachineEvent, RealtimeCommand};
use gcodekit5_app::test_support::{
    Loopback, LoopbackShared, Wrote, loopback_controller, tcp_settings,
};
use gcodekit5_communication::{Communicator, SerialCommunicator};
use gcodekit5_settings::ConnectionSettings;

/// Upper bound for "eventually" waits; generous so a loaded machine is not flaky.
const EVENTUALLY: Duration = Duration::from_secs(10);

fn program(lines: usize) -> Arc<[String]> {
    (0..lines)
        .map(|i| format!("G1 X{i} F1000"))
        .collect::<Vec<_>>()
        .into()
}

/// Polls until an event matching `pred` arrives or [`EVENTUALLY`] passes.
fn wait_for(
    controller: &mut MachineController,
    pred: impl Fn(&MachineEvent) -> bool,
) -> Option<MachineEvent> {
    let deadline = Instant::now() + EVENTUALLY;
    while Instant::now() < deadline {
        if let Some(event) = controller.poll().into_iter().find(|e| pred(e)) {
            return Some(event);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    None
}

/// Waits until `cond` holds or [`EVENTUALLY`] passes.
fn wait_until(cond: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + EVENTUALLY;
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    false
}

fn connect(controller: &mut MachineController) {
    controller.connect(&tcp_settings()).expect("connect");
    assert!(
        wait_for(controller, |e| *e == MachineEvent::Connected).is_some(),
        "no Connected event"
    );
}

#[test]
fn test_stream_does_not_block_caller() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(50));
    connect(&mut controller);

    let started = Instant::now();
    controller.start_stream(program(100)).expect("start_stream");
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(10),
        "start_stream blocked for {elapsed:?}"
    );

    // 100 acks at 50 ms each take about 5 s.
    let finished = wait_for(&mut controller, |e| {
        matches!(e, MachineEvent::StreamFinished { .. })
    });
    assert_eq!(finished, Some(MachineEvent::StreamFinished { lines: 100 }));
    assert_eq!(shared.lines_written(), 100);
}

#[test]
fn test_realtime_bypasses_queue() {
    let shared = Arc::new(LoopbackShared::default());
    shared.hold_acks.store(true, Ordering::SeqCst);
    let mut controller = loopback_controller(&shared, Duration::from_millis(50));
    connect(&mut controller);

    controller.start_stream(program(100)).expect("start_stream");
    // With acks held, the character-counting window fills and the rest of the
    // program waits in the queue.
    assert!(wait_until(|| shared.lines_written() > 0), "no line sent");

    controller.realtime(RealtimeCommand::FeedHold);
    assert!(
        wait_until(|| shared.log().contains(&Wrote::Realtime(b'!'))),
        "feed hold never reached the transport"
    );
    shared.hold_acks.store(false, Ordering::SeqCst);

    assert_eq!(
        wait_for(&mut controller, |e| {
            matches!(e, MachineEvent::StreamFinished { .. })
        }),
        Some(MachineEvent::StreamFinished { lines: 100 })
    );

    let log = shared.log();
    let hold_at = log
        .iter()
        .position(|w| *w == Wrote::Realtime(b'!'))
        .expect("feed hold logged");
    let lines_before = log[..hold_at]
        .iter()
        .filter(|w| matches!(w, Wrote::Line(_)))
        .count();
    let lines_after = log[hold_at..]
        .iter()
        .filter(|w| matches!(w, Wrote::Line(_)))
        .count();
    assert!(lines_before > 0, "feed hold sent before the stream began");
    assert!(
        lines_after > 0,
        "feed hold waited behind the whole queue ({lines_before} lines first)"
    );
    assert_eq!(lines_before + lines_after, 100);
}

#[test]
fn test_drop_joins_worker() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(50));
    connect(&mut controller);
    controller.start_stream(program(100)).expect("start_stream");
    assert!(wait_until(|| shared.lines_written() > 0), "no line sent");

    // Arc strong count: the test's handle plus the factory closure's clone plus
    // the live transport's clone.
    assert_eq!(Arc::strong_count(&shared), 3);
    drop(controller);
    // Drop joined the worker, so the transport (and its Arc) is already gone and
    // the transport was disconnected on the way out.
    assert_eq!(Arc::strong_count(&shared), 1);
    assert!(!shared.connected.load(Ordering::SeqCst));
}

#[test]
fn test_disconnect_emits_disconnected_and_allows_reconnect() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    connect(&mut controller);
    controller.disconnect();
    assert!(!shared.connected.load(Ordering::SeqCst));
    assert!(
        controller
            .poll()
            .into_iter()
            .any(|e| e == MachineEvent::Disconnected)
    );
    connect(&mut controller);
}

#[test]
fn test_start_stream_requires_connection() {
    let mut controller = MachineController::with_transport(Box::new(|| {
        Box::new(SerialCommunicator::new()) as Box<dyn Communicator>
    }));
    assert!(controller.start_stream(program(1)).is_err());
}

#[test]
fn test_jog_writes_jog_command() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    connect(&mut controller);
    controller.jog(gcodekit5_app::machine::Axis::X, 1.0, 1000.0);
    assert!(
        wait_until(|| shared
            .log()
            .contains(&Wrote::Line("$J=G91 G21 X1.00 F1000".to_string()))),
        "jog not written: {:?}",
        shared.log()
    );
}

#[test]
fn test_repaint_hook_called_on_event() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hook_calls = calls.clone();
    controller.set_repaint_hook(Box::new(move || {
        hook_calls.fetch_add(1, Ordering::SeqCst);
    }));
    connect(&mut controller);
    assert!(calls.load(Ordering::SeqCst) > 0);
}

/// Starts a 100-line stream with acks held and waits until the window is in use.
fn held_stream(lines: usize) -> (Arc<LoopbackShared>, MachineController) {
    let shared = Arc::new(LoopbackShared::default());
    shared.hold_acks.store(true, Ordering::SeqCst);
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    connect(&mut controller);
    controller
        .start_stream(program(lines))
        .expect("start_stream");
    assert!(wait_until(|| shared.lines_written() > 0), "no line sent");
    (shared, controller)
}

/// Collects events until `pred` matches, returning all of them.
fn events_until(
    controller: &mut MachineController,
    pred: impl Fn(&MachineEvent) -> bool,
) -> Vec<MachineEvent> {
    let deadline = Instant::now() + EVENTUALLY;
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        let batch = controller.poll();
        let done = batch.iter().any(&pred);
        seen.extend(batch);
        if done {
            return seen;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("event never arrived; saw {seen:?}");
}

/// The jog line [`MachineController::jog`] writes for X +1 mm at 1000 mm/min.
fn jog_line() -> Wrote {
    Wrote::Line("$J=G91 G21 X1.00 F1000".to_string())
}

#[test]
fn test_disconnect_during_slow_connect_returns_quickly() {
    let shared = Arc::new(LoopbackShared {
        connect_delay: Duration::from_secs(2),
        ..LoopbackShared::default()
    });
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    controller.connect(&tcp_settings()).expect("connect");

    let started = Instant::now();
    controller.disconnect();
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(500),
        "disconnect blocked {elapsed:?}"
    );
    assert!(controller.poll().contains(&MachineEvent::Disconnected));
    // The detached worker never reports Connected to this controller.
    let deadline = Instant::now() + Duration::from_millis(2500);
    while Instant::now() < deadline {
        assert!(!controller.poll().contains(&MachineEvent::Connected));
        std::thread::sleep(Duration::from_millis(20));
    }
    // And it shut its transport once connect returned.
    assert!(wait_until(|| !shared.connected.load(Ordering::SeqCst)));
}

#[test]
fn test_drop_during_slow_connect_returns_quickly() {
    let shared = Arc::new(LoopbackShared {
        connect_delay: Duration::from_secs(2),
        ..LoopbackShared::default()
    });
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    controller.connect(&tcp_settings()).expect("connect");

    let started = Instant::now();
    drop(controller);
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(500),
        "drop blocked {elapsed:?}"
    );
}

#[test]
fn test_stop_soft_resets_and_clears_buffers() {
    let (shared, mut controller) = held_stream(100);
    controller.stop();
    let events = events_until(&mut controller, |e| {
        matches!(e, MachineEvent::StreamStopped { .. })
    });
    assert!(events.contains(&MachineEvent::StreamStopped {
        acked: 0,
        total: 100
    }));
    let log = shared.log();
    let reset_at = log
        .iter()
        .position(|w| *w == Wrote::Realtime(0x18))
        .expect("soft reset written");

    // The window was cleared: a jog goes out although no line was ever acked.
    controller.jog(gcodekit5_app::machine::Axis::X, 1.0, 1000.0);
    assert!(wait_until(|| shared.log().contains(&jog_line())));
    let log = shared.log();
    let program_lines_after_reset = log[reset_at..]
        .iter()
        .filter(|w| matches!(w, Wrote::Line(_)) && **w != jog_line())
        .count();
    assert_eq!(program_lines_after_reset, 0, "program continued after stop");
}

#[test]
fn test_pause_stops_feeding_and_resume_restarts() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(10));
    connect(&mut controller);
    controller.start_stream(program(100)).expect("start_stream");
    assert!(wait_until(|| shared.lines_written() > 0), "no line sent");

    controller.pause();
    // Every in-flight line is acked and the feed hold went out.
    assert!(wait_until(|| {
        shared.log().contains(&Wrote::Realtime(b'!'))
            && shared.outstanding.load(Ordering::SeqCst) == 0
    }));
    // Several status-report cycles later, still nothing new.
    let paused_lines = shared.lines_written();
    let settle = Instant::now() + Duration::from_millis(300);
    while Instant::now() < settle {
        controller.poll();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(shared.lines_written(), paused_lines, "fed while paused");
    assert!(paused_lines < 100);
    let log = shared.log();
    let hold_at = log
        .iter()
        .position(|w| *w == Wrote::Realtime(b'!'))
        .expect("feed hold logged");
    assert!(
        !log[hold_at..].iter().any(|w| matches!(w, Wrote::Line(_))),
        "line written after feed hold"
    );

    controller.resume();
    assert_eq!(
        wait_for(&mut controller, |e| {
            matches!(e, MachineEvent::StreamFinished { .. })
        }),
        Some(MachineEvent::StreamFinished { lines: 100 })
    );
    let log = shared.log();
    let resume_at = log
        .iter()
        .position(|w| *w == Wrote::Realtime(b'~'))
        .expect("cycle start logged");
    assert!(resume_at > hold_at);
    assert_eq!(shared.lines_written(), 100);
}

#[test]
fn test_error_reply_counts_as_ack() {
    let (shared, mut controller) = held_stream(3);
    assert!(wait_until(|| shared.lines_written() == 3));
    shared.inject("error:20\n");
    let events = events_until(&mut controller, |e| {
        matches!(e, MachineEvent::StreamProgress { acked: 1, .. })
    });
    assert!(
        events
            .iter()
            .any(|e| matches!(e, MachineEvent::Error(m) if m.contains("error:20"))),
        "{events:?}"
    );
    // Two more oks finish the 3-line program; the third queued ok is surplus.
    shared.hold_acks.store(false, Ordering::SeqCst);
    assert_eq!(
        wait_for(&mut controller, |e| {
            matches!(e, MachineEvent::StreamFinished { .. })
        }),
        Some(MachineEvent::StreamFinished { lines: 3 })
    );
}

#[test]
fn test_alarm_ends_stream() {
    let (shared, mut controller) = held_stream(10);
    shared.inject("ALARM:1\n");
    let events = events_until(&mut controller, |e| {
        matches!(e, MachineEvent::StreamStopped { .. })
    });
    assert!(events.contains(&MachineEvent::Error("ALARM:1".to_string())));
    assert!(events.contains(&MachineEvent::StreamStopped {
        acked: 0,
        total: 10
    }));
}

#[test]
fn test_reset_banner_clears_window() {
    let (shared, mut controller) = held_stream(100);
    let banner = "Grbl 1.1h ['$' for help]";
    shared.inject(&format!("{banner}\n"));
    let events = events_until(&mut controller, |e| {
        matches!(e, MachineEvent::StreamStopped { .. })
    });
    assert!(events.contains(&MachineEvent::Received(banner.to_string())));
    // With acks still held, only an emptied window lets the jog out.
    controller.jog(gcodekit5_app::machine::Axis::X, 1.0, 1000.0);
    assert!(
        wait_until(|| shared.log().contains(&jog_line())),
        "jog blocked by a stale window"
    );
}

#[test]
fn test_failed_connect_emits_error_then_disconnected() {
    let shared = Arc::new(LoopbackShared {
        fail_connect: true,
        ..LoopbackShared::default()
    });
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    controller.connect(&tcp_settings()).expect("connect starts");
    let events = events_until(&mut controller, |e| *e == MachineEvent::Disconnected);
    let error_at = events
        .iter()
        .position(|e| matches!(e, MachineEvent::Error(m) if m.contains("connect failed")))
        .expect("connect error reported");
    let disconnected_at = events
        .iter()
        .position(|e| *e == MachineEvent::Disconnected)
        .expect("disconnected reported");
    assert!(error_at < disconnected_at, "{events:?}");
    assert!(!events.contains(&MachineEvent::Connected));
}

#[test]
fn test_failed_connect_clears_streaming() {
    let shared = Arc::new(LoopbackShared {
        connect_delay: Duration::from_millis(200),
        fail_connect: true,
        ..LoopbackShared::default()
    });
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    controller.connect(&tcp_settings()).expect("connect starts");
    // Queued while the worker is still connecting.
    controller.start_stream(program(5)).expect("queued");
    events_until(&mut controller, |e| *e == MachineEvent::Disconnected);

    controller.jog(gcodekit5_app::machine::Axis::X, 1.0, 1000.0);
    let events = controller.poll();
    assert!(
        events.contains(&MachineEvent::Error(
            gcodekit5_app::machine::MachineError::NotConnected.to_string()
        )),
        "{events:?}"
    );
}

#[test]
fn test_connect_rejects_auto_or_empty_port() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    for port in ["Auto", "auto", "", "  "] {
        let cfg = ConnectionSettings {
            port: port.to_string(),
            ..tcp_settings()
        };
        assert!(
            matches!(
                controller.connect(&cfg),
                Err(gcodekit5_app::machine::MachineError::InvalidSettings(_))
            ),
            "port {port:?} accepted"
        );
    }
}

#[test]
fn test_stale_worker_cannot_clear_new_session_streaming() {
    // First transport: a slow connect that fails. Second: a working loopback.
    let stale = Arc::new(LoopbackShared {
        connect_delay: Duration::from_millis(300),
        fail_connect: true,
        ..LoopbackShared::default()
    });
    let live = Arc::new(LoopbackShared::default());
    live.hold_acks.store(true, Ordering::SeqCst);
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (factory_stale, factory_live) = (stale.clone(), live.clone());
    let mut controller = MachineController::with_transport(Box::new(move || {
        let shared = if calls.fetch_add(1, Ordering::SeqCst) == 0 {
            factory_stale.clone()
        } else {
            factory_live.clone()
        };
        Box::new(Loopback::new(shared, Duration::from_millis(1))) as Box<dyn Communicator>
    }));

    controller
        .connect(&tcp_settings())
        .expect("slow connect starts");
    controller.disconnect();
    connect(&mut controller);
    controller.start_stream(program(100)).expect("start_stream");
    assert!(wait_until(|| live.lines_written() > 0), "no line sent");

    // Bounded wait until the stale worker has finished (its Arc clone is dropped:
    // left are the test's handle and the factory closure's clone).
    assert!(
        wait_until(|| Arc::strong_count(&stale) == 2),
        "stale worker never finished"
    );
    controller.poll();

    assert_eq!(
        controller.start_stream(program(1)),
        Err(gcodekit5_app::machine::MachineError::StreamInProgress)
    );
    controller.jog(gcodekit5_app::machine::Axis::X, 1.0, 1000.0);
    let events = controller.poll();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, MachineEvent::Error(m) if m.contains("streaming"))),
        "{events:?}"
    );
}

#[test]
fn test_send_line_writes_line() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    connect(&mut controller);
    assert_eq!(controller.send_line("  $H  "), Ok(()));
    assert!(
        wait_until(|| shared.log().contains(&Wrote::Line("$H".to_string()))),
        "line not written: {:?}",
        shared.log()
    );
}

#[test]
fn test_send_line_rejects_unsafe_lines() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    assert_eq!(
        controller.send_line("G0 X1"),
        Err(MachineError::NotConnected)
    );
    connect(&mut controller);
    let too_long = format!("G0 X{}", "1".repeat(MAX_LINE_LENGTH));
    let cases: [(&str, MachineError); 9] = [
        ("   ", MachineError::EmptyLine),
        ("G0 X1\nG0 X2", MachineError::NonPrintable(b'\n')),
        ("G0 X1\rG0", MachineError::NonPrintable(b'\r')),
        ("G0\u{18}X1", MachineError::NonPrintable(0x18)),
        ("G0 X1 \u{e9}", MachineError::NonPrintable(0xc3)),
        ("G0 X1!", MachineError::RealtimeCharacter('!')),
        ("~", MachineError::RealtimeCharacter('~')),
        ("G0?", MachineError::RealtimeCharacter('?')),
        (
            too_long.as_str(),
            MachineError::LineTooLong {
                length: too_long.len(),
                max: MAX_LINE_LENGTH,
            },
        ),
    ];
    for (line, expected) in cases {
        assert_eq!(controller.send_line(line), Err(expected), "{line:?}");
    }
    std::thread::sleep(Duration::from_millis(50));
    assert!(
        !shared
            .log()
            .iter()
            .any(|w| matches!(w, Wrote::Line(l) if l.starts_with("G0") || l == "~")),
        "refused line written: {:?}",
        shared.log()
    );
}

#[test]
fn test_send_line_refused_while_streaming() {
    let (shared, mut controller) = held_stream(100);
    assert_eq!(
        controller.send_line("G0 X5"),
        Err(MachineError::StreamInProgress)
    );
    std::thread::sleep(Duration::from_millis(50));
    assert!(!shared.log().contains(&Wrote::Line("G0 X5".to_string())));
}

#[test]
fn test_is_streaming_from_start_stream() {
    let (_shared, mut controller) = held_stream(100);
    assert!(controller.is_streaming());
    controller.stop();
    assert!(wait_until(|| !controller.is_streaming()));
}

/// Index of the first jog cancel byte in `log`, and the log length when the transport closed.
fn cancel_and_close(shared: &LoopbackShared) -> (Option<usize>, Option<usize>) {
    let log = shared.log();
    let cancel = log.iter().position(|w| *w == Wrote::Realtime(0x85));
    let closed = *shared.closed_at.lock().expect("closed_at");
    (cancel, closed)
}

/// Connects and starts a jog that the loopback never reports finished.
fn jogging() -> (Arc<LoopbackShared>, MachineController) {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    connect(&mut controller);
    controller.jog(gcodekit5_app::machine::Axis::X, 200.0, 1000.0);
    assert!(
        wait_until(|| shared
            .log()
            .iter()
            .any(|w| matches!(w, Wrote::Line(l) if l.starts_with("$J=")))),
        "jog not written"
    );
    (shared, controller)
}

#[test]
fn test_disconnect_during_jog_writes_jog_cancel_first() {
    let (shared, mut controller) = jogging();
    controller.disconnect();
    let (cancel, closed) = cancel_and_close(&shared);
    let cancel = cancel.expect("no jog cancel on disconnect");
    let closed = closed.expect("transport not closed");
    assert!(
        cancel < closed,
        "jog cancel after the close: {:?}",
        shared.log()
    );
}

#[test]
fn test_drop_during_jog_writes_jog_cancel_first() {
    let (shared, controller) = jogging();
    drop(controller);
    let (cancel, closed) = cancel_and_close(&shared);
    let cancel = cancel.expect("no jog cancel on drop");
    let closed = closed.expect("transport not closed");
    assert!(
        cancel < closed,
        "jog cancel after the close: {:?}",
        shared.log()
    );
}

#[test]
fn test_jog_cancel_or_idle_clears_outstanding_jog() {
    // Cancelled by the caller: no second cancel on disconnect.
    let (shared, mut controller) = jogging();
    controller.realtime(RealtimeCommand::JogCancel);
    assert!(wait_until(|| shared.log().contains(&Wrote::Realtime(0x85))));
    controller.disconnect();
    let cancels = shared
        .log()
        .iter()
        .filter(|w| **w == Wrote::Realtime(0x85))
        .count();
    assert_eq!(cancels, 1, "{:?}", shared.log());

    // Finished: GRBL reported Jog, then Idle.
    let (shared, mut controller) = jogging();
    shared.inject("<Jog|MPos:1.000,0.000,0.000|FS:1000,0>\n");
    shared.inject("<Idle|MPos:200.000,0.000,0.000|FS:0,0>\n");
    assert!(
        wait_for(&mut controller, |e| matches!(
            e,
            MachineEvent::StatusReport(s) if s.machine_state.as_deref() == Some("Idle")
        ))
        .is_some(),
        "no Idle report"
    );
    controller.disconnect();
    assert!(
        !shared.log().contains(&Wrote::Realtime(0x85)),
        "cancel after a finished jog: {:?}",
        shared.log()
    );
}
