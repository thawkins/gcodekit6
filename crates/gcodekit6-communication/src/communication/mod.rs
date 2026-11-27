//! gcodekit6-communication/src/communication/mod.rs
//! Communication layer for serial, TCP, and WebSocket protocols
//!
//! Provides trait-based communication interface for different connection types.
//!
//! # Features
//! - Pluggable communication backends
//! - Serial (USB) communication
//! - TCP/IP network communication  
//! - WebSocket communication
//! - Event callbacks for connection state changes
//! - Configurable connection parameters

pub mod reply_ledger;
pub mod serial;
pub mod tcp;
pub mod websocket;

use serde::{Deserialize, Serialize};
use std::fmt;

pub use serial::{SerialPortInfo, list_ports};
pub use tcp::TcpConnectionInfo;
pub use websocket::websocket_url;

/// Connection driver type
///
/// Specifies the underlying protocol for communication with the CNC controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionDriver {
    /// Serial port (USB/RS-232)
    Serial,
    /// TCP/IP network connection
    Tcp,
    /// WebSocket connection
    WebSocket,
}

impl fmt::Display for ConnectionDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serial => write!(f, "serial"),
            Self::Tcp => write!(f, "tcp"),
            Self::WebSocket => write!(f, "websocket"),
        }
    }
}

/// Connection parameters for establishing communication
///
/// Contains all information needed to establish a connection with a CNC controller.
/// The required fields depend on the connection driver type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectionParams {
    /// Connection driver type
    pub driver: ConnectionDriver,

    /// Port name (e.g., "/dev/ttyUSB0" for serial, "192.168.1.100" for TCP)
    pub port: String,

    /// Port number (used for TCP/WebSocket connections, typically 8888 for g2core, 23 for GRBL)
    pub network_port: u16,

    /// Baud rate for serial connections (115200 typical for GRBL)
    pub baud_rate: u32,

    /// Connection timeout in milliseconds
    pub timeout_ms: u64,

    /// Whether to use RTS/CTS flow control (serial only)
    pub flow_control: bool,

    /// Number of data bits (serial only, typically 8)
    pub data_bits: u8,

    /// Number of stop bits (serial only, typically 1)
    pub stop_bits: u8,

    /// Parity setting (serial only)
    pub parity: SerialParity,

    /// Whether to enable automatic reconnection on disconnect
    pub auto_reconnect: bool,

    /// Maximum number of reconnection attempts before giving up
    pub max_retries: u32,
}

/// Serial port parity setting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SerialParity {
    /// No parity bit
    None,
    /// Even parity
    Even,
    /// Odd parity
    Odd,
}

impl Default for ConnectionParams {
    fn default() -> Self {
        Self {
            driver: ConnectionDriver::Serial,
            port: "/dev/ttyUSB0".to_string(),
            network_port: 8888,
            baud_rate: 115200,
            timeout_ms: 5000,
            flow_control: false,
            data_bits: 8,
            stop_bits: 1,
            parity: SerialParity::None,
            auto_reconnect: true,
            max_retries: 3,
        }
    }
}

impl ConnectionParams {
    /// Create connection parameters for serial communication
    pub fn serial(port: &str, baud_rate: u32) -> Self {
        Self {
            driver: ConnectionDriver::Serial,
            port: port.to_string(),
            baud_rate,
            ..Default::default()
        }
    }

    /// Create connection parameters for TCP communication
    pub fn tcp(host: &str, port: u16) -> Self {
        Self {
            driver: ConnectionDriver::Tcp,
            port: host.to_string(),
            network_port: port,
            ..Default::default()
        }
    }

    /// Create connection parameters for WebSocket communication
    pub fn websocket(host: &str, port: u16) -> Self {
        Self {
            driver: ConnectionDriver::WebSocket,
            port: host.to_string(),
            network_port: port,
            ..Default::default()
        }
    }

    /// Validate the connection parameters
    pub fn validate(&self) -> gcodekit6_core::Result<()> {
        match self.driver {
            ConnectionDriver::Serial => {
                if self.port.is_empty() {
                    return Err(gcodekit6_core::Error::other(
                        "Serial port name cannot be empty",
                    ));
                }
                if self.baud_rate == 0 {
                    return Err(gcodekit6_core::Error::other("Baud rate must be > 0"));
                }
                if self.data_bits == 0 || self.data_bits > 8 {
                    return Err(gcodekit6_core::Error::other("Data bits must be 5-8"));
                }
            }
            ConnectionDriver::Tcp | ConnectionDriver::WebSocket => {
                if self.port.is_empty() {
                    return Err(gcodekit6_core::Error::other("Host/address cannot be empty"));
                }
                if self.network_port == 0 {
                    return Err(gcodekit6_core::Error::other("Network port must be > 0"));
                }
            }
        }

        if self.timeout_ms == 0 {
            return Err(gcodekit6_core::Error::other("Timeout must be > 0"));
        }

        Ok(())
    }
}

