use serialport::available_ports;
use std::io;

/// List available serial ports on the host.
/// Returns a vector of system path strings (e.g., /dev/ttyUSB0 or COM3).
pub fn list_serial_ports() -> io::Result<Vec<String>> {
    match available_ports() {
        Ok(ports) => Ok(ports.into_iter().map(|p| p.port_name).collect()),
        Err(e) => Err(io::Error::new(io::ErrorKind::Other, format!("serialport error: {}", e))),
    }
}
