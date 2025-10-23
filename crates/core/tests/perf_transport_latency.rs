use std::io::{Read, Write};
use std::net::{TcpListener, SocketAddr};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Instant;
use gcodekit_device_adapters::create_tcp_transport;

/// Simple in-process TCP echo server used by the performance harness. The
/// listener is non-blocking and the loop checks for a shutdown signal so the
/// test can cleanly stop the server.
fn start_echo_server(listener: TcpListener, shutdown_rx: Receiver<()>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        listener.set_nonblocking(true).ok();
        loop {
            if shutdown_rx.try_recv().is_ok() {
                break;
            }
            match listener.accept() {
                Ok((mut s, _)) => {
                    thread::spawn(move || {
                        let mut buf = [0u8; 1024];
                        loop {
                            match s.read(&mut buf) {
                                Ok(0) | Err(_) => break,
                                Ok(n) => {
                                    let _ = s.write_all(&buf[..n]);
                                }
                            }
                        }
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(std::time::Duration::from_millis(5));
                    continue;
                }
                Err(_) => break,
            }
        }
    })
}

#[test]
fn perf_transport_latency() {
    // This test is a manual performance harness. It is ignored by default.
    // Initialize tracing for harness runs so logs include timestamps/levels
    let _ = gcodekit_utils::logging::init_logging();
    // Allow overriding the port for reproducible runs; default to ephemeral (0)
    let port = std::env::var("GCK_PERF_PORT").ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0u16);
    let bind_addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&bind_addr).expect("bind");
    let addr = listener.local_addr().unwrap();

    // Shutdown channel to stop the echo server cleanly
    let (tx_shutdown, rx_shutdown) = mpsc::channel();
    let server_handle = start_echo_server(listener, rx_shutdown);

    // Connect via the project's TCP transport factory so we measure the same code paths
    let sock: SocketAddr = addr;
    let mut transport = create_tcp_transport(sock).expect("create_tcp_transport");

    // Warmup
    for _ in 0..10 {
        transport.send_line("warmup").unwrap();
        let _ = transport.read_line();
    }

    // Measure N round-trips
    let n = 1000usize;
    let mut latencies = Vec::with_capacity(n);
    for i in 0..n {
        let payload = format!("ping {}", i);
        let start = Instant::now();
        transport.send_line(&payload).unwrap();
        let _ = transport.read_line().unwrap();
        let dur = start.elapsed();
        latencies.push(dur.as_micros() as u64);
    }

    latencies.sort_unstable();
    let p50 = latencies[latencies.len() * 50 / 100];
    let p95 = latencies[latencies.len() * 95 / 100];
    let p99 = latencies[latencies.len() * 99 / 100];

    tracing::info!(n = n, p50 = p50, p95 = p95, p99 = p99, "Perf transport latency (us)");

    // Tear down transport and stop server
    let _ = transport.disconnect();
    let _ = tx_shutdown.send(());
    let _ = server_handle.join();
}
