//! `SerialCommunicator` is a plain byte pipe: it writes exactly what it is given and
//! returns exactly what the port read. Line numbering, checksums and realtime-command
//! filtering for Repetier belong to the machine layer's Repetier link, not here.

use std::sync::{Arc, Mutex};

use gcodekit5_communication::communication::serial::SerialPort;
use gcodekit5_communication::{Communicator, SerialCommunicator};

/// A port that records every write and hands out scripted reads.
struct ScriptedPort {
    written: Arc<Mutex<Vec<u8>>>,
    reads: Vec<Vec<u8>>,
}

impl SerialPort for ScriptedPort {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.written
            .lock()
            .expect("written lock")
            .extend_from_slice(data);
        Ok(data.len())
    }
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.reads.is_empty() {
            return Ok(0);
        }
        let next = self.reads.remove(0);
        buf[..next.len()].copy_from_slice(&next);
        Ok(next.len())
    }
    fn name(&self) -> String {
        "scripted".to_string()
    }
    fn close(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn communicator(reads: Vec<Vec<u8>>) -> (SerialCommunicator, Arc<Mutex<Vec<u8>>>) {
    let written = Arc::new(Mutex::new(Vec::new()));
    let port = ScriptedPort {
        written: Arc::clone(&written),
        reads,
    };
    (SerialCommunicator::with_port(Box::new(port)), written)
}

#[test]
fn test_serial_passthrough_send_writes_bytes_verbatim() {
    let (mut comm, written) = communicator(Vec::new());
    for data in [&b"!"[..], b"~", &[0x18u8], b"g0 x1", b"\n"] {
        let n = comm.send(data).expect("send");
        assert_eq!(n, data.len(), "every byte of {data:?} is written");
    }
    assert_eq!(
        written.lock().expect("written lock").as_slice(),
        b"!~\x18g0 x1\n",
        "no numbering, checksum, case change or filtering"
    );
}

#[test]
fn test_serial_passthrough_receive_returns_raw_bytes() {
    let banner = b"start\nRepetier-Firmware\nok 1\nResend:2\n".to_vec();
    let (mut comm, _written) = communicator(vec![banner.clone()]);

    let data = comm.receive().expect("receive");

    assert_eq!(data, banner, "received bytes are returned unchanged");
}
