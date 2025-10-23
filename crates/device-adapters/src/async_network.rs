use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::timeout;

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
        let stream = tokio::time::timeout(connect_timeout, TcpStream::connect(addr))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "connect timeout"))??;

    // Use the same hard-coded 30s for read/write operations
    let read_timeout = std::time::Duration::from_secs(30);
        Ok(AsyncTcpTransport {
            stream,
            read_timeout,
        })
    }

    pub async fn send_line(&mut self, line: &str) -> io::Result<()> {
        let mut s = line.as_bytes().to_vec();
        s.push(b'\n');
        timeout(self.read_timeout, self.stream.write_all(&s))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "write timeout"))??;
        Ok(())
    }

    pub async fn emergency_stop(&mut self) -> io::Result<()> {
        // Send immediate '!' as GRBL panic/stop if supported
        timeout(self.read_timeout, self.stream.write_all(b"!\n"))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "es write timeout"))??;
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
        timeout(self.read_timeout, reader.read_line(&mut line))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "read timeout"))??;
        // Trim trailing newline and carriage return
        while line.ends_with('\n') || line.ends_with('\r') {
            line.pop();
        }
        Ok(line)
    }
}
