//! The app's side of a CAM page's Load and Save, and of the Gerber page's Browse: the file or
//! folder dialog the pane asks for, and its answer handed back to the pane
//! ([`crate::panes::cam_tools::CamToolsPane::load_preset`],
//! [`crate::panes::cam_tools::CamToolsPane::save_preset`],
//! [`crate::panes::cam_tools::CamToolsPane::choose_gerber_folder`]).
//!
//! Dependencies: none beyond the app's dialogs and the CAM pane.

use super::GcodeKitApp;
use crate::dialogs::{Asked, DialogAnswer, FilePurpose};
use crate::panes::cam_tools::PresetRequest;

impl GcodeKitApp {
    /// Shows the file dialog a CAM page's Load or Save asked for.
    pub(super) fn ask_cam_preset(&mut self) {
        let Some(request) = self.cam_tools.borrow_mut().take_preset_request() else {
            return;
        };
        match request {
            PresetRequest::Load => self.dialogs.open_file(
                FilePurpose::LoadCamPreset,
                &FilePurpose::LoadCamPreset.filters(),
            ),
            PresetRequest::Save { file_name } => self.dialogs.save_file_named(
                FilePurpose::SaveCamPreset,
                &FilePurpose::SaveCamPreset.filters(),
                file_name,
            ),
        }
    }

    /// Shows the folder dialog the Gerber page's Browse asked for.
    pub(super) fn ask_cam_folder(&mut self) {
        if self.cam_tools.borrow_mut().take_folder_request() {
            self.dialogs.open_folder(FilePurpose::OpenGerberFolder);
        }
    }

    /// Hands a CAM page's file or folder dialog answer to the CAM pane.
    ///
    /// # Returns
    /// Whether the answer was a CAM page's.
    pub(super) fn handle_cam_preset_answer(&mut self, answer: &DialogAnswer) -> bool {
        let tr = self.state.translator;
        let mut cam = self.cam_tools.borrow_mut();
        match answer {
            DialogAnswer::File {
                purpose: FilePurpose::LoadCamPreset,
                path,
            } => cam.load_preset(path, &tr),
            DialogAnswer::File {
                purpose: FilePurpose::SaveCamPreset,
                path,
            } => cam.save_preset(path, &tr),
            DialogAnswer::File {
                purpose: FilePurpose::OpenGerberFolder,
                path,
            } => cam.choose_gerber_folder(path),
            DialogAnswer::Cancelled {
                purpose: FilePurpose::LoadCamPreset | FilePurpose::SaveCamPreset,
            } => cam.cancel_preset(),
            // Nothing to forget: the page keeps its folder.
            DialogAnswer::Cancelled {
                purpose: FilePurpose::OpenGerberFolder,
            } => {}
            DialogAnswer::Unavailable {
                asked: Asked::File(FilePurpose::LoadCamPreset | FilePurpose::SaveCamPreset),
                reason,
            } => {
                tracing::warn!(%reason, "CAM preset dialog unavailable");
                cam.cancel_preset();
            }
            DialogAnswer::Unavailable {
                asked: Asked::File(FilePurpose::OpenGerberFolder),
                reason,
            } => tracing::warn!(%reason, "Gerber folder dialog unavailable"),
            DialogAnswer::File { .. }
            | DialogAnswer::Cancelled { .. }
            | DialogAnswer::Unavailable { .. }
            | DialogAnswer::Confirm { .. }
            | DialogAnswer::QuitConfirm { .. } => return false,
        }
        true
    }
}
