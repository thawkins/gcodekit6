use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{debug, info};

/// Async TCP transport using tokio::net::TcpStream
pub struct AsyncTcpTransport {
    stream: TcpStream,
    read_timeout: Duration,
}

impl AsyncTcpTransport {
    pub async fn connect(addr: SocketAddr) -> io::Result<Self> {
    // Use a hard-coded 30s connect/read timeout to avoid long hangs
    let connect_timeout = std::time::Duration::from_secs(30);
        // Wrap the connect in a timeout to avoid indefinite hangs during DNS/handshake
        info!(%addr, "async_network::connect: attempting");
        let stream = tokio::time::timeout(connect_timeout, TcpStream::connect(addr))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "connect timeout"))??;
        // Use the same hard-coded 30s for read/write operations
        let read_timeout = std::time::Duration::from_secs(30);
        let peer = stream.peer_addr().ok();
        debug!(%addr, ?peer, "async_network::connect: connected");
        Ok(AsyncTcpTransport {
            stream,
            read_timeout,
        })
    }

    pub async fn send_line(&mut self, line: &str) -> io::Result<()> {
        let mut s = line.as_bytes().to_vec();
        s.push(b'\n');
        debug!(len = s.len(), "async_network::send_line: sending bytes");
        timeout(self.read_timeout, self.stream.write_all(&s))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "write timeout"))??;
        debug!(len = s.len(), "async_network::send_line: sent bytes");
        Ok(())
    }

    pub async fn emergency_stop(&mut self) -> io::Result<()> {
        // Send immediate '!' as GRBL panic/stop if supported
        debug!("async_network::emergency_stop: sending stop sequence");
        timeout(self.read_timeout, self.stream.write_all(b"!\n"))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "es write timeout"))??;
        debug!("async_network::emergency_stop: sent stop sequence");
        Ok(())
    }

    pub async fn flush(&mut self) -> io::Result<()> {
        // no-op for tokio TcpStream; writes are flushed via write_all
        Ok(())
    }

    pub async fn disconnect(&mut self) -> io::Result<()> {
        // Shutdown write half
        let _ = self.stream.shutdown().await;
        Ok(())
    }

    pub async fn is_alive(&self) -> io::Result<bool> {
        // There's no direct is_closed(); try a zero-timeout peek by using poll_read_ready
    Ok(self.stream.peer_addr().is_ok())
    }

    pub async fn read_line(&mut self) -> io::Result<String> {
        // Create a BufReader around a mutable reference to the stream
        let mut reader = BufReader::new(&mut self.stream);
        let mut line = String::new();
        debug!("async_network::read_line: waiting for line");
        timeout(self.read_timeout, reader.read_line(&mut line))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "read timeout"))??;
        // Trim trailing newline and carriage return
        while line.ends_with('\n') || line.ends_with('\r') {
            line.pop();
        }
        debug!(len = line.len(), "async_network::read_line: read line bytes");
        Ok(line)
    }
}
