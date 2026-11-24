//! # GCodeKit5 App
//!
//! Toolkit-free application state shared by the GCodeKit5 front ends (GTK and
//! egui): device status, console model, command history, tool and material
//! manager back ends, and shared parameter types.
//!
//! Depends on `gcodekit5-core` and `gcodekit5-communication`.
#![deny(unsafe_code)]

/// Translates a string through gettext (same catalog as the GTK front end).
macro_rules! t {
    ($s:expr) => {
        gettextrs::gettext($s)
    };
}

pub mod command_history;
pub mod console;
pub mod device_console_manager;
pub mod device_status;
pub mod machine;
pub mod materials_manager_backend;
pub mod tools_manager_backend;
pub mod types;

pub use types::{
    BitmapEngravingParams, JigsawPuzzleParams, TabbedBoxParams, VectorEngravingParams,
};
