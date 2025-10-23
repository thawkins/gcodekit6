use crate::models::Device;
use std::io;
use tracing::{debug, info};

/// Save devices list to `devices.json` in the platform data directory.
pub fn save_devices(devices: Vec<Device>) -> io::Result<()> {
    let fname = "devices.json";
    debug!(count = devices.len(), "device::save_devices: saving devices");
    let res = gcodekit_utils::storage::write_json(fname, &devices).map_err(io::Error::other);
    if res.is_ok() {
        info!(count = devices.len(), "device::save_devices: saved devices successfully");
    } else {
        debug!(err = ?res.as_ref().err(), "device::save_devices: failed to save devices");
    }
    res
}

/// Load devices from `devices.json`. Returns empty Vec when file not found.
pub fn load_devices() -> io::Result<Vec<Device>> {
    let fname = "devices.json";
    debug!(fname = %fname, "device::load_devices: loading");
    match gcodekit_utils::storage::read_json::<Vec<Device>>(fname) {
        Ok(v) => {
            info!(count = v.len(), "device::load_devices: loaded devices");
            Ok(v)
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                debug!(fname = %fname, "device::load_devices: file not found, returning empty vec");
                Ok(Vec::new())
            } else {
                debug!(err = ?e, "device::load_devices: error reading devices");
                Err(e)
            }
        }
    }
}
