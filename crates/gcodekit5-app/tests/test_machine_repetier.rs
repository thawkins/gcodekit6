//! Tests for `MachineController` on a Repetier connection and for its connect-time check of
//! the controller's firmware against the profile's protocol.
//!
//! The loopback transport of `gcodekit5_app::test_support` stands in for the controller: it
//! records every write and answers each complete line with `ok`, and can answer one numbered
//! line with `Resend:N`. A Repetier connection must never write a GRBL real-time byte or a
//! `$` command.

use std::sync::Arc;
use std::time::{Duration, Instant};

use gcodekit5_app::machine::{
    Axis, MachineController, MachineError, MachineEvent, REPETIER_ESTOP_LABEL, RealtimeCommand,
    StreamMode, worker_text,
};
use gcodekit5_app::test_support::{LoopbackShared, Wrote, loopback_controller, tcp_settings};
use gcodekit5_communication::firmware::ControllerProtocol;

/// Upper bound for "eventually" waits; generous so a loaded machine is not flaky.
const EVENTUALLY: Duration = Duration::from_secs(10);

fn program(lines: usize) -> Arc<[String]> {
    (0..lines)
        .map(|i| format!("G1 X{} F1000", i + 1))
        .collect::<Vec<_>>()
        .into()
}

/// Polls until an event matching `pred` arrives or [`EVENTUALLY`] passes; every event seen
/// is appended to `seen`.
fn wait_for(
    controller: &mut MachineController,
    seen: &mut Vec<MachineEvent>,
    pred: impl Fn(&MachineEvent) -> bool,
) -> Option<MachineEvent> {
    let deadline = Instant::now() + EVENTUALLY;
    while Instant::now() < deadline {
        let events = controller.poll();
        let found = events.iter().find(|e| pred(e)).cloned();
        seen.extend(events);
        if found.is_some() {
            return found;
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

/// The lines written, without their `N<n> ` number and `*<checksum>`.
fn commands(shared: &LoopbackShared) -> Vec<String> {
    shared
        .log()
        .iter()
        .filter_map(|w| match w {
            Wrote::Line(line) => Some(strip_number(line)),
            Wrote::Realtime(_) => None,
        })
        .collect()
}

fn strip_number(line: &str) -> String {
    let body = line.split('*').next().unwrap_or(line).trim();
    match body.strip_prefix('N') {
        Some(rest) => rest
            .split_once(' ')
            .map_or(String::new(), |(_, cmd)| cmd.trim().to_string()),
        None => body.to_string(),
    }
}

/// Asserts that nothing GRBL-only was written: no real-time byte, no `$` command, no byte
/// outside printable ASCII.
fn assert_no_grbl_writes(shared: &LoopbackShared) {
    for wrote in shared.log() {
        match wrote {
            Wrote::Realtime(byte) => panic!("GRBL real-time byte 0x{byte:02X} written"),
            Wrote::Line(line) => {
                assert!(!line.contains('$'), "GRBL `$` command written: {line}");
                assert!(
                    line.bytes().all(|b| (0x20..=0x7e).contains(&b)),
                    "non-printable byte written: {line:?}"
                );
            }
        }
    }
}

/// Connects a Repetier controller over the loopback and waits for the start-up lines'
/// acknowledgements.
fn connect_repetier(shared: &Arc<LoopbackShared>) -> MachineController {
    let mut controller = loopback_controller(shared, Duration::from_millis(1));
    controller
        .connect_with(&tcp_settings(), ControllerProtocol::Repetier)
        .expect("connect");
    let mut seen = Vec::new();
    assert!(
        wait_for(&mut controller, &mut seen, |e| *e
            == MachineEvent::Connected)
        .is_some()
    );
    assert!(controller.is_repetier());
    // M110 then M115 go out first; wait until both are answered.
    assert!(wait_until(|| shared.lines_written() >= 2
        && shared.outstanding.load(std::sync::atomic::Ordering::SeqCst)
            == 0));
    controller
}

#[test]
fn repetier_stream_numbers_every_line_and_finishes() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = connect_repetier(&shared);
    controller.start_stream(program(5)).expect("start");
    let mut seen = Vec::new();
    assert!(
        wait_for(&mut controller, &mut seen, |e| matches!(
            e,
            MachineEvent::StreamFinished { lines: 5 }
        ))
        .is_some(),
        "stream did not finish: {seen:?}"
    );
    let log = shared.log();
    let numbered: Vec<&String> = log
        .iter()
        .filter_map(|w| match w {
            Wrote::Line(line) => Some(line),
            Wrote::Realtime(_) => None,
        })
        .collect();
    for (i, line) in numbered.iter().enumerate() {
        let (head, checksum) = line.rsplit_once('*').expect("checksum");
        assert!(head.starts_with(&format!("N{} ", i + 1)), "{line}");
        let expected = head.bytes().fold(0u8, |acc, b| acc ^ b);
        assert_eq!(checksum.parse::<u8>().ok(), Some(expected), "{line}");
    }
    assert_eq!(
        commands(&shared),
        vec![
            "M110",
            "M115",
            "G1 X1 F1000",
            "G1 X2 F1000",
            "G1 X3 F1000",
            "G1 X4 F1000",
            "G1 X5 F1000"
        ]
    );
    assert_no_grbl_writes(&shared);
    // The stand-in status report keeps the position readout working.
    assert!(
        seen.iter()
            .any(|e| matches!(e, MachineEvent::StatusReport(_))),
        "no status report"
    );
}

#[test]
fn repetier_resend_replays_the_requested_line() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = connect_repetier(&shared);
    // N3 is the program's first line (after M110 and M115); ask for N4 again.
    *shared.resend_once.lock().expect("resend") = Some(4);
    controller.start_stream(program(4)).expect("start");
    let mut seen = Vec::new();
    assert!(
        wait_for(&mut controller, &mut seen, |e| matches!(
            e,
            MachineEvent::StreamFinished { lines: 4 }
        ))
        .is_some(),
        "stream did not finish after the resend: {seen:?}"
    );
    let lines: Vec<String> = shared
        .log()
        .iter()
        .filter_map(|w| match w {
            Wrote::Line(line) => Some(line.clone()),
            Wrote::Realtime(_) => None,
        })
        .collect();
    let n4: Vec<&String> = lines.iter().filter(|l| l.starts_with("N4 ")).collect();
    assert_eq!(n4.len(), 2, "N4 not re-sent exactly once: {lines:?}");
    assert_eq!(n4[0], n4[1], "the resend differs from the original");
    // The program after the replayed line continues in order.
    assert_eq!(
        commands(&shared)[2..],
        [
            "G1 X1 F1000",
            "G1 X2 F1000",
            "G1 X2 F1000",
            "G1 X3 F1000",
            "G1 X4 F1000"
        ]
    );
    assert!(
        seen.iter()
            .any(|e| matches!(e, MachineEvent::Received(l) if l == "Resend:4")),
        "resend request not shown: {seen:?}"
    );
    assert_no_grbl_writes(&shared);
}

#[test]
fn repetier_emergency_stop_writes_m112_and_drops_the_rest() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = connect_repetier(&shared);
    shared
        .hold_acks
        .store(true, std::sync::atomic::Ordering::SeqCst);
    controller.start_stream(program(20)).expect("start");
    assert!(wait_until(|| shared.lines_written() >= 3));
    let before_stop = Instant::now();
    controller.stop();
    let mut seen = Vec::new();
    assert!(
        wait_for(&mut controller, &mut seen, |e| matches!(
            e,
            MachineEvent::StreamStopped { .. }
        ))
        .is_some(),
        "the stop did not end the stream: {seen:?}"
    );
    assert!(seen.contains(&MachineEvent::Sent(REPETIER_ESTOP_LABEL.to_string())));
    // A jog the user sent before the stop, reaching the worker after it, must not run.
    controller.jog_sent_earlier(Axis::X, 1.0, 1000.0, before_stop);
    let dropped = |e: &MachineEvent| matches!(e, MachineEvent::Error(m) if m.starts_with(worker_text::NOT_SENT_AFTER_RESET));
    if !seen.iter().any(dropped) {
        wait_for(&mut controller, &mut seen, dropped).expect("the earlier jog was not dropped");
    }
    shared
        .hold_acks
        .store(false, std::sync::atomic::Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(100));
    let lines = commands(&shared);
    let stop = lines
        .iter()
        .position(|l| l == "M112")
        .expect("M112 written");
    assert_eq!(stop, lines.len() - 1, "lines written after M112: {lines:?}");
    assert!(
        shared
            .log()
            .iter()
            .any(|w| *w == Wrote::Line("M112".to_string())),
        "M112 must be written unnumbered"
    );
    assert!(controller.lines_answered());
    assert!(!controller.is_streaming());
    assert_no_grbl_writes(&shared);
}

