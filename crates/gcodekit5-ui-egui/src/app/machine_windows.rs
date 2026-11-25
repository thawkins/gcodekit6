//! The app's side of the Firmware Settings, Device Info and Device Manager windows: the
//! machine events handed to them, drawing them, the Save / Load file dialogs Firmware
//! Settings asks for ([`crate::windows::FirmwareSettingsWindow::save_to`],
//! [`crate::windows::FirmwareSettingsWindow::load_from`]), and what passes between Firmware
//! Settings and the device profiles (the active profile's controller, a retrieved dump kept
//! in it, Sync from Device), and the active device given to the CAM tools.
//!
//! Dependencies: `gcodekit5-devicedb` (profiles), and the app's dialogs, machine and
//! windows.

use super::GcodeKitApp;
use crate::dialogs::{Asked, DialogAnswer, FilePurpose};
use crate::machine::lock;
use crate::windows::firmware_settings::{EXPORT_FILE_NAME, FileRequest};
use crate::windows::{DeviceInfoWindow, DeviceManagerWindow, FirmwareSettingsWindow};
use gcodekit5_app::machine::MachineEvent;
use gcodekit5_devicedb::{ControllerType, DeviceProfile};

impl GcodeKitApp {
    /// The Firmware Settings window.
    pub fn firmware_settings(&self) -> &FirmwareSettingsWindow {
        &self.firmware_settings
    }

    /// The Firmware Settings window, to feed it events directly (tests).
    pub fn firmware_settings_mut(&mut self) -> &mut FirmwareSettingsWindow {
        &mut self.firmware_settings
    }

    /// The Device Info window.
    pub fn device_info(&self) -> &DeviceInfoWindow {
        &self.device_info
    }

    /// The Device Manager window.
    pub fn device_manager(&self) -> &DeviceManagerWindow {
        &self.device_manager
    }

    /// The active device profile, if one is set.
    pub fn active_device(&self) -> Option<DeviceProfile> {
        self.device_manager.active_profile()
    }

    /// Hands the machine events no observer has seen yet to the windows, open or not, and
    /// keeps a retrieved dump in the profile of the machine it came from.
    pub(super) fn machine_windows_events(&mut self, events: &[MachineEvent]) {
        let tr = &self.state.translator;
        self.firmware_settings.machine_events(events, tr);
        self.device_info.machine_events(events, tr);
        if let Some(dump) = self.firmware_settings.take_dump_for_profile() {
            let connection = lock(self.state.machine.controller())
                .last_connection()
                .cloned();
            let tr = &self.state.translator;
            match self.device_manager.store_dump(&dump, connection.as_ref()) {
                Some(name) => {
                    let title = tr.tr(n_!("Settings stored in device profile:"));
                    self.firmware_settings
                        .note(&format!("{title} {name}"), false);
                }
                None => {
                    let text = n_!(
                        "Settings not stored in a device profile: no profile matches the connected port."
                    );
                    self.firmware_settings.note(&tr.tr(text), true);
                }
            }
            self.sync_active_device();
        }
    }

    /// Gives the CAM tools the active device again once it changed.
    fn sync_active_device(&mut self) {
        if self.device_manager.take_changed() {
            self.cam_tools.borrow_mut().speeds_feeds_mut().device =
                self.device_manager.active_profile();
        }
    }

    /// Draws both windows and shows the file dialog Firmware Settings asked for.
    pub(super) fn show_machine_windows(&mut self, ctx: &egui::Context) {
        let tr = &self.state.translator;
        let profile_repetier = self
            .device_manager
            .active_profile()
            .is_some_and(|p| p.controller_type == ControllerType::Repetier);
        self.firmware_settings.show(
            ctx,
            &self.state.machine,
            self.device_info.identity(),
            profile_repetier,
            tr,
        );
        self.device_info.show(ctx, &self.state.machine, tr);
        self.device_manager
            .show(ctx, &self.state.machine, self.state.settings.config(), tr);
        if self.device_manager.take_sync_request() {
            let reported = self.firmware_settings.reported_values();
            let connected = self.state.machine.status().connected;
            self.device_manager.sync_from(&reported, connected, tr);
        }
        self.sync_active_device();
        match self.firmware_settings.take_file_request() {
            Some(FileRequest::Save) => self.dialogs.save_file_named(
                FilePurpose::ExportFirmwareSettings,
                &FilePurpose::ExportFirmwareSettings.filters(),
                EXPORT_FILE_NAME,
            ),
            Some(FileRequest::Load) => self.dialogs.open_file(
                FilePurpose::ImportFirmwareSettings,
                &FilePurpose::ImportFirmwareSettings.filters(),
            ),
            None => {}
        }
    }

    /// Hands a Firmware Settings file dialog's answer to the window.
    ///
    /// # Returns
    /// Whether the answer was one of its dialogs'.
    pub(super) fn handle_firmware_settings_answer(&mut self, answer: &DialogAnswer) -> bool {
        let tr = &self.state.translator;
        match answer {
            DialogAnswer::File {
                purpose: FilePurpose::ExportFirmwareSettings,
                path,
            } => self.firmware_settings.save_to(path, tr),
            DialogAnswer::File {
                purpose: FilePurpose::ImportFirmwareSettings,
                path,
            } => self.firmware_settings.load_from(path, tr),
            DialogAnswer::Cancelled {
                purpose: FilePurpose::ExportFirmwareSettings | FilePurpose::ImportFirmwareSettings,
            } => {}
            DialogAnswer::Unavailable {
                asked:
                    Asked::File(
                        FilePurpose::ExportFirmwareSettings | FilePurpose::ImportFirmwareSettings,
                    ),
                reason,
            } => tracing::warn!(%reason, "firmware settings file dialog unavailable"),
            _ => return false,
        }
        true
    }
}
