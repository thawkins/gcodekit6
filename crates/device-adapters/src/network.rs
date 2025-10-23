use std::io::{self, Write};
use std::net::{TcpStream, UdpSocket, ToSocketAddrs};

/// Simple network transport connection enum for tests and stubbing
pub enum NetworkConnection {
    Tcp(TcpStream),
    Udp(UdpSocket, String), // socket + peer addr
}

impl NetworkConnection {
    pub fn connect_tcp<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nonblocking(true)?;
        Ok(NetworkConnection::Tcp(stream))
    }

    pub fn connect_udp<A: ToSocketAddrs>(bind_addr: A, peer: &str) -> io::Result<Self> {
        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(peer)?;
        socket.set_nonblocking(true)?;
        Ok(NetworkConnection::Udp(socket, peer.to_string()))
    }

    pub fn send_line(&mut self, line: &str) -> io::Result<()> {
        match self {
            NetworkConnection::Tcp(s) => {
                s.write_all(line.as_bytes())?;
                s.write_all(b"\n")?;
                Ok(())
            }
            NetworkConnection::Udp(s, _) => {
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
                s.write_all(stop)?;
                Ok(())
            }
            NetworkConnection::Udp(s, _) => {
                s.send(stop)?;
                Ok(())
            }
        }
    }
}
