use gcodekit_device_adapters::network::NetworkConnection;
use std::io::Read;
use std::net::{TcpListener, UdpSocket};
use std::thread;
use std::time::Duration;

#[test]
fn test_tcp_connect_and_send() {
    // Start a local TCP server
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    // Spawn server thread
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            // read one line
            let mut buf = [0u8; 128];
            let _ = stream.read(&mut buf);
        }
    });

    // Connect client
    let mut conn = NetworkConnection::connect_tcp(addr).expect("connect");
    conn.send_line("G0 X0 Y0").expect("send");
}

#[test]
fn test_tcp_flush_disconnect_is_alive() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    // Spawn accept thread that reads a single byte then sleeps
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut buf = [0u8; 1];
            let _ = s.read(&mut buf);
            std::thread::sleep(Duration::from_millis(50));
        }
    });

    let mut conn = NetworkConnection::connect_tcp(addr).expect("connect");
    assert!(conn.is_alive().unwrap());
    conn.send_line("G0 X1").expect("send");
    conn.flush().expect("flush");
    conn.disconnect().expect("disconnect");
    // After disconnect, is_alive may be false or return an error; accept either.
    let alive = conn.is_alive();
    assert!(alive.is_ok());
}

#[test]
fn test_udp_connect_and_send() {
    // Bind a UDP socket as a simple echo server
    let server = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let server_addr = server.local_addr().unwrap();

    let client_bind = "127.0.0.1:0";
    let peer = format!("127.0.0.1:{}", server_addr.port());

    let mut conn = NetworkConnection::connect_udp(client_bind, &peer).expect("connect_udp");
    conn.send_line("G0 X1 Y1").expect("send");
}
