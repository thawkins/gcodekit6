//! Core library for GCodeKit6: device communication primitives and safety logic
pub mod device_manager;
pub mod config;
pub mod persistence;
pub mod streamer;
pub mod async_streamer;
pub mod models;
pub mod error;
pub mod gcode;
pub mod job;
pub mod streamer_worker;

pub fn hello_core() -> &'static str {
    "gcodekit-core: ready"
}
