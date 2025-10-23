use gcodekit_core::device_manager::DeviceManager;
use std::io::Write;
use std::io::BufRead;
use std::net::TcpListener;
use std::time::Duration;

#[test]
#[ignore]
fn integration_simulated_device_identify_and_persist() {
    // Start a TCP listener that responds to M115 with a simple identifier
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut reader = std::io::BufReader::new(s.try_clone().expect("clone"));
            let mut line = String::new();
            if let Ok(_) = reader.read_line(&mut line) {
                if line.contains("M115") {
                    let _ = s.write_all(b"FIRMWARE_NAME:SimDevice 1.0\n");
                    let _ = s.flush();
                }
            }
            // keep connection open longer to avoid RST race
            std::thread::sleep(Duration::from_millis(1000));
        }
    });

    // Discovery using network helper should find the peer
    let peer = DeviceManager::discover_tcp_peer(&addr.to_string(), Duration::from_millis(2000)).expect("discover peer");
    assert!(peer.contains(&addr.port().to_string()));

    // Connect via DeviceManager and perform identification read
    let mut conn = DeviceManager::connect_network(addr).expect("connect");
    let _ = conn.send_line("M115");
    // give peer a moment to respond
    std::thread::sleep(Duration::from_millis(500));
    let id = conn.read_line().expect("read id");
    assert!(id.contains("FIRMWARE_NAME"));

    // Persisted devices should contain the tcp device
    let devices = gcodekit_core::device::load_devices().expect("load devices");
    assert!(devices.iter().any(|d| d.port.contains(&addr.port().to_string())));
}
