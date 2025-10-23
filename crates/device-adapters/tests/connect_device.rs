use gcodekit_device_adapters::network::NetworkConnection;
use std::net::TcpListener;
use std::thread;
use std::io::Read;

#[test]
fn integration_connect_to_simulated_tcp_device() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    let server_handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 256];
            let _ = stream.read(&mut buf);
        }
    });

    let mut conn = NetworkConnection::connect_tcp(addr).expect("connect");
    conn.send_line("M115").expect("send firmware query");

    let _ = server_handle.join();
}
