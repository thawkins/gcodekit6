//! In-memory loopback transport for tests of [`MachineController`] and the front ends that
//! drive it. Not part of the public API: built only with the `test-support` feature, which
//! only dev-dependencies enable.
//!
//! The loopback records every write in order and answers each complete line with `ok` after
//! a fixed delay, like a GRBL controller executing a short move.
//!
//! Status reports are answers to `?`, as on a real controller: an injected `<...>` report
//! waits until a `?` is written and is then owed as that `?`'s reply. Once one report has
//! been injected, every later `?` is answered with the latest injected report. Replies owed
//! are handed out in order, at most [`LoopbackShared::reply_budget`] of them when it is set,
//! each no sooner than [`LoopbackShared::reply_delay`] after its `?` (a slow link).
//!
//! [`LoopbackShared::link_down`] drops the link: every read and write fails as a pulled
//! cable does, and a connect is refused until it is cleared.
//!
//! It stands in for a Repetier controller too: each numbered line gets its `ok`, and
//! [`LoopbackShared::resend_once`] answers one line with `Resend:N` and `ok` instead.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gcodekit5_communication::{Communicator, ConnectionParams};
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
    /// Response bytes handed out by `receive` before any `ok` (status reports excepted).
    pub inject: Mutex<VecDeque<Vec<u8>>>,
    /// Injected status reports waiting for a `?` to answer.
    pub status_reports: Mutex<VecDeque<Vec<u8>>>,
    /// The latest status report used as a reply; it answers every `?` once no injected one
    /// waits.
    pub sticky_report: Mutex<Option<Vec<u8>>>,
    /// Replies owed to written `?` bytes, oldest first, each with when it may be read.
    pub owed_replies: Mutex<VecDeque<(Instant, Vec<u8>)>>,
    /// How long after its `?` a status reply arrives (a slow link); zero by default.
    pub reply_delay: Mutex<Duration>,
    /// When set, how many more owed replies `receive` may hand out (`None`: all).
    pub reply_budget: Mutex<Option<usize>>,
    /// Lines written but not yet answered with `ok`.
    pub outstanding: AtomicUsize,
    /// The length of [`LoopbackShared::log`] when the transport was last closed, so a test
    /// can tell what was written before the close.
    pub closed_at: Mutex<Option<usize>>,
    /// A Repetier line number: the first line written as `N<n> ...` is answered with
    /// `Resend:<n>` and `ok`, as Repetier firmware answers a checksum error, instead of the
    /// line's own `ok`; then cleared.
    pub resend_once: Mutex<Option<u32>>,
    /// While set, the link is down: reads and writes fail and `connect` is refused.
    pub link_down: AtomicBool,
    /// How many times `connect` succeeded.
    pub connects: AtomicUsize,
}

impl LoopbackShared {
    /// Every write so far, in order.
    pub fn log(&self) -> Vec<Wrote> {
        self.log
            .lock()
            .expect("loopback log mutex poisoned")
            .clone()
    }

    /// Queues `response` to be read before any `ok`; a status report (`<...>`) instead waits
    /// to answer the next `?` written.
    pub fn inject(&self, response: &str) {
        let queue = if response.starts_with('<') {
            &self.status_reports
        } else {
            &self.inject
        };
        queue
            .lock()
            .expect("loopback inject mutex poisoned")
            .push_back(response.as_bytes().to_vec());
    }

    /// How many `?` status queries were written.
    pub fn status_queries(&self) -> usize {
        self.log()
            .iter()
            .filter(|w| **w == Wrote::Realtime(b'?'))
            .count()
    }

    /// Owes the reply to one written `?`: the next injected report, else the latest one.
    fn answer_status_query(&self) {
        let next = self
            .status_reports
            .lock()
            .expect("loopback status mutex poisoned")
            .pop_front();
        let mut sticky = self
            .sticky_report
            .lock()
            .expect("loopback sticky mutex poisoned");
        if let Some(report) = next {
            *sticky = Some(report);
        }
        if let Some(report) = sticky.clone() {
            let delay = *self
                .reply_delay
                .lock()
                .expect("loopback delay mutex poisoned");
            self.owed_replies
                .lock()
                .expect("loopback replies mutex poisoned")
                .push_back((Instant::now() + delay, report));
        }
    }

    /// The next owed reply `receive` may hand out.
    fn take_reply(&self) -> Option<Vec<u8>> {
        let mut budget = self
            .reply_budget
            .lock()
            .expect("loopback budget mutex poisoned");
        if *budget == Some(0) {
            return None;
        }
        let mut owed = self
            .owed_replies
            .lock()
            .expect("loopback replies mutex poisoned");
        if owed.front().is_none_or(|(due, _)| *due > Instant::now()) {
            return None;
        }
        let (_, reply) = owed.pop_front()?;
        if let Some(left) = budget.as_mut() {
            *left -= 1;
        }
        Some(reply)
    }

    /// Whether `line` is the numbered line [`LoopbackShared::resend_once`] names; if so it
    /// is recorded, answered with `Resend:<n>` and the request is cleared.
    fn take_resend(&self, line: &str) -> bool {
        let mut resend = self
            .resend_once
            .lock()
            .expect("loopback resend mutex poisoned");
        let Some(n) = *resend else {
            return false;
        };
        if !line.starts_with(&format!("N{n} ")) {
            return false;
        }
        *resend = None;
        self.log
            .lock()
            .expect("loopback log mutex poisoned")
            .push(Wrote::Line(line.to_string()));
        self.inject(&format!("Resend:{n}\nok\n"));
        true
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
        if self.shared.fail_connect || self.shared.link_down.load(Ordering::SeqCst) {
            return Err(gcodekit5_core::Error::other("loopback refused"));
        }
        // A new link: nothing owed by the last one is still in flight.
        self.carry.clear();
        self.pending_acks.clear();
        self.shared.outstanding.store(0, Ordering::SeqCst);
        self.shared.connects.fetch_add(1, Ordering::SeqCst);
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
        if self.shared.link_down.load(Ordering::SeqCst) {
            self.shared.connected.store(false, Ordering::SeqCst);
            return Err(gcodekit5_core::Error::other(
                "Connection lost: loopback link down",
            ));
        }
        if data.len() == 1 && self.carry.is_empty() && is_realtime_byte(data[0]) {
            self.shared
                .log
                .lock()
                .expect("loopback log mutex poisoned")
                .push(Wrote::Realtime(data[0]));
            if data[0] == b'?' {
                self.shared.answer_status_query();
            }
            return Ok(1);
        }
        for &b in data {
            if b == b'\n' {
                let line = String::from_utf8_lossy(&self.carry).to_string();
                self.carry.clear();
                if self.shared.take_resend(&line) {
                    continue;
                }
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
        if self.shared.link_down.load(Ordering::SeqCst) {
            self.shared.connected.store(false, Ordering::SeqCst);
            return Err(gcodekit5_core::Error::other(
                "Connection lost: loopback link down",
            ));
        }
        if let Some(data) = self
            .shared
            .inject
            .lock()
            .expect("loopback inject mutex poisoned")
            .pop_front()
        {
            return Ok(data);
        }
        if let Some(reply) = self.shared.take_reply() {
            return Ok(reply);
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
