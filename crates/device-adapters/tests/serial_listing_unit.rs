use gcodekit_device_adapters::serial;

#[test]
fn test_list_serial_ports_returns_vec() {
    // This is tolerant: it will pass if the function returns an empty Vec.
    // On systems with serial devices present it will validate that entries
    // have at least a path string.
    let ports = serial::list_serial_ports().unwrap_or_default();

    // If there are ports, validate the first entry has a path.
    if !ports.is_empty() {
        let p = &ports[0];
        assert!(!p.path.is_empty(), "serial port path should be non-empty");
        // product/manufacturer may be empty on some platforms; that's okay
    }
}