/// Abstract communicator trait for the connection to a CNC controller
///
/// The production implementor is [`SerialCommunicator`], which carries every
/// [`ConnectionDriver`]: serial ports directly, and TCP and WebSocket
/// connections as port backends ([`tcp::RealTcpPort`] and
/// [`websocket::RealWebSocketPort`]) behind the same byte-stream interface.
/// [`NoOpCommunicator`] is the do-nothing implementor used when no connection
/// exists; the app crate's test support adds an in-memory loopback.
pub trait Communicator: Send + Sync {
    /// Connect to the device/server using the provided parameters
    fn connect(&mut self, params: &ConnectionParams) -> gcodekit6_core::Result<()>;

    /// Disconnect from the device/server
    fn disconnect(&mut self) -> gcodekit6_core::Result<()>;

    /// Check if currently connected
    fn is_connected(&self) -> bool;

    /// Send raw data to the device
    ///
    /// Returns the number of bytes sent
    fn send(&mut self, data: &[u8]) -> gcodekit6_core::Result<usize>;

    /// Receive data from the device
    ///
    /// Returns a vector of received bytes. May return empty vector if no data available.
    fn receive(&mut self) -> gcodekit6_core::Result<Vec<u8>>;

    /// Send a text command with newline termination
    ///
    /// Convenience method that sends a command string followed by newline.
    fn send_command(&mut self, command: &str) -> gcodekit6_core::Result<()> {
        self.send(command.as_bytes())?;
        self.send(b"\n")?;
        Ok(())
    }

    /// Sends a command and attempts to receive a response.
    fn send_command_and_receive(&mut self, command: &str) -> gcodekit6_core::Result<Vec<u8>> {
        self.send_command(command)?;
        self.receive()
    }

    /// Get connection parameters
    fn connection_params(&self) -> Option<&ConnectionParams>;

    /// Set connection parameters (without connecting)
    fn set_connection_params(&mut self, params: ConnectionParams) -> gcodekit6_core::Result<()>;

    /// Get the connection driver type
    fn driver_type(&self) -> ConnectionDriver {
        self.connection_params()
            .map(|p| p.driver)
            .unwrap_or(ConnectionDriver::Serial)
    }

