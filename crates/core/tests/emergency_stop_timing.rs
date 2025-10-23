use std::net::TcpListener;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

/// Simulated streaming device: accepts connections on an already-bound listener
/// and echoes lines until stopped. The listener is non-blocking and the loop
/// checks the provided stop flag and a shutdown receiver for graceful exit.
fn start_simulated_device(listener: TcpListener, stop_flag: Arc<AtomicBool>, shutdown_rx: Receiver<()>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        listener.set_nonblocking(true).ok();
        loop {
            // Check for external shutdown
            if shutdown_rx.try_recv().is_ok() {
                break;
            }

            match listener.accept() {
                Ok((mut s, _peer)) => {
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
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(_) => break,
            }
        }
    })
}

#[test]
fn emergency_stop_timing() {
    // Manual test: measures time from issuing emergency stop to device cease.
    // Bind listener in the test thread so it's ready before client connects.
    let port = std::env::var("GCK_EMERGENCY_PORT").ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0u16);
    let bind_addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&bind_addr).expect("bind");
    let addr = listener.local_addr().unwrap();

    let stop_flag = Arc::new(AtomicBool::new(false));

    // Shutdown channel to stop the server background thread gracefully
    let (tx_shutdown, rx_shutdown) = mpsc::channel();
    let server_handle = start_simulated_device(listener, stop_flag.clone(), rx_shutdown);

    let sock: std::net::SocketAddr = addr;
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

    // Issue emergency stop: send EMERGENCY_STOP via transport so device can detect it
    let start = Instant::now();
    if let Ok(mut guard) = transport.lock() {
        let _ = guard.send_line("EMERGENCY_STOP");
    }

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

    // Signal server to shutdown and join
    let _ = tx_shutdown.send(());
    let _ = server_handle.join();

    match elapsed {
        Some(d) => println!("emergency-stop latency: {:?}", d),
        None => println!("emergency-stop latency: not observed within timeout"),
    }
}
