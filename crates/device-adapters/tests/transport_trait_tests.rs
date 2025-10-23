use gcodekit_device_adapters::network::NetworkConnection;
use std::net::TcpListener;

#[test]
fn test_transport_trait_with_network_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    std::thread::spawn(move || {
        if let Ok((_s, _p)) = listener.accept() {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });

    let mut conn = NetworkConnection::connect_tcp(addr).expect("connect");
    let res = conn.send_line("M115");
    assert!(res.is_ok());
}
