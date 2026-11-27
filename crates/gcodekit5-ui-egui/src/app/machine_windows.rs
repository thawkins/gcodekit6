//! The app's side of the Device Config (firmware settings) and Device Manager pages and the
//! Device Info panel: the machine events handed to them, drawing them, the Save / Load file
//! dialogs Firmware Settings asks for ([`crate::windows::FirmwareSettingsWindow::save_to`],
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
use crate::windows::{DeviceInfoPanel, DeviceManagerWindow, FirmwareSettingsWindow};
use crate::workspace::Page;
use gcodekit5_app::UnsavedChangesChoice;
use gcodekit5_app::machine::MachineEvent;
use gcodekit5_devicedb::DeviceProfile;

/// Width of the Device Config page's Device Info panel (320 px).
const DEVICE_INFO_WIDTH: f32 = 320.0;

impl GcodeKitApp {
    /// The Device Config page's firmware settings.
    pub fn firmware_settings(&self) -> &FirmwareSettingsWindow {
        &self.managers.firmware_settings
    }

    /// The Device Config page's firmware settings, to feed them events directly (tests).
    pub fn firmware_settings_mut(&mut self) -> &mut FirmwareSettingsWindow {
        &mut self.managers.firmware_settings
    }

    /// The Device Info panel (the left column of the Device Config page).
    pub fn device_info(&self) -> &DeviceInfoPanel {
        &self.managers.device_info
    }

    /// The Device Manager page.
    pub fn device_manager(&self) -> &DeviceManagerWindow {
        &self.managers.device_manager
    }

    /// The active device profile, if one is set.
    pub fn active_device(&self) -> Option<DeviceProfile> {
        self.managers.device_manager.active_profile()
    }

    /// Hands the machine events no observer has seen yet to the pages and windows, shown or
    /// not, and
    /// keeps a retrieved dump in the profile of the machine it came from.
    pub(super) fn machine_windows_events(&mut self, events: &[MachineEvent]) {
        let tr = &self.state.translator;
        self.managers.firmware_settings.machine_events(events, tr);
        self.managers.device_info.machine_events(events, tr);
        if let Some(dump) = self.managers.firmware_settings.take_dump_for_profile() {
            let connection = lock(self.state.machine.controller())
                .last_connection()
                .cloned();
            let tr = &self.state.translator;
            match self
                .managers
                .device_manager
                .store_dump(&dump, connection.as_ref())
            {
                Some(name) => {
                    let title = tr.tr(n_!("Settings stored in device profile:"));
                    self.managers
                        .firmware_settings
                        .note(&format!("{title} {name}"), false);
                }
                None => {
                    let text = n_!(
                        "Settings not stored in a device profile: no profile matches the connected port."
                    );
                    self.managers.firmware_settings.note(&tr.tr(text), true);
                }
            }
            self.sync_active_device();
        }
    }

    /// Acts on the quit flow's question about an unsaved device profile: Save saves it and
    /// goes on quitting (a refused save stops the quit and shows the page with the reason),
    /// Discard goes on without it, Cancel stops the quit and shows the page.
    pub(super) fn device_quit_answer(&mut self, choice: UnsavedChangesChoice) {
        let tr = &self.state.translator;
        match choice {
            UnsavedChangesChoice::Save => {
                let job_running = self.state.job_running();
                let saved = self.managers.device_manager.save_for_quit(job_running, tr);
                self.sync_active_device();
                if saved {
                    self.continue_quit();
                } else {
                    self.show_page(Page::DeviceManager);
                    self.cancel_quit();
                }
            }
            UnsavedChangesChoice::Discard => {
                self.flows.quit.discarded_device = true;
                self.continue_quit();
            }
            UnsavedChangesChoice::Cancel => {
                self.show_page(Page::DeviceManager);
                self.cancel_quit();
            }
        }
    }

    /// Gives the CAM tools the active device again once it changed.
    fn sync_active_device(&mut self) {
        if self.managers.device_manager.take_changed() {
            let profile = self.managers.device_manager.active_profile();
            self.workspace.set_device(profile.as_ref());
            self.cam_tools.borrow_mut().speeds_feeds_mut().device = profile;
        }
    }

    /// The active device profile's controller type, if a profile is active.
    fn profile_controller(&self) -> Option<gcodekit5_devicedb::ControllerType> {
        self.managers
            .device_manager
            .active_profile()
            .map(|p| p.controller_type)
    }

    /// Runs the firmware settings' reads and writes; called once every frame, before the
    /// page is drawn, whatever page is shown.
    pub(super) fn update_firmware_settings(&mut self, ctx: &egui::Context) {
        let controller = self.profile_controller();
        self.managers.firmware_settings.update(
            ctx,
            self.managers.device_info.identity(),
            controller,
            &self.state.translator,
        );
    }

    /// Draws the Device Config page into `ui`: the Device Info panel
    /// on the left, the firmware settings on the right.
    pub(super) fn show_firmware_settings(&mut self, ui: &mut egui::Ui) {
        let controller = self.profile_controller();
        let tr = &self.state.translator;
        ui.horizontal_top(|ui| {
            let size = egui::vec2(DEVICE_INFO_WIDTH, ui.available_height());
            ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
                self.managers.device_info.show(ui, &self.state.machine, tr);
            });
            ui.separator();
            ui.add_space(6.0);
            ui.vertical(|ui| {
                self.managers.firmware_settings.show(
                    ui,
                    &self.state.machine,
                    self.managers.device_info.identity(),
                    controller,
                    tr,
                );
            });
        });
    }

    /// Runs the Device Info panel's writes, takes Sync from Device and shows the file dialog
    /// Firmware Settings asked for.
    pub(super) fn show_machine_windows(&mut self, ctx: &egui::Context) {
        let tr = &self.state.translator;
        self.managers.device_info.update(ctx, tr);
        if self.managers.device_manager.take_sync_request() {
            let reported = self.managers.firmware_settings.reported_values();
            let connected = self.state.machine.status().connected;
            self.managers
                .device_manager
                .sync_from(&reported, connected, tr);
        }
        self.sync_active_device();
        match self.managers.firmware_settings.take_file_request() {
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
            } => self.managers.firmware_settings.save_to(path, tr),
            DialogAnswer::File {
                purpose: FilePurpose::ImportFirmwareSettings,
                path,
            } => self.managers.firmware_settings.load_from(path, tr),
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
