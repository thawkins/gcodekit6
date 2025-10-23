#[cfg(all(feature = "with-slint", feature = "slint_generated"))]
pub mod typed {
    use crate::Device as UiDevice;

    #[allow(dead_code)]
    pub fn slint_device_model() -> slint::VecModel<UiDevice> {
        slint::VecModel::from(Vec::<UiDevice>::new())
    }

    pub fn ui_connect_endpoint(id: &str) -> Result<(), String> {
        tracing::info!(id = id, "ui_connect_endpoint: attempting connect");
        if id.starts_with("serial:") {
            let path = &id["serial:".len()..];
            match gcodekit_core::device_manager::DeviceManager::connect_endpoint(path) {
                Ok(_t) => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        } else if id.starts_with("tcp:") {
            let addr = &id["tcp:".len()..];
            match addr.parse::<std::net::SocketAddr>() {
                Ok(sock) => match gcodekit_core::device_manager::DeviceManager::connect_network(sock) {
                    Ok(_t) => Ok(()),
                    Err(e) => Err(e.to_string()),
                },
                Err(e) => Err(e.to_string()),
            }
        } else {
            match gcodekit_core::device_manager::DeviceManager::connect_endpoint(id) {
                Ok(_t) => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        }
    }
}

#[cfg(not(all(feature = "with-slint", feature = "slint_generated")))]
pub mod typed {
    #[allow(dead_code)]
    pub fn slint_device_model() -> () { () }

    #[allow(dead_code)]
    pub fn ui_connect_endpoint(_id: &str) -> Result<(), String> {
        Err("ui_connect_endpoint: Slint UI connect handler not available".into())
    }
}