    /// Get the port name for this connection
    fn port_name(&self) -> String {
        self.connection_params()
            .map(|p| p.port.clone())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

/// No-op communicator for testing
pub struct NoOpCommunicator {
    connected: bool,
    params: Option<ConnectionParams>,
}

impl NoOpCommunicator {
    /// Create a new no-op communicator
    pub fn new() -> Self {
        Self {
            connected: false,
            params: None,
        }
    }
}

impl Default for NoOpCommunicator {
    fn default() -> Self {
        Self::new()
    }
}

impl Communicator for NoOpCommunicator {
    fn connect(&mut self, params: &ConnectionParams) -> gcodekit6_core::Result<()> {
        params.validate()?;
        self.params = Some(params.clone());
        self.connected = true;
        Ok(())
    }

    fn disconnect(&mut self) -> gcodekit6_core::Result<()> {
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn send(&mut self, data: &[u8]) -> gcodekit6_core::Result<usize> {
        if !self.connected {
            return Err(gcodekit6_core::Error::other("Not connected"));
        }
        Ok(data.len())
    }

    fn receive(&mut self) -> gcodekit6_core::Result<Vec<u8>> {
        if !self.connected {
            return Err(gcodekit6_core::Error::other("Not connected"));
        }
        Ok(vec![])
    }

    fn connection_params(&self) -> Option<&ConnectionParams> {
        self.params.as_ref()
    }

    fn set_connection_params(&mut self, params: ConnectionParams) -> gcodekit6_core::Result<()> {
        params.validate()?;
        self.params = Some(params);
        Ok(())
    }
}

/// Serial/USB communicator for direct hardware connection
///
/// Provides full serial port communication using the serialport crate.
/// Supports configurable baud rates, parity, stop bits, and flow control.
pub struct SerialCommunicator {
    port: Option<Box<dyn serial::SerialPort>>,
    params: Option<ConnectionParams>,

    /// Lines written on the GRBL path that await their `ok` / `error:N`.
    reply_ledger: reply_ledger::ReplyLedger,

    /// Received bytes handed back with [`Self::unread`]; returned by the next
    /// `receive` before anything is read from the port.
    unread: Vec<u8>,
}

impl SerialCommunicator {
    /// Create a new serial communicator
    pub fn new() -> Self {
        Self {
            port: None,
            params: None,
            reply_ledger: reply_ledger::ReplyLedger::new(),
            unread: Vec::new(),
        }
    }

    /// Hands received bytes back, so the next `receive` returns them first.
    ///
    /// A reader that stops part-way through what it received (such as a job
    /// streamer ending on `error:N`) uses this so the replies it did not
    /// process still reach the next reader, in order.
    ///
    /// # Arguments
    /// * `data` - Bytes to return before any new data from the port.
    pub fn unread(&mut self, data: &[u8]) {
        self.unread.extend_from_slice(data);
    }

    /// Sends `command` as one line and tags it, so its reply can be recognised
    /// with [`Self::take_reply_tag`].
    ///
    /// # Arguments
    /// * `command` - The line to send, without the newline.
    /// * `tag` - Identifier returned by `take_reply_tag` for this line's reply.
    ///
    /// # Errors
    /// Fails if the line could not be written.
    pub fn send_tagged_command(&mut self, command: &str, tag: u64) -> gcodekit6_core::Result<()> {
        self.send_command(command)?;
        self.reply_ledger.tag_last(tag);
        Ok(())
    }

    /// Records one `ok` / `error:N` reply and returns the tag of the line it
    /// answers (see [`Self::send_tagged_command`]), if that line was tagged.
    pub fn take_reply_tag(&mut self) -> Option<u64> {
        self.reply_ledger.on_reply()
    }

    /// Forgets the lines awaiting a reply, for when the controller will not
    /// answer them (a reset, or a streamed job that reads its own replies).
    pub fn clear_reply_ledger(&mut self) {
        self.reply_ledger.clear();
    }

    /// Creates a communicator already connected through `port`.
    ///
    /// For tests: lets a fake [`serial::SerialPort`] stand in for hardware.
    #[doc(hidden)]
    pub fn with_port(port: Box<dyn serial::SerialPort>) -> Self {
        Self {
            port: Some(port),
            ..Self::new()
        }
    }

    /// Drops a port whose connection is gone, so `is_connected()` is false.
    /// Returns the error to report.
    fn drop_dead_port(&mut self, e: &std::io::Error) -> gcodekit6_core::Error {
        if let Some(mut port) = self.port.take()
            && let Err(close_err) = port.close()
        {
            tracing::debug!("Error closing dead port: {}", close_err);
        }
        let msg = format!("Connection lost: {}", e);
        tracing::warn!("{}", msg);
        gcodekit6_core::Error::other(msg)
    }

    /// Turns a port write error into the error `send` returns. A write that
    /// fails because the connection is gone (cable pulled, peer closed)
    /// drops the port, like the same error on a read.
    fn write_failed(&mut self, e: std::io::Error) -> gcodekit6_core::Error {
        if is_terminal_io_error(e.kind()) {
            self.drop_dead_port(&e)
        } else {
            gcodekit6_core::Error::other(format!("Send error: {}", e))
        }
    }
}

impl Default for SerialCommunicator {
    fn default() -> Self {
        Self::new()
    }
}

/// Presents a TCP connection through the [`serial::SerialPort`] interface, so the
/// byte-stream code built on [`SerialCommunicator`] works unchanged over the network
/// (e.g. FluidNC or grblHAL controllers reached by Wi-Fi/Ethernet).
struct TcpAsSerialPort {
    inner: Box<dyn tcp::TcpPort>,
    name: String,
}

impl serial::SerialPort for TcpAsSerialPort {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.inner.write(data)
    }

    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.inner.read(buf) {
            // A zero-byte read on a TCP stream means the peer closed the connection;
            // report it instead of returning "no data" forever.
            Ok(0) if !buf.is_empty() => Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "connection closed by the controller",
            )),
            other => other,
        }
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn close(&mut self) -> std::io::Result<()> {
        self.inner.close()
    }
}

/// Returns true for I/O errors that mean the connection itself is gone (as opposed to
/// a timeout or a transient fault), so the port must be dropped.
fn is_terminal_io_error(kind: std::io::ErrorKind) -> bool {
    matches!(
        kind,
        std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::UnexpectedEof
            | std::io::ErrorKind::NotConnected
    )
}

