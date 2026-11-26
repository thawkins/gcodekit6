//! Windows the app opens over its pages (Preferences, About and Help), and the manager pages
//! that were windows in earlier builds (Device Manager, Device Config's Device Info panel and
//! firmware settings, CNC Tools and Materials).
//!
//! Dependencies: `egui`, `craft-ui`, `gcodekit5-settings`, `gcodekit5-app`,
//! `gcodekit5-communication`, `gcodekit5-devicedb`.

pub mod about;
pub mod device_info;
pub mod device_manager;
pub mod firmware_settings;
pub mod help;
pub(crate) mod manager_ui;
pub mod materials_manager;
pub mod setting_write;
pub mod settings;
pub mod tools_manager;

pub use about::AboutWindow;
pub use device_info::DeviceInfoPanel;
pub use device_manager::DeviceManagerWindow;
pub use firmware_settings::FirmwareSettingsWindow;
pub use help::HelpWindow;
pub use materials_manager::MaterialsManagerWindow;
pub use settings::{SettingsOutcome, SettingsWindow};
pub use tools_manager::ToolsManagerWindow;
