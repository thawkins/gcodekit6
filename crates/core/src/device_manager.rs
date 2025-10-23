use anyhow::Result;
use gcodekit_device_adapters::Transport;
use std::net::SocketAddr;

/// High-level device manager that orchestrates adapter connections.
pub struct DeviceManager {}

impl DeviceManager {
    /// Connect to a network device by socket address and return a boxed `Transport`.
    pub fn connect_network(addr: SocketAddr) -> Result<Box<dyn Transport>> {
        // Use the adapter factory to create a transport instance (TCP for now)
        let transport = gcodekit_device_adapters::create_tcp_transport(addr)?;
        Ok(transport)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn test_connect_network_to_local_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().unwrap();

        // Spawn accept thread
        std::thread::spawn(move || {
            if let Ok((_s, _p)) = listener.accept() {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        });

    let mut conn = DeviceManager::connect_network(addr).expect("connect network");
    // Basic smoke: send a line
    let _ = conn.send_line("M115");
    }
}
