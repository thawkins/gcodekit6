use std::net::TcpListener;
use std::time::Duration;

// This test intentionally simulates a server that accepts the TCP connection
// but delays completing the websocket handshake. The client-side connect is
// expected to fail with a timeout (30s). The test is marked `#[ignore]`
// because it sleeps for longer than the configured timeout and would slow
// default test runs.

#[test]
#[ignore]
fn test_handshake_times_out_after_30s() {
    // Bind a local TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    // Server thread: accept, then sleep longer than the client's 30s timeout
    let server = std::thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            // Sleep 35s to exceed the client's 30s handshake timeout
            std::thread::sleep(Duration::from_secs(35));
            // After sleeping, attempt to perform the websocket accept to clean up
            let _ = tungstenite::accept(stream);
        }
    });

    // Client attempt: connect to the server using the sync WebSocketTransport
    let url = format!("ws://127.0.0.1:{}", addr.port());

    // Use the public API to attempt a connect; expecting an error due to timeout
    match crate::WebSocketTransport::connect(&url) {
        Err(e) => {
            // Expect a timeout error kind
            assert!(matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock));
        }
        Ok(_) => panic!("handshake unexpectedly succeeded"),
    }

    let _ = server.join();
}
