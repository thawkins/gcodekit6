use std::time::Duration;

/// This test is ignored by default since most CI runners won't have a serial
/// device attached. Run locally with `cargo test -p gcodekit_device_adapters -- --ignored`.
#[test]
#[ignore]
fn integration_serial_transport_basic() {
    // Replace with a real device path on your machine to run the test.
    let device = "/dev/ttyUSB0";
    let baud = 115200;
    let timeout = Duration::from_millis(200);

    if std::path::Path::new(device).exists() {
        let mut conn =
            gcodekit_device_adapters::serial::SerialConnection::open(device, baud, timeout)
                .expect("open serial");
        conn.send_line("M115").expect("send");
        conn.flush().expect("flush");
        assert!(conn.is_alive().unwrap());
        conn.disconnect().expect("disconnect");
    } else {
        eprintln!("serial device {} not present; skipping", device);
    }
}
