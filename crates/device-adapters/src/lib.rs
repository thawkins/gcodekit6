//! Device adapters for GCodeKit6 (GRBL, TinyG, Smoothieware)
pub mod network;
pub mod serial;

pub fn hello_adapters() -> &'static str {
    "gcodekit-device-adapters: ready"
}
