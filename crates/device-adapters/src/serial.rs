use serialport::available_ports;
use serialport::SerialPort;
use std::io;
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A thin wrapper around a boxed `SerialPort` to provide higher-level
/// operations used by the Transport trait. The inner port is wrapped in an
/// Arc<Mutex<...>> so the connection can be shared across threads and still
/// satisfy `Sync` for the Transport trait object.
pub struct SerialConnection {
    port: Arc<Mutex<Box<dyn SerialPort>>>,
}

impl SerialConnection {
    /// Open a serial device path at the given baud rate and return a
    /// `SerialConnection`.
    pub fn open(path: &str, baud: u32, timeout: Duration) -> io::Result<Self> {
        let opts = super::SerialOptions {
            baud,
            timeout,
            parity: None,
            flow_control: None,
        };
        SerialConnection::open_with_options(path, opts)
    }

    pub fn open_with_options(path: &str, opts: super::SerialOptions) -> io::Result<Self> {
        // For now we only use baud and timeout; parity/flow_control are
        // accepted but not applied. Extend this with serialport settings as
        // needed per platform.
        match serialport::new(path, opts.baud)
            .timeout(opts.timeout)
            .open()
        {
            Ok(p) => Ok(SerialConnection {
                port: Arc::new(Mutex::new(p)),
            }),
            Err(e) => Err(io::Error::other(format!("serial open: {}", e))),
        }
    }

    pub fn send_line(&mut self, line: &str) -> io::Result<()> {
        let mut guard = self
            .port
            .lock()
            .map_err(|_| io::Error::other("mutex poisoned"))?;
        guard.write_all(line.as_bytes())?;
        guard.write_all(b"\n")?;
        Ok(())
    }

    pub fn emergency_stop(&mut self) -> io::Result<()> {
        let mut guard = self
            .port
            .lock()
            .map_err(|_| io::Error::other("mutex poisoned"))?;
        guard.write_all(b"!")?;
        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        let mut guard = self
            .port
            .lock()
            .map_err(|_| io::Error::other("mutex poisoned"))?;
        guard
            .flush()
            .map_err(|e| io::Error::other(format!("flush: {}", e)))
    }

    pub fn disconnect(&mut self) -> io::Result<()> {
        // Dropping the Arc/Mutex-held port will close the device. Explicitly
        // dropping the locked value isn't necessary here; callers should
        // drop the SerialConnection when they want the port closed. We keep a
        // no-op implementation to satisfy the Transport contract.
        Ok(())
    }

    pub fn is_alive(&self) -> io::Result<bool> {
        // Conservative approach: assume alive if we can lock the port. A more
        // thorough check could try a non-blocking read or ioctl, but that's
        // platform-specific.
        match self.port.lock() {
            Ok(_) => Ok(true),
                Err(_) => Err(io::Error::other("mutex poisoned")),
        }
    }

    /// Read a line from the serial port (blocking until newline) and return
    /// it without the trailing newline.
    pub fn read_line(&mut self) -> io::Result<String> {
        let mut guard = self
            .port
            .lock()
                .map_err(|_| io::Error::other("mutex poisoned"))?;
        let mut reader = std::io::BufReader::new(guard.as_mut());
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| io::Error::other(format!("read_line: {}", e)))?;
        if line.ends_with('\n') {
            line.truncate(line.len() - 1);
        }
        Ok(line)
    }
}

/// List available serial ports on the host.
/// Returns a vector of system path strings (e.g., /dev/ttyUSB0 or COM3).
pub fn list_serial_ports() -> io::Result<Vec<String>> {
    match available_ports() {
        Ok(ports) => Ok(ports.into_iter().map(|p| p.port_name).collect()),
        Err(e) => Err(io::Error::other(format!("serialport error: {}", e))),
    }
}
