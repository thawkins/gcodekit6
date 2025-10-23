use std::io::{Read, Write};
use std::net::{TcpListener, SocketAddr};
use std::thread;
use std::time::Instant;
use gcodekit_device_adapters::create_tcp_transport;

/// Simple in-process TCP echo server used by the performance harness.
fn start_echo_server(listener: TcpListener) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(mut s) => {
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
                Err(_) => break,
            }
        }
    })
}

#[test]
fn perf_transport_latency() {
    // This test is a manual performance harness. It is ignored by default.
    // Bind listener in the test thread to ensure it's ready before connecting
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();
    let _server = start_echo_server(listener);

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

    println!("Perf transport latency (us): n={}, p50={}, p95={}, p99={}", n, p50, p95, p99);

    // Tear down transport. The server runs in background and will be terminated
    // when the test process exits; avoid joining the server thread to prevent
    // blocking on listener.accept.
    let _ = transport.disconnect();
}
