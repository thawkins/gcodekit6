//! Windows the app opens over its workspace: Preferences and About.
//!
//! Dependencies: `egui`, `craft-ui`, `gcodekit5-settings`, `gcodekit5-app`.

pub mod about;
pub mod settings;

pub use about::AboutWindow;
pub use settings::{SettingsOutcome, SettingsWindow};
