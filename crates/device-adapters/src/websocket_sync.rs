#[cfg(feature = "websocket")]
use std::io;
#[cfg(feature = "websocket")]
use std::net::TcpStream;
#[cfg(feature = "websocket")]
use std::time::Duration;

#[cfg(feature = "websocket")]
use tungstenite::{client, protocol::Message};
#[cfg(feature = "websocket")]
use url::Url;
#[cfg(feature = "websocket")]
use crate::Transport;

/// Synchronous WebSocket transport using blocking tungstenite client.
#[cfg(feature = "websocket")]
pub struct WebSocketTransport {
    ws: tungstenite::protocol::WebSocket<std::net::TcpStream>,
    #[allow(dead_code)]
    timeout: Duration,
}

#[cfg(feature = "websocket")]
impl Transport for WebSocketTransport {
    fn send_line(&mut self, line: &str) -> std::io::Result<()> {
        WebSocketTransport::send_line(self, line)
    }

    fn emergency_stop(&mut self) -> std::io::Result<()> {
        WebSocketTransport::emergency_stop(self)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        WebSocketTransport::flush(self)
    }

    fn disconnect(&mut self) -> std::io::Result<()> {
        WebSocketTransport::disconnect(self)
    }

    fn is_alive(&self) -> std::io::Result<bool> {
        WebSocketTransport::is_alive(self)
    }

    fn read_line(&mut self) -> std::io::Result<String> {
        WebSocketTransport::read_line(self)
    }
}

#[cfg(feature = "websocket")]
impl WebSocketTransport {
    /// Connect to a ws:// URL. Only ws (non-TLS) is supported in this sync implementation.
    pub fn connect(url: &str) -> io::Result<Self> {
    let url = Url::parse(url).map_err(io::Error::other)?;

        // Resolve addresses from the URL
    let addrs = url.socket_addrs(|| None).map_err(io::Error::other)?;
        let timeout = gcodekit_utils::settings::network_timeout();

        // Try to connect to the first resolved address with a connect timeout
        let addr = addrs
            .into_iter()
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no socket addrs"))?;

        let stream = TcpStream::connect_timeout(&addr, timeout).map_err(io::Error::other)?;

        stream.set_read_timeout(Some(timeout)).ok();
        stream.set_write_timeout(Some(timeout)).ok();

    let (ws, _resp) = client(url, stream).map_err(io::Error::other)?;
        Ok(WebSocketTransport { ws, timeout })
    }

    pub fn send_line(&mut self, line: &str) -> io::Result<()> {
        self.ws
            .send(Message::Text(line.trim_end_matches(&['\n', '\r'][..]).to_string()))
            .map_err(io::Error::other)
    }

    pub fn emergency_stop(&mut self) -> io::Result<()> {
        self.ws
            .send(Message::Text("!".to_string()))
            .map_err(io::Error::other)
    }

    pub fn flush(&mut self) -> io::Result<()> {
        // Tungstenite write_message is synchronous and flushed on return
        Ok(())
    }

    pub fn disconnect(&mut self) -> io::Result<()> {
        let _ = self.ws.close(None).map_err(io::Error::other);
        Ok(())
    }

    pub fn is_alive(&self) -> io::Result<bool> {
        // Best-effort: if the peer is set, consider it alive. There is no direct API, return true.
        Ok(true)
    }

    pub fn read_line(&mut self) -> io::Result<String> {
    let msg = self.ws.read().map_err(io::Error::other)?;
        match msg {
            Message::Text(t) => Ok(t),
            Message::Binary(b) => Ok(String::from_utf8_lossy(&b).into_owned()),
            _ => Err(io::Error::other("unsupported ws frame")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use tungstenite::accept;

    #[test]
    fn test_ws_sync_connect_send_receive() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut ws = accept(stream).unwrap();
            if let Ok(msg) = ws.read() {
                let _ = ws.send(msg);
            }
        });

        let url = format!("ws://{}", addr);
        let mut client = WebSocketTransport::connect(&url).unwrap();
        client.send_line("hello").unwrap();
        let resp = client.read_line().unwrap();
        assert_eq!(resp, "hello");
        let _ = server.join();
    }
}
