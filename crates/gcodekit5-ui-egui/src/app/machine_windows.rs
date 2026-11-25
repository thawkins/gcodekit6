//! The app's side of the Device Info window: the machine events handed to it and drawing
//! it.
//!
//! Dependencies: none beyond the app's machine and windows.

use super::GcodeKitApp;
use crate::windows::DeviceInfoWindow;
use gcodekit5_app::machine::MachineEvent;

impl GcodeKitApp {
    /// The Device Info window.
    pub fn device_info(&self) -> &DeviceInfoWindow {
        &self.device_info
    }

    /// Hands the machine events no observer has seen yet to the window, open or not.
    pub(super) fn machine_windows_events(&mut self, events: &[MachineEvent]) {
        let tr = &self.state.translator;
        self.device_info.machine_events(events, tr);
    }

    /// Draws the window.
    pub(super) fn show_machine_windows(&mut self, ctx: &egui::Context) {
        let tr = &self.state.translator;
        self.device_info.show(ctx, &self.state.machine, tr);
    }
}
