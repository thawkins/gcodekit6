//! Regression tests for the communication audit follow-ups: serial port
//! filtering and GRBL probe report parsing.

use gcodekit5_communication::communication::serial::is_valid_cnc_port_name;
use gcodekit5_communication::firmware::grbl::status_parser::parse_probe_report;

// ---------- Item 5: serial port filtering ----------

#[test]
fn test_serial_port_filter_includes_uarts_and_bluetooth() {
    for name in [
        "/dev/ttyUSB0",
        "/dev/ttyACM1",
        "/dev/ttyS0",
        "/dev/ttyAMA0",
        "/dev/rfcomm0",
        "/dev/ttyGRBL",
        "COM3",
        "/dev/cu.usbmodem1421",
        "/dev/cu.HC-05",
    ] {
        assert!(is_valid_cnc_port_name(name), "{name} should be listed");
    }
}

#[test]
fn test_serial_port_filter_excludes_non_devices() {
    for name in [
        "/dev/tty1",
        "/dev/tty",
        "/dev/console",
        "/dev/ptmx",
        "/dev/pts/0",
        "/dev/ttyS",
        "/dev/tty.usbmodem1421",
        "/dev/cu.Bluetooth-Incoming-Port",
        "/dev/cu.debug-console",
        "COM",
        "COMX",
    ] {
        assert!(!is_valid_cnc_port_name(name), "{name} should be hidden");
    }
}

// ---------- Item 10: GRBL probing and work offsets ----------

#[test]
fn test_grbl_probe_report_parsing() {
    let ok = parse_probe_report("[PRB:1.500,-2.250,-10.125:1]").expect("PRB");
    assert!(ok.success);
    assert_eq!(ok.position.x, 1.5);
    assert_eq!(ok.position.y, -2.25);
    assert_eq!(ok.position.z, -10.125);

    let miss = parse_probe_report("[PRB:0.000,0.000,-5.000:0]").expect("PRB");
    assert!(!miss.success);

    let four_axis = parse_probe_report("[PRB:1.0,2.0,3.0,45.0:1]").expect("PRB 4-axis");
    assert_eq!(four_axis.position.a, Some(45.0));

    assert!(parse_probe_report("[PRB:1.0,2.0]").is_none());
    assert!(parse_probe_report("[GC:G0 G54]").is_none());
    assert!(parse_probe_report("ok").is_none());
}

/// What one write call on a [`ScriptedPort`] does.
#[derive(Clone, Copy)]
enum WriteStep {
    /// Accept at most this many bytes.
    Accept(usize),
    /// Fail with `TimedOut`.
    TimeOut,
}

/// A fake serial stream that answers writes from a script (then accepts everything)
/// and records the bytes it accepted.
struct ScriptedPort {
    steps: std::collections::VecDeque<WriteStep>,
    written: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
}

impl std::io::Read for ScriptedPort {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::TimedOut.into())
    }
}

impl std::io::Write for ScriptedPort {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = match self.steps.pop_front() {
            Some(WriteStep::TimeOut) => {
                // A real port blocks for its timeout before reporting it.
                std::thread::sleep(std::time::Duration::from_millis(1));
                return Err(std::io::ErrorKind::TimedOut.into());
            }
            Some(WriteStep::Accept(n)) => n.min(buf.len()),
            None => buf.len(),
        };
        self.written
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend_from_slice(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Writes `line` through a `RealSerialPort` over a [`ScriptedPort`] running `steps`.
///
/// # Returns
/// The write result and the bytes the fake accepted.
fn scripted_write(
    steps: &[WriteStep],
    timeout: std::time::Duration,
    line: &[u8],
) -> (std::io::Result<usize>, Vec<u8>) {
    use gcodekit5_communication::communication::serial::{RealSerialPort, SerialPort};
    let written = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let fake = ScriptedPort {
        steps: steps.iter().copied().collect(),
        written: written.clone(),
    };
    let mut port = RealSerialPort::from_port(Box::new(fake), timeout);
    let result = port.write(line);
    let bytes = written
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    (result, bytes)
}

/// A serial write that times out twice before the port accepts it completes (APP-7).
#[test]
fn test_serial_write_retries_timeouts_until_written() {
    let line = b"G1 X10 F1000\n";
    let steps = [WriteStep::TimeOut, WriteStep::TimeOut];
    let (result, bytes) = scripted_write(&steps, std::time::Duration::from_secs(5), line);
    assert_eq!(result.expect("write completes"), line.len());
    assert_eq!(bytes, line);
}

/// A timeout after a partial write resumes after the bytes already sent: nothing is
/// repeated or dropped (APP-7).
#[test]
fn test_serial_write_resumes_after_partial_write_timeout() {
    let line = b"G1 X10 F1000\n";
    let steps = [WriteStep::Accept(3), WriteStep::TimeOut, WriteStep::TimeOut];
    let (result, bytes) = scripted_write(&steps, std::time::Duration::from_secs(5), line);
    assert_eq!(result.expect("write completes"), line.len());
    assert_eq!(bytes, line);
}

/// A port that never accepts the data still fails once the write timeout passes.
#[test]
fn test_serial_write_fails_after_write_timeout() {
    let steps = vec![WriteStep::TimeOut; 10_000];
    let (result, bytes) = scripted_write(&steps, std::time::Duration::from_millis(20), b"G0\n");
    let err = result.expect_err("write must time out");
    assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
    assert!(bytes.is_empty());
}
