//! WebSocket transport for network-attached controllers.
//!
//! FluidNC and grblHAL boards expose a WebSocket (FluidNC's default is `ws://<host>:81/`)
//! that carries the same GRBL text protocol as their serial port: each message is a run of
//! bytes from the controller's byte stream. [`RealWebSocketPort`] presents that socket
//! through the [`SerialPort`] byte-stream interface, so `SerialCommunicator` streams,
//! polls and sends real-time bytes over it exactly as it does over serial or TCP.
//!
//! Every write is sent at once as one message: a real-time byte (`?`, `!`, `~`, `0x18`)
//! written on its own goes out unbuffered, in its own message. Text that is valid UTF-8 is
//! sent as a text message and anything else (the extended real-time bytes `0x80`-`0xFF`)
//! as a binary message; both controllers read either kind as raw bytes.
//!
//! Only plain `ws://` is supported: controllers on a local network do not serve TLS.
//!
//! Dependencies: `tungstenite` (blocking WebSocket client over `std::net::TcpStream`).

use std::io;
use std::net::TcpStream;
use std::time::Duration;

use gcodekit6_core::{Error, Result};
use tungstenite::{Message, WebSocket};

use super::serial::SerialPort;
use super::tcp::connect_stream;
use crate::{ConnectionDriver, ConnectionParams};

/// The URL a WebSocket connection to `host:port` opens.
///
/// An IPv6 literal is put in brackets, as URLs require.
///
/// # Arguments
/// * `host` - Hostname, IPv4 or IPv6 address.
/// * `port` - The WebSocket port (FluidNC's default is 81).
pub fn websocket_url(host: &str, port: u16) -> String {
    let host = host.trim();
    if host.contains(':') && !host.starts_with('[') {
        format!("ws://[{host}]:{port}/")
    } else {
        format!("ws://{host}:{port}/")
    }
}

/// A WebSocket connection to a controller, read and written as a byte stream.
pub struct RealWebSocketPort {
    socket: WebSocket<TcpStream>,
    /// Received bytes that did not fit the caller's buffer, returned by the next read.
    pending: Vec<u8>,
    name: String,
}

impl RealWebSocketPort {
    /// Opens a WebSocket connection with the given parameters.
    ///
    /// Connects over TCP to `params.port:params.network_port` (every resolved address is
    /// tried), performs the WebSocket handshake within `params.timeout_ms`, then sets the
    /// read timeout to `read_timeout` so an idle read returns quickly.
    ///
    /// # Arguments
    /// * `params` - Connection parameters with the [`ConnectionDriver::WebSocket`] driver.
    /// * `read_timeout` - Read timeout once connected (the poll interval).
    ///
    /// # Errors
    /// Fails if the driver is not WebSocket, the host cannot be reached, or the server
    /// refuses the WebSocket handshake.
    pub fn open(params: &ConnectionParams, read_timeout: Duration) -> Result<Self> {
        if params.driver != ConnectionDriver::WebSocket {
            return Err(Error::other(
                "RealWebSocketPort requires WebSocket driver type",
            ));
        }
        let url = websocket_url(&params.port, params.network_port);
        let stream = connect_stream(params)?;
        let (socket, _response) =
            tungstenite::client::client(url.as_str(), stream).map_err(|e| {
                let reason = match e {
                    tungstenite::HandshakeError::Interrupted(_) => "handshake timed out".into(),
                    tungstenite::HandshakeError::Failure(e) => e.to_string(),
                };
                tracing::error!("WebSocket handshake with {} failed: {}", url, reason);
                Error::other(format!("WebSocket handshake with {url} failed: {reason}"))
            })?;
        socket
            .get_ref()
            .set_read_timeout(Some(read_timeout))
            .map_err(|e| Error::other(format!("Failed to set WebSocket read timeout: {e}")))?;
        tracing::info!("WebSocket connected to {}", url);
        Ok(Self {
            socket,
            pending: Vec::new(),
            name: url,
        })
    }

    /// Copies as much of `self.pending` into `buf` as fits and returns the count.
    fn drain_pending(&mut self, buf: &mut [u8]) -> usize {
        let n = self.pending.len().min(buf.len());
        buf[..n].copy_from_slice(&self.pending[..n]);
        self.pending.drain(..n);
        n
    }
}

/// The I/O error a WebSocket error reads as: an I/O error as it is (so a read timeout
/// still means "no data"), anything else (a close, a protocol violation) as a closed
/// connection.
fn io_error(e: tungstenite::Error) -> io::Error {
    match e {
        tungstenite::Error::Io(e) => e,
        tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed => io::Error::new(
            io::ErrorKind::ConnectionAborted,
            "connection closed by the controller",
        ),
        other => io::Error::new(io::ErrorKind::ConnectionAborted, other.to_string()),
    }
}

impl SerialPort for RealWebSocketPort {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let message = match std::str::from_utf8(data) {
            Ok(text) => Message::text(text),
            Err(_) => Message::binary(data.to_vec()),
        };
        self.socket.send(message).map_err(io_error)?;
        Ok(data.len())
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if !self.pending.is_empty() {
            return Ok(self.drain_pending(buf));
        }
        loop {
            match self.socket.read().map_err(io_error)? {
                Message::Text(text) => self.pending.extend_from_slice(text.as_bytes()),
                Message::Binary(data) => self.pending.extend_from_slice(&data),
                Message::Close(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::ConnectionAborted,
                        "connection closed by the controller",
                    ));
                }
                // Pings are answered by tungstenite itself; nothing to hand out.
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
            }
            if !self.pending.is_empty() {
                return Ok(self.drain_pending(buf));
            }
        }
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn close(&mut self) -> io::Result<()> {
        // Best effort: say goodbye, then drop the TCP stream whatever the peer does.
        if let Err(e) = self.socket.close(None) {
            tracing::debug!("WebSocket close failed: {}", e);
        }
        if let Err(e) = self.socket.flush() {
            tracing::debug!("WebSocket close flush failed: {}", e);
        }
        let _ = self.socket.get_ref().shutdown(std::net::Shutdown::Both);
        Ok(())
    }
}
