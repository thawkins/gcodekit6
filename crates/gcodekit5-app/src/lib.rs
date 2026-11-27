//! # GCodeKit5 App
//!
//! Toolkit-free application state of the GCodeKit5 front end (egui): the machine
//! controller, work zero, device status, console model, command history, the firmware
//! settings and device info models, the device profile form, the tool and material manager
//! forms and back ends, shared parameter types, and the answer to an unsaved-changes prompt.
//!
//! Depends on `gcodekit5-core`, `gcodekit5-communication` and
//! `gcodekit5-settings` (which holds the shared translation tables).
#![deny(unsafe_code)]

/// Marks a string literal for translation extraction without translating it, for msgids
/// translated later through a variable. `scripts/update-po.sh` passes `--keyword=n_!`, as
/// for the front ends' macros of the same name.
macro_rules! n_ {
    ($s:literal) => {
        $s
    };
}

pub mod command_history;
pub mod console;
pub mod device_console_manager;
pub mod device_identity;
pub mod device_profiles;
pub mod device_status;
/// Saving files safely; lives in `gcodekit5-core` so `gcodekit5-devicedb` can use it too.
pub use gcodekit5_core::files;
pub mod firmware_settings;
pub mod i18n;
pub mod machine;
pub mod material_form;
pub mod materials_manager_backend;
pub mod override_target;
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod test_support;
pub mod tool_form;
pub mod tools_manager_backend;
pub mod types;
pub mod unsaved_changes;
pub mod work_zero;

pub use gcodekit5_settings::{tr, trn};
// Controller types and serial-port listing the front ends use, re-exported so a front
// end reaches the communication layer only through the application layer.
pub use gcodekit5_communication::firmware::ControllerProtocol;
pub use gcodekit5_communication::firmware::grbl::settings::SettingsManager;
pub use gcodekit5_communication::firmware::grbl::status_parser::{
    FullStatus, OverrideState, WorkCoordinateOffset, parse_probe_report_in,
};
pub use gcodekit5_communication::{SerialPortInfo, list_ports};
pub use types::{
    BitmapEngravingParams, JigsawPuzzleParams, TabbedBoxParams, VectorEngravingParams,
};
pub use unsaved_changes::UnsavedChangesChoice;
