use gcodekit_utils::settings::network_timeout;
use std::io::BufRead;
use std::io::BufReader;
use std::io::{self, Write};
use std::net::{TcpStream, ToSocketAddrs, UdpSocket};
use std::time::Duration;

/// Simple network transport connection enum for tests and stubbing
pub enum NetworkConnection {
    Tcp(TcpStream),
    Udp(UdpSocket, String), // socket + peer addr
}

impl NetworkConnection {
    pub fn connect_tcp<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        // Use the configured network timeout (defaults to 30s)
        let timeout = network_timeout();
        let mut last_err = None;
        for sock in addr.to_socket_addrs()? {
            match TcpStream::connect_timeout(&sock, timeout) {
                Ok(stream) => {
                    // Ensure reasonable blocking behavior with timeouts.
                    stream.set_nonblocking(false)?;
                    stream.set_read_timeout(Some(timeout))?;
                    stream.set_write_timeout(Some(timeout))?;
                    return Ok(NetworkConnection::Tcp(stream));
                }
                Err(e) => last_err = Some(e),
            }
        }

    Err(last_err.unwrap_or_else(|| io::Error::other("connect failed")))
    }

    pub fn connect_udp<A: ToSocketAddrs>(bind_addr: A, peer: &str) -> io::Result<Self> {
        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(peer)?;
        // Set blocking mode and timeouts so send/recv honor configured durations.
        socket.set_nonblocking(false)?;
        let timeout = network_timeout();
        socket.set_read_timeout(Some(timeout))?;
        socket.set_write_timeout(Some(timeout))?;
        Ok(NetworkConnection::Udp(socket, peer.to_string()))
    }

    pub fn send_line(&mut self, line: &str) -> io::Result<()> {
        match self {
            NetworkConnection::Tcp(s) => {
                // write_all will block but respect the socket write timeout set at connect time
                s.write_all(line.as_bytes())?;
                s.write_all(b"\n")?;
                Ok(())
            }
            NetworkConnection::Udp(s, _) => {
                // send will respect the socket write timeout (where supported)
                s.send(line.as_bytes())?;
                Ok(())
            }
        }
    }

    pub fn emergency_stop(&mut self) -> io::Result<()> {
        // For stubs: send a standard ascii kill sequence or a single 0x18 (CAN)
        let stop = b"!"; // placeholder
        match self {
            NetworkConnection::Tcp(s) => {
                // Respect write timeout
                s.write_all(stop)?;
                Ok(())
            }
            NetworkConnection::Udp(s, _) => {
                s.send(stop)?;
                Ok(())
            }
        }
    }

    /// Flush any buffered output. For TCP this forwards to the underlying
    /// stream's flush implementation. For UDP this is a no-op.
    pub fn flush(&mut self) -> io::Result<()> {
        use std::io::Write;
        match self {
            NetworkConnection::Tcp(s) => s.flush(),
            NetworkConnection::Udp(_, _) => Ok(()),
        }
    }

    /// Attempt to gracefully disconnect the transport. For TCP we shutdown the
    /// socket; for UDP this is a no-op (socket will be closed when dropped).
    pub fn disconnect(&mut self) -> io::Result<()> {
        use std::net::Shutdown;
        match self {
            NetworkConnection::Tcp(s) => s.shutdown(Shutdown::Both),
            NetworkConnection::Udp(_, _) => Ok(()),
        }
    }

    /// Lightweight liveness check. For TCP this inspects any pending socket
    /// error; for UDP we assume the socket is alive if it exists.
    pub fn is_alive(&self) -> io::Result<bool> {
        match self {
            NetworkConnection::Tcp(s) => {
                // take_error returns any pending socket error; None means no
                // error observed.
                match s.take_error() {
                    Ok(None) => Ok(true),
                    Ok(Some(_)) => Ok(false),
                    Err(e) => Err(e),
                }
            }
            NetworkConnection::Udp(_, _) => Ok(true),
        }
    }

    /// Attempt to connect to a TCP peer at `addr` with a short timeout to
    /// determine reachability. Returns the peer string on success.
    pub fn discover_tcp_peer(addr: &str, timeout: Duration) -> io::Result<String> {
        // Resolve address to a SocketAddr and attempt a connect with timeout.
        let mut addrs = addr.to_socket_addrs()?;
        if let Some(sock) = addrs.next() {
            match TcpStream::connect_timeout(&sock, timeout) {
                Ok(_) => Ok(sock.to_string()),
                Err(e) => Err(e),
            }
        } else {
            Err(io::Error::new(io::ErrorKind::InvalidInput, "no addrs"))
        }
    }

    /// Read a line (up to and including a newline) from the transport and
    /// return it without the trailing newline.
    pub fn read_line(&mut self) -> io::Result<String> {
        match self {
            NetworkConnection::Tcp(s) => {
                // Create a temporary BufReader borrowing the stream. To avoid
                // taking ownership we duplicate the stream handle using try_clone.
                let mut reader = BufReader::new(s.try_clone()?);
                let mut line = String::new();
                reader.read_line(&mut line)?;
                if line.ends_with('\n') {
                    line.truncate(line.len() - 1);
                }
                Ok(line)
            }
            NetworkConnection::Udp(s, _) => {
                let mut buf = [0u8; 1500];
                let n = s.recv(&mut buf)?;
                let mut s = String::from_utf8_lossy(&buf[..n]).to_string();
                if s.ends_with('\n') {
                    s.truncate(s.len() - 1);
                }
                Ok(s)
            }
        }
    }
}

/// Module-level helper to discover a TCP peer. Kept for ergonomic import in tests.
pub fn discover_tcp_peer(addr: &str, timeout: Duration) -> io::Result<String> {
    NetworkConnection::discover_tcp_peer(addr, timeout)
}
