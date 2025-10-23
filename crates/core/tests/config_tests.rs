use std::fs;
use std::io::Write;
use std::time::Duration;

#[test]
fn test_config_file_overrides_default() {
    // Create a temp dir to act as XDG_DATA_HOME
    let td = tempfile::tempdir().expect("tempdir");
    let data_home = td.path().to_path_buf();
    std::env::set_var("XDG_DATA_HOME", &data_home);

    let cfg_dir = data_home.join("gcodekit6");
    fs::create_dir_all(&cfg_dir).expect("create cfg dir");
    let cfg_path = cfg_dir.join("config.json");

    // Write config with network_timeout_secs = 7
    let mut f = fs::File::create(&cfg_path).expect("create cfg");
    f.write_all(b"{ \"network_timeout_secs\": 7 }")
        .expect("write");
    drop(f);

    // Ensure env var is not set
    std::env::remove_var("GCK_NETWORK_TIMEOUT_SECS");

    let d = gcodekit_core::config::network_timeout();
    assert_eq!(d, Duration::from_secs(7));
}

#[test]
fn test_device_manager_returns_transport() {
    // Start a listener and use DeviceManager to obtain a transport
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((_s, _p)) = listener.accept() {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });

    let mut transport =
        gcodekit_core::device_manager::DeviceManager::connect_network(addr).expect("connect");
    let _ = transport.send_line("M115");
}
