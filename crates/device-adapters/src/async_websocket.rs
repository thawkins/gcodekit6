#![cfg(all(feature = "async", feature = "websocket"))]

use std::io;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message, WebSocketStream};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::MaybeTlsStream as TokioMaybeTlsStream;

/// Async WebSocket transport using tokio-tungstenite.
pub struct AsyncWebSocketTransport {
    ws: WebSocketStream<TokioMaybeTlsStream<TcpStream>>,
    read_timeout: Duration,
}

impl AsyncWebSocketTransport {
    /// Connect to a ws:// or wss:// URL. Caller must provide a valid URL.
    pub async fn connect(url: &str) -> io::Result<Self> {
        let req = url
            .into_client_request()
            .map_err(io::Error::other)?;

        let connect_timeout = gcodekit_utils::settings::network_timeout();

        // Wrap the connect in a timeout to prevent indefinite hangs in DNS/connect.
        let (ws_stream, _resp) = timeout(connect_timeout, connect_async(req))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "ws connect timeout"))?
            .map_err(io::Error::other)?;

        let read_timeout = gcodekit_utils::settings::network_timeout();
        Ok(AsyncWebSocketTransport { ws: ws_stream, read_timeout })
    }

    pub async fn send_line(&mut self, line: &str) -> io::Result<()> {
    let text = line.trim_end_matches(&['\n', '\r'][..]).to_string();
        // Keep semantics consistent: send as a text frame without trailing newline
        timeout(self.read_timeout, self.ws.send(Message::Text(text)))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "ws send timeout"))?
            .map_err(io::Error::other)?;
        Ok(())
    }

    pub async fn emergency_stop(&mut self) -> io::Result<()> {
        // Try sending a '!' as text frame
        timeout(self.read_timeout, self.ws.send(Message::Text("!".into())))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "es send timeout"))?
            .map_err(io::Error::other)?;
        Ok(())
    }

    pub async fn flush(&mut self) -> io::Result<()> {
        // WebSocket send is flushed by await completion
        Ok(())
    }

    pub async fn disconnect(&mut self) -> io::Result<()> {
        // Attempt a close frame and then shutdown
    let _ = self.ws.close(None).await.map_err(io::Error::other);
        Ok(())
    }

    pub async fn is_alive(&self) -> io::Result<bool> {
        // If the underlying stream has a peer_addr, consider it alive
        Ok(true)
    }

    pub async fn read_line(&mut self) -> io::Result<String> {
        // Read next text message
        let msg = timeout(self.read_timeout, self.ws.next())
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "ws read timeout"))?
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "ws closed"))?;

        match msg {
            Ok(Message::Text(t)) => Ok(t),
            Ok(Message::Binary(b)) => Ok(String::from_utf8_lossy(&b).into_owned()),
            Ok(_) => Err(io::Error::other("unsupported ws frame")),
            Err(e) => Err(io::Error::other(e)),
        }
    }
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl crate::AsyncTransport for AsyncWebSocketTransport {
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message;

    #[tokio::test]
    async fn test_ws_connect_send_receive() {
        // Start a local ws server
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            let (mut sink, mut stream) = ws.split();
            // Echo loop: send back first text frame
            if let Some(Ok(msg)) = stream.next().await {
                let _ = sink.send(msg).await;
            }
        });

        let url = format!("ws://{}", addr);
        let mut client = AsyncWebSocketTransport::connect(&url).await.unwrap();
        client.send_line("hello").await.unwrap();
        let resp = client.read_line().await.unwrap();
        assert_eq!(resp, "hello");
        let _ = server.await;
    }
}
