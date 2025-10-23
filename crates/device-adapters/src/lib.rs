//! Device adapters for GCodeKit6 (GRBL, TinyG, Smoothieware)
pub mod network;

pub fn hello_adapters() -> &'static str {
    "gcodekit-device-adapters: ready"
}
