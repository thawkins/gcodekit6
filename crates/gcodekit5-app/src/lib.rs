//! # GCodeKit5 App
//!
//! Toolkit-free application state shared by the GCodeKit5 front ends (GTK and
//! egui): device status, console model, command history, tool and material
//! manager back ends, shared parameter types, and the answer to an unsaved-changes prompt.
//!
//! Depends on `gcodekit5-core`, `gcodekit5-communication` and
//! `gcodekit5-settings` (which holds the shared translation tables).
#![deny(unsafe_code)]

pub mod command_history;
pub mod console;
pub mod device_console_manager;
pub mod device_status;
pub mod i18n;
pub mod machine;
pub mod materials_manager_backend;
pub mod tools_manager_backend;
pub mod types;
pub mod unsaved_changes;

pub use gcodekit5_settings::{tr, trn};
pub use types::{
    BitmapEngravingParams, JigsawPuzzleParams, TabbedBoxParams, VectorEngravingParams,
};
pub use unsaved_changes::UnsavedChangesChoice;
