use gcodekit_device_adapters::network::NetworkConnection;
use std::net::TcpListener;
use std::thread;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::io::Read;

#[test]
#[ignore]
fn integration_emergency_stop_stops_streaming() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    let server_handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 256];
            while r.load(Ordering::SeqCst) {
                let _ = stream.read(&mut buf);
            }
        }
    });

    let mut conn = NetworkConnection::connect_tcp(addr).expect("connect");

    conn.send_line("G0 X0 Y0").expect("send");
    conn.send_line("G0 X1 Y1").expect("send");

    conn.emergency_stop().expect("estop");

    let res = conn.send_line("G0 X2 Y2");
    let _ = res;

    running.store(false, Ordering::SeqCst);
    let _ = server_handle.join();
}
