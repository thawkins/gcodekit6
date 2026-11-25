//! The app's side of the Firmware Settings and Device Info windows: the machine events
//! handed to them, drawing them, and the Save / Load file dialogs Firmware Settings asks for
//! ([`crate::windows::FirmwareSettingsWindow::save_to`],
//! [`crate::windows::FirmwareSettingsWindow::load_from`]).
//!
//! Dependencies: none beyond the app's dialogs, machine and windows.

use super::GcodeKitApp;
use crate::dialogs::{Asked, DialogAnswer, FilePurpose};
use crate::windows::firmware_settings::{EXPORT_FILE_NAME, FileRequest};
use crate::windows::{DeviceInfoWindow, FirmwareSettingsWindow};
use gcodekit5_app::machine::MachineEvent;

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

    /// Hands the machine events no observer has seen yet to both windows, open or not.
    pub(super) fn machine_windows_events(&mut self, events: &[MachineEvent]) {
        let tr = &self.state.translator;
        self.firmware_settings.machine_events(events, tr);
        self.device_info.machine_events(events, tr);
    }

    /// Draws both windows and shows the file dialog Firmware Settings asked for.
    pub(super) fn show_machine_windows(&mut self, ctx: &egui::Context) {
        let tr = &self.state.translator;
        self.firmware_settings
            .show(ctx, &self.state.machine, self.device_info.identity(), tr);
        self.device_info.show(ctx, &self.state.machine, tr);
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