#[test]
fn repetier_soft_reset_command_is_the_emergency_stop() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = connect_repetier(&shared);
    controller.realtime(RealtimeCommand::SoftReset);
    let mut seen = Vec::new();
    assert!(
        wait_for(&mut controller, &mut seen, |e| *e
            == MachineEvent::Sent(REPETIER_ESTOP_LABEL.to_string()))
        .is_some()
    );
    assert_no_grbl_writes(&shared);
}

#[test]
fn repetier_never_writes_grbl_bytes_or_dollar_commands() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = connect_repetier(&shared);
    for command in [
        RealtimeCommand::StatusQuery,
        RealtimeCommand::FeedHold,
        RealtimeCommand::CycleStart,
        RealtimeCommand::JogCancel,
        RealtimeCommand::FeedOverridePlus10,
        RealtimeCommand::SpindleOverrideMinus1,
        RealtimeCommand::RapidOverride25,
        RealtimeCommand::SpindleStop,
    ] {
        controller.realtime(command);
    }
    controller.pause();
    controller.resume();
    assert!(matches!(
        controller.send_line("$X"),
        Err(MachineError::NotOnRepetier(_))
    ));
    assert!(matches!(
        controller.send_query("$G"),
        Err(MachineError::NotOnRepetier(_))
    ));
    assert!(matches!(
        controller.start_stream_with(program(1), StreamMode::StopOnError),
        Err(MachineError::NotOnRepetier(_))
    ));
    controller.send_line("$H").expect("homing is translated");
    controller.jog(Axis::Y, -2.0, 500.0);
    let mut seen = Vec::new();
    assert!(wait_until(|| commands(&shared).iter().any(|l| l == "G90")));
    controller.poll().into_iter().for_each(|e| seen.push(e));
    let refused = seen
        .iter()
        .filter(
            |e| matches!(e, MachineEvent::Error(m) if m.starts_with(worker_text::NOT_ON_REPETIER)),
        )
        .count();
    assert_eq!(refused, 4, "each override refused once: {seen:?}");
    assert_eq!(
        commands(&shared)[2..],
        ["G28", "G21", "G91", "G1 Y-2.00 F500", "G90"]
    );
    assert!(wait_until(|| controller.lines_answered()));
    assert_no_grbl_writes(&shared);
}

