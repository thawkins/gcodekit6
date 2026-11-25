//! In-memory loopback transport for tests of [`MachineController`] and the front ends that
//! drive it. Not part of the public API: built only with the `test-support` feature, which
//! only dev-dependencies enable.
//!
//! The loopback records every write in order and answers each complete line with `ok` after
//! a fixed delay, like a GRBL controller executing a short move.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gcodekit5_communication::{Communicator, CommunicatorListenerHandle, ConnectionParams};
use gcodekit5_settings::{ConnectionSettings, ConnectionType};

use crate::machine::MachineController;

/// One write observed by the loopback transport.
#[derive(Debug, Clone, PartialEq)]
pub enum Wrote {
    /// A complete newline-terminated line (without the newline).
    Line(String),
    /// A single real-time byte written on its own.
    Realtime(u8),
}

/// State shared between the test and the loopback transport.
#[derive(Default)]
pub struct LoopbackShared {
    /// Every write, in order.
    pub log: Mutex<Vec<Wrote>>,
    /// While set, no `ok` is released (the controller is "holding").
    pub hold_acks: AtomicBool,
    /// Whether the transport is open.
    pub connected: AtomicBool,
    /// How long `connect` takes (a slow TCP connect).
    pub connect_delay: Duration,
    /// When set, `connect` fails after `connect_delay`.
    pub fail_connect: bool,
    /// Response bytes handed out by `receive` before any `ok`.
    pub inject: Mutex<VecDeque<Vec<u8>>>,
    /// Lines written but not yet answered with `ok`.
    pub outstanding: AtomicUsize,
    /// The length of [`LoopbackShared::log`] when the transport was last closed, so a test
    /// can tell what was written before the close.
    pub closed_at: Mutex<Option<usize>>,
}

impl LoopbackShared {
    /// Every write so far, in order.
    pub fn log(&self) -> Vec<Wrote> {
        self.log
            .lock()
            .expect("loopback log mutex poisoned")
            .clone()
    }

    /// Queues `response` to be read before any `ok`.
    pub fn inject(&self, response: &str) {
        self.inject
            .lock()
            .expect("loopback inject mutex poisoned")
            .push_back(response.as_bytes().to_vec());
    }

    /// How many complete lines were written.
    pub fn lines_written(&self) -> usize {
        self.log()
            .iter()
            .filter(|w| matches!(w, Wrote::Line(_)))
            .count()
    }
}

/// In-memory GRBL stand-in: acks each line `ok_delay` after the previous ack.
pub struct Loopback {
    shared: Arc<LoopbackShared>,
    ok_delay: Duration,
    carry: Vec<u8>,
    pending_acks: VecDeque<Instant>,
    last_due: Instant,
    params: Option<ConnectionParams>,
}

impl Loopback {
    /// A loopback over `shared` that acks each line `ok_delay` after the previous ack.
    pub fn new(shared: Arc<LoopbackShared>, ok_delay: Duration) -> Self {
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
        std::thread::sleep(self.shared.connect_delay);
        if self.shared.fail_connect {
            return Err(gcodekit5_core::Error::other("loopback refused"));
        }
        self.params = Some(params.clone());
        self.shared.connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn disconnect(&mut self) -> gcodekit5_core::Result<()> {
        let written = self
            .shared
            .log
            .lock()
            .expect("loopback log mutex poisoned")
            .len();
        *self
            .shared
            .closed_at
            .lock()
            .expect("loopback closed_at mutex poisoned") = Some(written);
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
                .expect("loopback log mutex poisoned")
                .push(Wrote::Realtime(data[0]));
            return Ok(1);
        }
        for &b in data {
            if b == b'\n' {
                let line = String::from_utf8_lossy(&self.carry).to_string();
                self.carry.clear();
                self.shared
                    .log
                    .lock()
                    .expect("loopback log mutex poisoned")
                    .push(Wrote::Line(line));
                let due = self.last_due.max(Instant::now()) + self.ok_delay;
                self.last_due = due;
                self.pending_acks.push_back(due);
                self.shared.outstanding.fetch_add(1, Ordering::SeqCst);
            } else {
                self.carry.push(b);
            }
        }
        Ok(data.len())
    }

    fn receive(&mut self) -> gcodekit5_core::Result<Vec<u8>> {
        if let Some(data) = self
            .shared
            .inject
            .lock()
            .expect("loopback inject mutex poisoned")
            .pop_front()
        {
            return Ok(data);
        }
        if !self.shared.hold_acks.load(Ordering::SeqCst)
            && let Some(&due) = self.pending_acks.front()
            && due <= Instant::now()
        {
            self.pending_acks.pop_front();
            self.shared.outstanding.fetch_sub(1, Ordering::SeqCst);
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
pub fn loopback_controller(shared: &Arc<LoopbackShared>, ok_delay: Duration) -> MachineController {
    let shared = shared.clone();
    MachineController::with_transport(Box::new(move || {
        Box::new(Loopback::new(shared.clone(), ok_delay)) as Box<dyn Communicator>
    }))
}

/// TCP settings naming the loopback host.
pub fn tcp_settings() -> ConnectionSettings {
    ConnectionSettings {
        connection_type: ConnectionType::Tcp,
        port: "loopback".to_string(),
        tcp_port: 23,
        ..ConnectionSettings::default()
    }
}
