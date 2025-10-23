//! Device adapters for GCodeKit6 (GRBL, TinyG, Smoothieware)
pub mod network;
pub mod serial;
pub mod async_network;
pub mod async_serial;

/// Transport trait that adapters should implement; simplified for initial plumbing.
pub trait Transport: Send + Sync {
    /// Send a single line/command to the device. Implementations should append
    /// any required line terminator (e.g., newline) if the underlying protocol
    /// requires it.
    fn send_line(&mut self, line: &str) -> std::io::Result<()>;

    /// Attempt to perform an emergency stop on the device. This should be a
    /// fast, best-effort operation that attempts to halt motion or streaming.
    fn emergency_stop(&mut self) -> std::io::Result<()>;

    /// Flush any buffered output to the device. For stream-based transports
    /// this should block until data is handed off to the OS socket/port.
    fn flush(&mut self) -> std::io::Result<()>;

    /// Disconnect/close the transport. After this call the transport should
    /// no longer be usable.
    fn disconnect(&mut self) -> std::io::Result<()>;

    /// Check whether the transport appears to be alive/connected. This should
    /// be a lightweight check suitable for polling from higher-level logic.
    fn is_alive(&self) -> std::io::Result<bool>;
    /// Read a single line (terminated by newline) from the transport. Returns
    /// the line without the trailing newline.
    fn read_line(&mut self) -> std::io::Result<String>;
}

/// AsyncTransport: an async counterpart to `Transport` that uses async I/O.
#[cfg(feature = "async")]
#[async_trait::async_trait]
pub trait AsyncTransport: Send + Sync {
    async fn send_line(&mut self, line: &str) -> std::io::Result<()>;
    async fn emergency_stop(&mut self) -> std::io::Result<()>;
    async fn flush(&mut self) -> std::io::Result<()>;
    async fn disconnect(&mut self) -> std::io::Result<()>;
    async fn is_alive(&self) -> std::io::Result<bool>;
    async fn read_line(&mut self) -> std::io::Result<String>;
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl AsyncTransport for async_network::AsyncTcpTransport {
    async fn send_line(&mut self, line: &str) -> std::io::Result<()> {
        self.send_line(line).await
    }

    async fn emergency_stop(&mut self) -> std::io::Result<()> {
        self.emergency_stop().await
    }

    async fn flush(&mut self) -> std::io::Result<()> {
        self.flush().await
    }

    async fn disconnect(&mut self) -> std::io::Result<()> {
        self.disconnect().await
    }

    async fn is_alive(&self) -> std::io::Result<bool> {
        self.is_alive().await
    }

    async fn read_line(&mut self) -> std::io::Result<String> {
        self.read_line().await
    }
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl AsyncTransport for async_serial::AsyncSerialTransport {
    async fn send_line(&mut self, line: &str) -> std::io::Result<()> {
        self.send_line(line).await
    }

    async fn emergency_stop(&mut self) -> std::io::Result<()> {
        self.emergency_stop().await
    }

    async fn flush(&mut self) -> std::io::Result<()> {
        self.flush().await
    }

    async fn disconnect(&mut self) -> std::io::Result<()> {
        self.disconnect().await
    }

    async fn is_alive(&self) -> std::io::Result<bool> {
        self.is_alive().await
    }

    async fn read_line(&mut self) -> std::io::Result<String> {
        self.read_line().await
    }
}

/// Factory to create an async serial transport from path and options. This wraps the blocking serial port.
#[cfg(feature = "async")]
pub fn create_serial_async_transport_with_options(path: &str, opts: SerialOptions) -> std::io::Result<Box<dyn AsyncTransport>> {
    let conn = serial::SerialConnection::open_with_options(path, opts)?;
    let wrapper = async_serial::AsyncSerialTransport::wrap(conn);
    Ok(Box::new(wrapper))
}

impl Transport for network::NetworkConnection {
    fn send_line(&mut self, line: &str) -> std::io::Result<()> {
        network::NetworkConnection::send_line(self, line)
    }

    fn emergency_stop(&mut self) -> std::io::Result<()> {
        network::NetworkConnection::emergency_stop(self)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        network::NetworkConnection::flush(self)
    }

    fn disconnect(&mut self) -> std::io::Result<()> {
        network::NetworkConnection::disconnect(self)
    }

    fn is_alive(&self) -> std::io::Result<bool> {
        network::NetworkConnection::is_alive(self)
    }

    fn read_line(&mut self) -> std::io::Result<String> {
        network::NetworkConnection::read_line(self)
    }
}

/// Create a TCP transport boxed as a `Transport` trait object.
pub fn create_tcp_transport(addr: std::net::SocketAddr) -> std::io::Result<Box<dyn Transport>> {
    let conn = network::NetworkConnection::connect_tcp(addr)?;
    Ok(Box::new(conn))
}

/// Create a serial transport from a device path and baud rate.
pub fn create_serial_transport(path: &str, baud: u32, timeout: std::time::Duration) -> std::io::Result<Box<dyn Transport>> {
    let conn = serial::SerialConnection::open(path, baud, timeout)?;
    Ok(Box::new(conn))
}

/// Serial transport options (parity/flow-control can be extended).
pub struct SerialOptions {
    pub baud: u32,
    pub timeout: std::time::Duration,
    pub parity: Option<String>,
    pub flow_control: Option<String>,
}

/// Create a serial transport with extra options.
pub fn create_serial_transport_with_options(path: &str, opts: SerialOptions) -> std::io::Result<Box<dyn Transport>> {
    let conn = serial::SerialConnection::open_with_options(path, opts)?;
    Ok(Box::new(conn))
}

impl Transport for serial::SerialConnection {
    fn send_line(&mut self, line: &str) -> std::io::Result<()> {
        serial::SerialConnection::send_line(self, line)
    }

    fn emergency_stop(&mut self) -> std::io::Result<()> {
        serial::SerialConnection::emergency_stop(self)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        serial::SerialConnection::flush(self)
    }

    fn disconnect(&mut self) -> std::io::Result<()> {
        serial::SerialConnection::disconnect(self)
    }

    fn is_alive(&self) -> std::io::Result<bool> {
        serial::SerialConnection::is_alive(self)
    }

    fn read_line(&mut self) -> std::io::Result<String> {
        serial::SerialConnection::read_line(self)
    }
}

pub fn hello_adapters() -> &'static str {
    "gcodekit-device-adapters: ready"
}