impl Communicator for SerialCommunicator {
    /// Opens the connection described by `params`.
    ///
    /// Serial ports are opened directly; TCP connections are opened with
    /// [`tcp::RealTcpPort`] and WebSocket connections with
    /// [`websocket::RealWebSocketPort`], both carried through the same byte-stream
    /// interface.
    fn connect(&mut self, params: &ConnectionParams) -> gcodekit6_core::Result<()> {
        params.validate()?;
        let opened: gcodekit6_core::Result<Box<dyn serial::SerialPort>> = match params.driver {
            ConnectionDriver::Serial => serial::RealSerialPort::open(params)
                .map(|p| Box::new(p) as Box<dyn serial::SerialPort>),
            ConnectionDriver::Tcp => tcp::RealTcpPort::open(params).and_then(|p| {
                // `open` applies `timeout_ms` to connect, read and write. Reads here are
                // polled with the communicator lock held, so use the same short timeout
                // as the serial port; `timeout_ms` still governs connecting and writes.
                p.set_read_timeout(Some(serial::POLL_READ_TIMEOUT))
                    .map_err(|e| {
                        gcodekit6_core::Error::other(format!("Failed to set TCP read timeout: {e}"))
                    })?;
                Ok(Box::new(TcpAsSerialPort {
                    inner: Box::new(p),
                    name: format!("{}:{}", params.port, params.network_port),
                }) as Box<dyn serial::SerialPort>)
            }),
            // As for TCP, `timeout_ms` governs connecting and writes; reads are polled.
            ConnectionDriver::WebSocket => {
                websocket::RealWebSocketPort::open(params, serial::POLL_READ_TIMEOUT)
                    .map(|p| Box::new(p) as Box<dyn serial::SerialPort>)
            }
        };

        // Forget the previous connection's pending replies and unread bytes
        self.reply_ledger.clear();
        self.unread.clear();

        match opened {
            Ok(port) => {
                self.port = Some(port);
                self.params = Some(params.clone());
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn disconnect(&mut self) -> gcodekit6_core::Result<()> {
        self.reply_ledger.clear();
        self.unread.clear();

        match self.port.take() {
            Some(mut port) => match port.close() {
                Ok(()) => Ok(()),
                Err(e) => Err(gcodekit6_core::Error::other(format!(
                    "Error closing port: {}",
                    e
                ))),
            },
            _ => Ok(()),
        }
    }

    fn is_connected(&self) -> bool {
        self.port.is_some()
    }

    fn send(&mut self, data: &[u8]) -> gcodekit6_core::Result<usize> {
        if let Some(port) = &mut self.port {
            match port.write(data) {
                Ok(n) => {
                    tracing::debug!("[TX] (raw) {:?}", String::from_utf8_lossy(data));
                    // Each complete line written gets exactly one reply.
                    self.reply_ledger
                        .on_lines_sent(data[..n].iter().filter(|&&b| b == b'\n').count());
                    Ok(n)
                }
                Err(e) => Err(self.write_failed(e)),
            }
        } else {
            Err(gcodekit6_core::Error::other("Not connected to serial port"))
        }
    }

    fn receive(&mut self) -> gcodekit6_core::Result<Vec<u8>> {
        if !self.unread.is_empty() && self.port.is_some() {
            return Ok(std::mem::take(&mut self.unread));
        }
        if let Some(port) = &mut self.port {
            let mut buf = [0u8; 4096];
            match port.read(&mut buf) {
                Ok(n) => {
                    let data = buf[..n].to_vec();

                    if !data.is_empty() {
                        tracing::debug!(
                            "[RX] {}",
                            String::from_utf8_lossy(&data)
                                .trim_end()
                                .replace('\n', " | ")
                        );
                    }
                    Ok(data)
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut
                    {
                        Ok(vec![])
                    } else if is_terminal_io_error(e.kind()) {
                        // The connection is gone (serialport reports a pulled USB
                        // cable as BrokenPipe): drop the port so is_connected() is
                        // false and the UI sees the disconnect.
                        Err(self.drop_dead_port(&e))
                    } else {
                        Err(gcodekit6_core::Error::other(format!(
                            "Receive error: {}",
                            e
                        )))
                    }
                }
            }
        } else {
            Err(gcodekit6_core::Error::other("Not connected to serial port"))
        }
    }

    fn connection_params(&self) -> Option<&ConnectionParams> {
        self.params.as_ref()
    }

    fn set_connection_params(&mut self, params: ConnectionParams) -> gcodekit6_core::Result<()> {
        params.validate()?;
        self.params = Some(params);
        Ok(())
    }
}
