//! # Repetier Settings Panel
//!
//! Loads the standard Repetier EEPROM definitions (from
//! `gcodekit5_app::firmware_settings`) into the Device Config list.

use gcodekit5_app::firmware_settings::{load_definitions, repetier_definitions};

use super::ConfigSettingsView;

impl ConfigSettingsView {
    pub(crate) fn load_default_repetier_settings(&self) {
        load_definitions(
            &mut self.settings_manager.borrow_mut(),
            repetier_definitions(),
        );
    }
}
