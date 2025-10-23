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
        // Use a connect timeout to avoid hanging indefinitely.
        let timeout = Duration::from_secs(30);
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

        Err(last_err.unwrap_or_else(|| io::Error::new(io::ErrorKind::Other, "connect failed")))
    }

    pub fn connect_udp<A: ToSocketAddrs>(bind_addr: A, peer: &str) -> io::Result<Self> {
        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(peer)?;
        // Set blocking mode and timeouts so send/recv honor configured durations.
        socket.set_nonblocking(false)?;
        let timeout = Duration::from_secs(30);
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
}

/// Module-level helper to discover a TCP peer. Kept for ergonomic import in tests.
pub fn discover_tcp_peer(addr: &str, timeout: Duration) -> io::Result<String> {
    NetworkConnection::discover_tcp_peer(addr, timeout)
}