#[test]
fn repetier_restart_drops_the_stream_and_renumbers() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = connect_repetier(&shared);
    shared
        .hold_acks
        .store(true, std::sync::atomic::Ordering::SeqCst);
    controller.start_stream(program(10)).expect("start");
    assert!(wait_until(|| shared.lines_written() >= 3));
    shared.inject("start\n");
    let mut seen = Vec::new();
    assert!(
        wait_for(&mut controller, &mut seen, |e| matches!(
            e,
            MachineEvent::StreamStopped { .. }
        ))
        .is_some(),
        "the restart did not end the stream: {seen:?}"
    );
    shared
        .hold_acks
        .store(false, std::sync::atomic::Ordering::SeqCst);
    assert!(wait_until(|| shared.lines_written() >= 5));
    let lines: Vec<String> = shared
        .log()
        .iter()
        .filter_map(|w| match w {
            Wrote::Line(line) => Some(line.clone()),
            Wrote::Realtime(_) => None,
        })
        .collect();
    // The start-up lines again, numbered from 1, and no program line after the restart.
    assert!(lines[3].starts_with("N1 M110 "), "{lines:?}");
    assert!(lines[4].starts_with("N2 M115 "), "{lines:?}");
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(shared.lines_written(), 5, "{:?}", shared.log());
    assert_no_grbl_writes(&shared);
}

/// Connects with `protocol`, lets the controller say `banner`, and returns the events up to
/// the disconnect.
fn connect_and_identify(protocol: ControllerProtocol, banner: &str) -> Vec<MachineEvent> {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    controller
        .connect_with(&tcp_settings(), protocol)
        .expect("connect");
    let mut seen = Vec::new();
    wait_for(&mut controller, &mut seen, |e| {
        *e == MachineEvent::Connected
    })
    .expect("connected");
    shared.inject(banner);
    wait_for(&mut controller, &mut seen, |e| {
        *e == MachineEvent::Disconnected
    })
    .expect("the mismatch did not disconnect");
    if protocol == ControllerProtocol::Repetier {
        assert_no_grbl_writes(&shared);
    }
    seen
}

#[test]
fn grbl_profile_rejects_a_repetier_controller() {
    let seen = connect_and_identify(
        ControllerProtocol::Grbl,
        "FIRMWARE_NAME:Repetier_1.0.6 FIRMWARE_URL:https://github.com/repetier\n",
    );
    assert!(seen.contains(&MachineEvent::ProtocolMismatch {
        configured: ControllerProtocol::Grbl,
        detected: ControllerProtocol::Repetier,
    }));
}

#[test]
fn repetier_profile_rejects_a_grbl_controller() {
    let seen = connect_and_identify(ControllerProtocol::Repetier, "Grbl 1.1h ['$' for help]\n");
    assert!(seen.contains(&MachineEvent::ProtocolMismatch {
        configured: ControllerProtocol::Repetier,
        detected: ControllerProtocol::Grbl,
    }));
}

#[test]
fn matching_identity_keeps_the_connection() {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    controller
        .connect_with(&tcp_settings(), ControllerProtocol::Grbl)
        .expect("connect");
    let mut seen = Vec::new();
    wait_for(&mut controller, &mut seen, |e| {
        *e == MachineEvent::Connected
    })
    .expect("connected");
    shared.inject("Grbl 1.1h ['$' for help]\n");
    wait_for(
        &mut controller,
        &mut seen,
        |e| matches!(e, MachineEvent::Received(l) if l.starts_with("Grbl")),
    )
    .expect("banner");
    std::thread::sleep(Duration::from_millis(50));
    seen.extend(controller.poll());
    assert!(!seen.contains(&MachineEvent::Disconnected), "{seen:?}");
    assert!(
        !seen
            .iter()
            .any(|e| matches!(e, MachineEvent::ProtocolMismatch { .. }))
    );
}
