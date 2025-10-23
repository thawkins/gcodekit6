use gcodekit_core::streamer::Streamer;
use gcodekit_core::device_manager::DeviceManager;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;
use std::sync::Arc;

#[test]
fn test_streamer_sends_lines_to_tcp() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    // Spawn a thread that accepts and reads lines and responds with an ACK
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut buf = [0u8; 512];
            loop {
                match s.read(&mut buf) {
                    Ok(n) if n > 0 => {
                        let _ = s.write_all(b"ok\n");
                    }
                    _ => break,
                }
            }
        }
    });

    let endpoint = format!("127.0.0.1:{}", addr.port());
    let transport = DeviceManager::connect_endpoint(&endpoint).expect("connect");
    let streamer = Arc::new(Streamer::new(transport, 4));

    let lines = vec!["G0 X0", "G0 X1", "G0 X2"];
    let res = streamer.stream(lines);
    assert!(res.is_ok());
}

#[test]
fn test_streamer_pause_resume_and_emergency_stop() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    // Spawn accept thread that reads slowly and replies with ACKs
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut buf = [0u8; 128];
            loop {
                match s.read(&mut buf) {
                    Ok(n) if n > 0 => {
                        // simulate slow processing
                        std::thread::sleep(Duration::from_millis(10));
                        let _ = s.write_all(b"ok\n");
                    }
                    _ => break,
                }
            }
        }
    });

    let endpoint = format!("127.0.0.1:{}", addr.port());
    let transport = DeviceManager::connect_endpoint(&endpoint).expect("connect");
    let streamer = Arc::new(Streamer::new(transport, 2));

    // Start streaming in a background thread
    let s_clone = Arc::clone(&streamer);
    let handle = std::thread::spawn(move || {
        let lines = (0..10).map(|i| format!("G0 X{}", i));
        let _ = s_clone.stream(lines);
    });

    // Pause, then resume
    std::thread::sleep(Duration::from_millis(10));
    streamer.pause();
    std::thread::sleep(Duration::from_millis(20));
    streamer.resume();

    // Call emergency_stop while streaming
    streamer.emergency_stop().expect("estop");

    let _ = handle.join();
}
