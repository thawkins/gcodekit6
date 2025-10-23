//! Core library for GCodeKit6: device communication primitives and safety logic
pub mod device_manager;
pub mod config;

pub fn hello_core() -> &'static str {
    "gcodekit-core: ready"
}
