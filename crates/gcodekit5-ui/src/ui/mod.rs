//! User interface module - GTK-based
//!
//! This module contains the GTK implementation of the UI.

pub mod gtk;

pub use gcodekit5_app::console as console_panel;
pub use gcodekit5_app::{device_console_manager, materials_manager_backend, tools_manager_backend};
