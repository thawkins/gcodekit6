use gcodekit_core::device_manager::DeviceManager;
use std::net::TcpListener;
use std::fs;
use std::env;

#[test]
fn test_discover_devices_and_connect_network() {
    // Start a local listener to simulate a network device
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((_s, _p)) = listener.accept() {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });

    // Use a temporary XDG_DATA_HOME so persistence writes to an isolated location
    let tmp = tempfile::tempdir().expect("tempdir");
    env::set_var("XDG_DATA_HOME", tmp.path());

    // Discovery should include serial ports (if available) and not error
    let _ = DeviceManager::discover_devices().expect("discover");

    // Try connecting to the listener via connect_network
    let mut transport = DeviceManager::connect_network(addr).expect("connect network");
    // Basic smoke: send a line if transport supports it
    let _ = transport.send_line("M115");

    // devices.json should now exist in the XDG data dir
    let devices_path = tmp.path().join("gcodekit6").join("devices.json");
    assert!(devices_path.exists(), "devices.json should be persisted");
    let raw = fs::read_to_string(&devices_path).expect("read devices");
    assert!(raw.contains("tcp"), "persisted devices should reference tcp");
}
