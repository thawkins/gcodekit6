//! GCodeKit5 Settings Crate
//!
//! Application configuration ([`Config`]), loading and saving it through the
//! craft-settings store ([`SettingsManager`]), and the one-time import of the
//! legacy settings ([`migrate`]).
#![deny(unsafe_code)]

pub mod config;
pub mod manager;
pub mod migrate;

pub use config::{
    Config, ConnectionSettings, ConnectionType, FileProcessingSettings, MachineSettings, Theme,
    UiSettings,
};
pub use manager::SettingsManager;
