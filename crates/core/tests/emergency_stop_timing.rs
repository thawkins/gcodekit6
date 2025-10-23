use std::net::TcpListener;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::{Duration, Instant};

/// Simulated streaming device: accepts connections and echoes lines until stopped.
fn start_simulated_device(addr: &str, stop_flag: Arc<AtomicBool>) -> thread::JoinHandle<()> {
    let addr = addr.to_string();
    thread::spawn(move || {
        let listener = TcpListener::bind(&addr).expect("bind");
        for stream in listener.incoming() {
            match stream {
                Ok(mut s) => {
                    let stop = stop_flag.clone();
                    thread::spawn(move || {
                        let mut buf = [0u8; 1024];
                        while !stop.load(Ordering::SeqCst) {
                            match s.read(&mut buf) {
                                Ok(0) | Err(_) => break,
                                Ok(n) => {
                                    let slice = &buf[..n];
                                    // If an EMERGENCY_STOP line arrives, set stop flag and break
                                    if slice.windows(14).any(|w| w == b"EMERGENCY_STOP") {
                                        stop.store(true, Ordering::SeqCst);
                                        break;
                                    }
                                    // simulate processing and respond with an ack
                                    let _ = s.write_all(b"ok\n");
                                    let _ = s.flush();
                                }
                            }
                        }
                    });
                }
                Err(_) => break,
            }
        }
    })
}

#[test]
#[ignore]
fn emergency_stop_timing() {
    // Manual test: measures time from issuing emergency stop to device cease.
    let addr = "127.0.0.1:40024";
    let stop_flag = Arc::new(AtomicBool::new(false));
    let server = start_simulated_device(addr, stop_flag.clone());

    let sock: std::net::SocketAddr = addr.parse().expect("parse addr");
    let transport = gcodekit_device_adapters::create_tcp_transport(sock).expect("create_tcp_transport");
    let transport = Arc::new(Mutex::new(transport));

    // Start streaming: use the transport's send_line on a background thread
    let running = Arc::new(AtomicBool::new(true));
    let running2 = running.clone();
    let tx = transport.clone();
    let streamer = thread::spawn(move || {
        let mut i = 0u64;
        while running2.load(Ordering::SeqCst) {
            if let Ok(mut guard) = tx.lock() {
                let _ = guard.send_line(&format!("G1 X{}", i));
            }
            i = i.wrapping_add(1);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    });

    // Let the stream warm up
    thread::sleep(Duration::from_millis(200));

    // Issue emergency stop: set stop flag and record time
    let start = Instant::now();
    // Send an EMERGENCY_STOP command via the transport so the simulated device can detect it
    if let Ok(mut guard) = transport.lock() {
        let _ = guard.send_line("EMERGENCY_STOP");
    }

    // Wait for server-side to stop responding (read returns error or 0)
    // Poll transport->is_alive to detect when the device stops
    let mut elapsed = None;
    for _ in 0..200 {
        match transport.lock() {
            Ok(guard) => match guard.is_alive() {
                Ok(false) => { elapsed = Some(start.elapsed()); break; }
                Ok(true) => { std::thread::sleep(std::time::Duration::from_millis(5)); }
                Err(_) => { elapsed = Some(start.elapsed()); break; }
            },
            Err(_) => { elapsed = Some(start.elapsed()); break; }
        }
    }

    // Stop local streamer thread
    running.store(false, Ordering::SeqCst);
    let _ = streamer.join();

    // best-effort join server
    let _ = server.join();

    match elapsed {
        Some(d) => println!("emergency-stop latency: {:?}", d),
        None => println!("emergency-stop latency: not observed within timeout"),
    }
}
