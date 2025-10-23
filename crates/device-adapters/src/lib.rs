//! Device adapters for GCodeKit6 (GRBL, TinyG, Smoothieware)
pub mod network;
pub mod serial;

/// Transport trait that adapters should implement; simplified for initial plumbing.
pub trait Transport: Send + Sync {
    fn send_line(&mut self, line: &str) -> std::io::Result<()>;
    fn emergency_stop(&mut self) -> std::io::Result<()>;
}

impl Transport for network::NetworkConnection {
    fn send_line(&mut self, line: &str) -> std::io::Result<()> {
        self.send_line(line)
    }
    fn emergency_stop(&mut self) -> std::io::Result<()> {
        self.emergency_stop()
    }
}

pub fn hello_adapters() -> &'static str {
    "gcodekit-device-adapters: ready"
}
