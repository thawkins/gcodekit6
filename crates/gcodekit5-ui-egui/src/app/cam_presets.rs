//! The app's side of a CAM page's Load and Save: the file dialog the pane asks for, and its
//! answer handed back to the pane ([`crate::panes::cam_tools::CamToolsPane::load_preset`],
//! [`crate::panes::cam_tools::CamToolsPane::save_preset`]).
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

    /// Hands a preset file dialog's answer to the CAM pane.
    ///
    /// # Returns
    /// Whether the answer was a CAM preset's.
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
            DialogAnswer::Cancelled {
                purpose: FilePurpose::LoadCamPreset | FilePurpose::SaveCamPreset,
            } => cam.cancel_preset(),
            DialogAnswer::Unavailable {
                asked: Asked::File(FilePurpose::LoadCamPreset | FilePurpose::SaveCamPreset),
                reason,
            } => {
                tracing::warn!(%reason, "CAM preset dialog unavailable");
                cam.cancel_preset();
            }
            DialogAnswer::File { .. }
            | DialogAnswer::Cancelled { .. }
            | DialogAnswer::Unavailable { .. }
            | DialogAnswer::Confirm { .. }
            | DialogAnswer::QuitConfirm { .. } => return false,
        }
        true
    }
}
