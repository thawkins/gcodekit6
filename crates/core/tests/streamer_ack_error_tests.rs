use gcodekit_core::device_manager::DeviceManager;
use gcodekit_core::streamer::Streamer;
use std::io::{Read, Write};
use std::net::TcpListener;

#[test]
fn test_streamer_error_ack_stops_streaming() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    // Spawn a thread that accepts a connection and responds with an error ACK
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut buf = [0u8; 128];
            // Read one command then respond with error
            match s.read(&mut buf) {
                Ok(n) if n > 0 => {
                    let _ = s.write_all(b"error: bad command\n");
                }
                _ => {}
            }
        }
    });

    let endpoint = format!("127.0.0.1:{}", addr.port());
    let transport = DeviceManager::connect_endpoint(&endpoint).expect("connect");
    let streamer = Streamer::new(transport, 2);

    let lines = vec!["G0 X0", "G0 X1"];
    let res = streamer.stream(lines);
    assert!(
        res.is_err(),
        "expected stream to error on device ACK 'error'"
    );
}
