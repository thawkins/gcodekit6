// The binary uses the library crate's public surface. Rely on the library's
// `ui_impl.rs` include so we don't duplicate generated code into the
// binary crate. Import the public types and bindings from the library.
use gcodekit_ui::{MainWindow, Device};

fn main() {
    // Initialize production-grade tracing for the UI binary. This will
    // respect `GCK_LOG_FILE` for rotating file logs and `RUST_LOG` for level.
    let _ = gcodekit_utils::logging::init_logging_prod();

    #[cfg(all(feature = "with-slint", feature = "slint_generated"))]
    let mut ui = MainWindow::new();
    #[cfg(not(all(feature = "with-slint", feature = "slint_generated")))]
    let ui = MainWindow::new();

    #[cfg(all(feature = "with-slint", feature = "slint_generated"))]
    {
        // Obtain a typed VecModel and set it on the generated UI, then
        // populate it with discovered devices.
        // Build a Vec<UiDevice> and construct a VecModel from it, then set
        // the complete model on the UI in one go.
    let mut items: Vec<crate::Device> = Vec::new();
        if let Ok(devs) = gcodekit_core::device_manager::DeviceManager::discover_devices_detailed() {
            for d in devs.into_iter() {
                items.push(crate::Device {
                    id: d.id,
                    display: d.display,
                    transport: d.transport,
                    product: d.product.unwrap_or_default(),
                    manufacturer: d.manufacturer.unwrap_or_default(),
                    vid: d.vid.map(|v| format!("{:04x}", v)).unwrap_or_default(),
                    pid: d.pid.map(|p| format!("{:04x}", p)).unwrap_or_default(),
                });
            }
        }

        let model = slint::VecModel::from(items);
        let _ = ui.set_deviceModel(model);

        // Attach the connect handler to call our typed endpoint.
        let _ = ui.on_connectRequested(move |wnd: &mut MainWindow, id: String| {
                match gcodekit_ui::slint_bindings::typed::ui_connect_endpoint(&id) {
                Ok(()) => {
                    // On success, refresh the device list model from discovery
                    if let Ok(devs) = gcodekit_core::device_manager::DeviceManager::discover_devices_detailed() {
                        let mut items: Vec<crate::Device> = Vec::new();
                        for d in devs.into_iter() {
                            items.push(crate::Device {
                                id: d.id,
                                display: d.display,
                                transport: d.transport,
                                product: d.product.unwrap_or_default(),
                                manufacturer: d.manufacturer.unwrap_or_default(),
                                vid: d.vid.map(|v| format!("{:04x}", v)).unwrap_or_default(),
                                pid: d.pid.map(|p| format!("{:04x}", p)).unwrap_or_default(),
                            });
                        }
                        let new_model = slint::VecModel::from(items);
                        let _ = wnd.set_deviceModel(new_model);
                    }
                }
                Err(e) => {
                    eprintln!("connect failed: {}", e);
                }
            }
        });
    }

    ui.run();
}
