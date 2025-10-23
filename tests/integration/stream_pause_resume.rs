use gcodekit_device_adapters::network::NetworkConnection;
use std::net::UdpSocket;
use std::thread;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

#[test]
fn integration_stream_pause_resume_udp() {
    // Bind a UDP server to receive packets
    let server = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let server_addr = server.local_addr().unwrap();

    // flag to stop server thread
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    let server_handle = thread::spawn(move || {
        let mut buf = [0u8; 512];
        while r.load(Ordering::SeqCst) {
            if let Ok((n, _)) = server.recv_from(&mut buf) {
                if n == 0 { break; }
            }
        }
    });

    let peer = format!("127.0.0.1:{}", server_addr.port());
    let mut conn = NetworkConnection::connect_udp("127.0.0.1:0", &peer).expect("connect_udp");

    // send some lines
    conn.send_line("G0 X0 Y0").expect("send");
    conn.send_line("G0 X1 Y1").expect("send");

    // simulate pause by sleeping
    std::thread::sleep(std::time::Duration::from_millis(50));

    // resume
    conn.send_line("G0 X2 Y2").expect("send");

    running.store(false, Ordering::SeqCst);
    let _ = server_handle.join();
}
