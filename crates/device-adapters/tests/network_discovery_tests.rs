use gcodekit_device_adapters::network::discover_tcp_peer;
use std::net::TcpListener;
use std::time::Duration;

#[test]
#[ignore]
fn integration_network_discovery_detects_local_listener() {
    // Start a listener on an OS-assigned port
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");

    // Keep the listener alive while discovery runs
    std::thread::spawn(move || {
        if let Ok((_socket, _peer)) = listener.accept() {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    let target = format!("127.0.0.1:{}", addr.port());
    let res = discover_tcp_peer(&target, Duration::from_millis(200));
    assert!(res.is_ok(), "discover_tcp_peer should find the local listener");
    assert_eq!(res.unwrap(), target);
}
