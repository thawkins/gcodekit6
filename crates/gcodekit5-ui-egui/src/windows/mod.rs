//! Windows the app opens over its workspace: Preferences, About, Firmware Settings and
//! Device Info.
//!
//! Dependencies: `egui`, `craft-ui`, `gcodekit5-settings`, `gcodekit5-app`,
//! `gcodekit5-communication`.

pub mod about;
pub mod device_info;
pub mod firmware_settings;
pub mod setting_write;
pub mod settings;

pub use about::AboutWindow;
pub use device_info::DeviceInfoWindow;
pub use firmware_settings::FirmwareSettingsWindow;
pub use settings::{SettingsOutcome, SettingsWindow};
