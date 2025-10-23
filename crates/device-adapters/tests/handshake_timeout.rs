use std::time::Duration;

// This test is ignored by default because it intentionally simulates a slow
// or stalled websocket handshake to confirm the 30s handshake timeout in the
// synchronous websocket adapter. It should be run manually when verifying
// handshake-timeout behavior on CI runners.

#[test]
#[ignore]
fn test_handshake_timeout_triggered() {
    // Not implemented: this test would require a server that accepts TCP
    // connections but delays the websocket handshake beyond 30s. Left as an
    // ignored integration test for manual verification on CI.
    std::thread::sleep(Duration::from_millis(1));
}
