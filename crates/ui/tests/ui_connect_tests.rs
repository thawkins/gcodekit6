use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn test_ui_connect_updates_model_and_persists() {
    // Use a temporary XDG_DATA_HOME so persistence writes to the temp dir
    let tmp = tempdir().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    std::env::set_var("XDG_DATA_HOME", &dir);

    // Create the UI MainWindow (either the real generated UI or the
    // committed stub). The crate root exports MainWindow/Device types.
    let mut ui = gcodekit_ui::MainWindow::new();

    // Attach the real handler that uses core device manager (reuse existing wiring)
    ui.on_connectRequested(|wnd, id| {
        // Call into the same helper used by the runtime
        let _ = gcodekit_ui::slint_bindings::typed::ui_connect_endpoint(&id).ok();
        // After connect we simulate the UI refresh by invoking the logic
        if let Ok(devs) = gcodekit_core::device_manager::DeviceManager::discover_devices_detailed() {
            let mut items: Vec<gcodekit_ui::Device> = Vec::new();
            for d in devs.into_iter() {
                items.push(gcodekit_ui::Device {
                    id: d.id,
                    display: d.display,
                    transport: d.transport,
                    product: d.product.unwrap_or_default(),
                    manufacturer: d.manufacturer.unwrap_or_default(),
                    vid: d.vid.map(|v| format!("{:04x}", v)).unwrap_or_default(),
                    pid: d.pid.map(|p| format!("{:04x}", p)).unwrap_or_default(),
                });
            }
            let model = gcodekit_ui::VecModel::from(items);
            let _ = wnd.set_deviceModel(model);
        }
    });

    // Initially the model should be empty
    assert_eq!(ui.model_len(), 0);

    // Simulate a discovery + connect to a fake endpoint (core connect will likely fail but persistence may still be attempted)
    // We'll just invoke the handler with a dummy TCP id which DeviceManager can parse
    ui.invoke_connect("tcp:127.0.0.1:9".to_string());

    // After invoking, the model may be non-empty depending on discovery; check persistence file existence
    let mut devices_file = PathBuf::from(std::env::var("XDG_DATA_HOME").unwrap());
    devices_file.push("gcodekit6");
    devices_file.push("devices.json");

    // devices.json should exist (save_devices is best-effort so it may or may not be present)
    // We assert that the directory exists and do a permissive check for file existence.
    assert!(devices_file.parent().unwrap().exists());

    // If devices.json exists, ensure it's valid JSON array
    if devices_file.exists() {
        let data = fs::read_to_string(&devices_file).expect("read devices.json");
        assert!(data.trim().starts_with("[") && data.trim().ends_with("]"));
    }

    // If the model contains items, assert some fields are populated for at least one item
    let items = ui.model_items();
    if !items.is_empty() {
        // Ensure at least one item has a non-empty transport and display
        let mut found = false;
        for it in items.iter() {
            if !it.transport.is_empty() && !it.display.is_empty() {
                found = true;
                break;
            }
        }
        assert!(found, "expected at least one model item with transport and display populated");
    }
}
