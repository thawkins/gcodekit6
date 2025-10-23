use gcodekit_device_adapters::serial::list_serial_ports;

#[test]
fn test_list_serial_ports_runs() {
    // This test simply ensures the call completes without panicking.
    let res = list_serial_ports();
    assert!(res.is_ok());
    // We can't assert presence of ports in CI; just ensure it's a Vec.
    let _ports = res.unwrap();
}
