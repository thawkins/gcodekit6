//! # GCodeKit6 App
//!
//! Toolkit-free application state of the GCodeKit6 front end (egui): the machine
//! controller, work zero, console model, command history, the firmware
//! settings and device info models, the device profile form, the tool and material manager
//! forms and back ends, shared parameter types, and the answer to an unsaved-changes prompt.
//!
//! Depends on `gcodekit6-core`, `gcodekit6-communication` and
//! `gcodekit6-settings`; owns the embedded translation tables ([`i18n`]).
//!
//! Rule: no process-global device state. Machine state lives in the instance that owns the
//! connection (`MachineController` here, `Machine` in the egui front end); do not add a
//! `static` `RwLock`/`Mutex`/`Atomic`/`LazyLock`/`OnceLock` holding device status.
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
pub mod device_identity;
pub mod device_profiles;
/// Saving files safely; lives in `gcodekit6-core` so `gcodekit6-devicedb` can use it too.
pub use gcodekit6_core::files;
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

// Controller types and serial-port listing the front ends use, re-exported so a front
// end reaches the communication layer only through the application layer.
pub use gcodekit6_communication::firmware::ControllerProtocol;
pub use gcodekit6_communication::firmware::grbl::error_decoder::{decode_alarm, decode_error};
pub use gcodekit6_communication::firmware::grbl::settings::SettingsManager;
pub use gcodekit6_communication::firmware::grbl::status_parser::{
    FullStatus, OverrideState, WorkCoordinateOffset, parse_probe_report_in,
};
pub use gcodekit6_communication::{SerialPortInfo, list_ports};
pub use types::{
    BitmapEngravingParams, JigsawPuzzleParams, TabbedBoxParams, VectorEngravingParams,
};
pub use unsaved_changes::UnsavedChangesChoice;
