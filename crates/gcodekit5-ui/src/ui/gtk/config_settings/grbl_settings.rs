//! # GRBL Settings Panel
//!
//! Loads the standard GRBL setting definitions ($0-$680, from
//! `gcodekit5_app::firmware_settings`) into the Device Config list.

use gcodekit5_app::firmware_settings::{grbl_definitions, load_definitions};

use super::ConfigSettingsView;

impl ConfigSettingsView {
    pub(crate) fn load_default_grbl_settings(&self) {
        load_definitions(&mut self.settings_manager.borrow_mut(), grbl_definitions());
    }
}
