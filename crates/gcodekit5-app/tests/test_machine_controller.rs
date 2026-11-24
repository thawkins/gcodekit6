//! Tests for the non-blocking `MachineController`.
//!
//! The controller runs against an in-memory loopback transport defined here: it
//! records every write in order and answers each complete line with `ok` after a
//! fixed delay, like a GRBL controller executing a short move.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gcodekit5_app::machine::{MachineController, MachineEvent, RealtimeCommand};
use gcodekit5_communication::{
    Communicator, CommunicatorListenerHandle, ConnectionParams, SerialCommunicator,
};
use gcodekit5_settings::{ConnectionSettings, ConnectionType};

/// Upper bound for "eventually" waits; generous so a loaded machine is not flaky.
const EVENTUALLY: Duration = Duration::from_secs(10);

/// One write observed by the loopback transport.
#[derive(Debug, Clone, PartialEq)]
enum Wrote {
    /// A complete newline-terminated line (without the newline).
    Line(String),
    /// A single real-time byte written on its own.
    Realtime(u8),
}

/// State shared between the test and the loopback transport.
#[derive(Default)]
struct LoopbackShared {
    log: Mutex<Vec<Wrote>>,
    /// While set, no `ok` is released (the controller is "holding").
    hold_acks: AtomicBool,
    connected: AtomicBool,
}

impl LoopbackShared {
    fn log(&self) -> Vec<Wrote> {
        self.log.lock().unwrap().clone()
    }

    fn lines_written(&self) -> usize {
        self.log()
            .iter()
            .filter(|w| matches!(w, Wrote::Line(_)))
            .count()
    }
}

/// In-memory GRBL stand-in: acks each line `ok_delay` after the previous ack.
struct Loopback {
    shared: Arc<LoopbackShared>,
    ok_delay: Duration,
    carry: Vec<u8>,
    pending_acks: VecDeque<Instant>,
    last_due: Instant,
    params: Option<ConnectionParams>,
}

impl Loopback {
    fn new(shared: Arc<LoopbackShared>, ok_delay: Duration) -> Self {
        Self {
            shared,
            ok_delay,
            carry: Vec::new(),
            pending_acks: VecDeque::new(),
            last_due: Instant::now(),
            params: None,
        }
    }
}

fn is_realtime_byte(b: u8) -> bool {
    matches!(b, b'?' | b'!' | b'~' | 0x18) || b >= 0x80
}

impl Communicator for Loopback {
    fn connect(&mut self, params: &ConnectionParams) -> gcodekit5_core::Result<()> {
        self.params = Some(params.clone());
        self.shared.connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn disconnect(&mut self) -> gcodekit5_core::Result<()> {
        self.shared.connected.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.shared.connected.load(Ordering::SeqCst)
    }

    fn send(&mut self, data: &[u8]) -> gcodekit5_core::Result<usize> {
        if data.len() == 1 && self.carry.is_empty() && is_realtime_byte(data[0]) {
            self.shared
                .log
                .lock()
                .unwrap()
                .push(Wrote::Realtime(data[0]));
            return Ok(1);
        }
        for &b in data {
            if b == b'\n' {
                let line = String::from_utf8_lossy(&self.carry).to_string();
                self.carry.clear();
                self.shared.log.lock().unwrap().push(Wrote::Line(line));
                let due = self.last_due.max(Instant::now()) + self.ok_delay;
                self.last_due = due;
                self.pending_acks.push_back(due);
            } else {
                self.carry.push(b);
            }
        }
        Ok(data.len())
    }

    fn receive(&mut self) -> gcodekit5_core::Result<Vec<u8>> {
        if !self.shared.hold_acks.load(Ordering::SeqCst)
            && let Some(&due) = self.pending_acks.front()
            && due <= Instant::now()
        {
            self.pending_acks.pop_front();
            return Ok(b"ok\n".to_vec());
        }
        // Behave like a serial read with a short timeout.
        std::thread::sleep(Duration::from_millis(1));
        Ok(Vec::new())
    }

    fn add_listener(&mut self, _listener: CommunicatorListenerHandle) {}

    fn remove_listener(&mut self, _listener: &CommunicatorListenerHandle) {}

    fn connection_params(&self) -> Option<&ConnectionParams> {
        self.params.as_ref()
    }

    fn set_connection_params(&mut self, params: ConnectionParams) -> gcodekit5_core::Result<()> {
        self.params = Some(params);
        Ok(())
    }
}

/// A controller whose transport is a loopback sharing `shared`.
fn loopback_controller(shared: &Arc<LoopbackShared>, ok_delay: Duration) -> MachineController {
    let shared = shared.clone();
    MachineController::with_transport(Box::new(move || {
        Box::new(Loopback::new(shared.clone(), ok_delay)) as Box<dyn Communicator>
    }))
}

fn tcp_settings() -> ConnectionSettings {
    ConnectionSettings {
        connection_type: ConnectionType::Tcp,
        port: "loopback".to_string(),
        tcp_port: 23,
        ..ConnectionSettings::default()
    }
}

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
