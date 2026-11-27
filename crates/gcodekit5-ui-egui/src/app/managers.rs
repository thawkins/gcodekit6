//! The manager pages the app owns: Device Config (firmware settings and the Device Info
//! panel), Device Manager, CNC Tools and Materials. [`Managers`] groups their windows, which
//! also hold the device profiles and the tool and material libraries; the app's side of
//! each page (dialogs, machine events, handing libraries to the CAM tools) stays in
//! `machine_windows.rs` and `library_windows.rs`.
//!
//! Dependencies: the app's windows, dialogs and workspace.

use super::{AppState, QuitFlow};
use crate::dialogs::ConfirmRequest;
use crate::windows::{
    DeviceInfoPanel, DeviceManagerWindow, FirmwareSettingsWindow, MaterialsManagerWindow,
    ToolsManagerWindow,
};
use crate::workspace::Page;
use gcodekit5_app::i18n::Translator;

/// The manager pages' windows.
#[derive(Debug)]
pub(super) struct Managers {
    /// The Device Config page's firmware settings.
    pub(super) firmware_settings: FirmwareSettingsWindow,
    /// The Device Info panel (the left column of the Device Config page).
    pub(super) device_info: DeviceInfoPanel,
    /// The Device Manager page, which also holds the device profiles.
    pub(super) device_manager: DeviceManagerWindow,
    /// The CNC Tools page, which also holds the tool library.
    pub(super) tools_manager: ToolsManagerWindow,
    /// The Materials page, which also holds the material library.
    pub(super) materials_manager: MaterialsManagerWindow,
}

impl Managers {
    /// Creates the manager windows, loading the device profiles and the tool and material
    /// libraries from the files `state` names.
    ///
    /// # Arguments
    /// * `state` - The front end's state.
    pub(super) fn new(state: &AppState) -> Self {
        Self {
            firmware_settings: FirmwareSettingsWindow::default(),
            device_info: DeviceInfoPanel::default(),
            device_manager: DeviceManagerWindow::new(state.device_file.clone()),
            tools_manager: ToolsManagerWindow::new(state.tools_file.clone(), state.config()),
            materials_manager: MaterialsManagerWindow::new(state.materials_file.clone()),
        }
    }

    /// Prepares the manager page `page` when it comes into view (it tells why its file
    /// could not be read, CNC Tools reopens the last tool); other pages need nothing.
    ///
    /// # Arguments
    /// * `page` - The page being shown.
    /// * `tr` - The translator.
    pub(super) fn open(&mut self, page: Page, tr: &Translator) {
        match page {
            Page::DeviceManager => self.device_manager.open(tr),
            Page::CncTools => self.tools_manager.open(tr),
            Page::Materials => self.materials_manager.open(tr),
            Page::Designer
            | Page::Visualizer
            | Page::MachineControl
            | Page::CamTools
            | Page::DeviceConfig => {}
        }
    }

    /// The quit prompt for the first manager with unsaved changes `quit` has not discarded
    /// yet, in the order CNC Tools, Materials, Device Manager.
    ///
    /// # Arguments
    /// * `quit` - The quit being decided.
    ///
    /// # Returns
    /// The prompt to ask, or `None` when no manager has unsaved changes left.
    pub(super) fn unsaved_request(&self, quit: &QuitFlow) -> Option<ConfirmRequest> {
        if self.tools_manager.has_unsaved_changes() && !quit.discarded_tool {
            Some(ConfirmRequest::UnsavedTool)
        } else if self.materials_manager.has_unsaved_changes() && !quit.discarded_material {
            Some(ConfirmRequest::UnsavedMaterial)
        } else if self.device_manager.has_unsaved_changes() && !quit.discarded_device {
            Some(ConfirmRequest::UnsavedDevice)
        } else {
            None
        }
    }
}
