use gcodekit_device_adapters::network::NetworkConnection;
use std::net::SocketAddr;
use std::time::Instant;

#[test]
fn test_connect_timeout_unroutable() {
    // Use a non-routable IP in many networks; connect should fail with timeout or error.
    let addr: SocketAddr = "10.255.255.1:9".parse().unwrap();
    let start = Instant::now();
    let res = NetworkConnection::connect_tcp(addr);
    let elapsed = start.elapsed();
    // We expect an error; elapsed should be approximately 30s (allow 5s margin)
    assert!(res.is_err(), "expected connect to fail or timeout");
    assert!(elapsed.as_secs() >= 25 && elapsed.as_secs() <= 35, "connect timeout not approx 30s: {:?}", elapsed);
}
