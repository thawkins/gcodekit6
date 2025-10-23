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

/// Create a TCP transport boxed as a `Transport` trait object.
pub fn create_tcp_transport(addr: std::net::SocketAddr) -> std::io::Result<Box<dyn Transport>> {
    let conn = network::NetworkConnection::connect_tcp(addr)?;
    Ok(Box::new(conn))
}

pub fn hello_adapters() -> &'static str {
    "gcodekit-device-adapters: ready"
}
