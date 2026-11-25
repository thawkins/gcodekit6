//! The app's side of the CNC Tools and Materials windows: drawing them, the GTC import and
//! export file dialogs CNC Tools asks for, handing a changed library to the speeds and
//! feeds calculator at once, and keeping CNC Tools' search and filters in the settings when
//! it closes (GTK remembered them).
//!
//! Dependencies: the app's dialogs, settings and windows.

use super::GcodeKitApp;
use crate::dialogs::{Asked, DialogAnswer, FilePurpose};
use crate::windows::ToolsManagerWindow;
use crate::windows::tools_manager::{GtcFile, ToolsFileRequest};

impl GcodeKitApp {
    /// The CNC Tools window.
    pub fn tools_manager(&self) -> &ToolsManagerWindow {
        &self.tools_manager
    }

    /// The CNC Tools window, to drive it directly (tests).
    pub fn tools_manager_mut(&mut self) -> &mut ToolsManagerWindow {
        &mut self.tools_manager
    }

    /// Draws the library windows, shows the file dialog CNC Tools asked for, and hands a
    /// changed library to the speeds and feeds calculator.
    pub(super) fn show_library_windows(&mut self, ctx: &egui::Context) {
        let tr = &self.state.translator;
        let tools_open = self.tools_manager.is_open();
        self.tools_manager.show(ctx, tr);
        if tools_open && !self.tools_manager.is_open() {
            self.store_tools_ui_state();
        }
        match self.tools_manager.take_file_request() {
            Some(ToolsFileRequest::Import(GtcFile::Package)) => self.dialogs.open_file(
                FilePurpose::ImportGtcPackage,
                &FilePurpose::ImportGtcPackage.filters(),
            ),
            Some(ToolsFileRequest::Import(GtcFile::Json)) => self.dialogs.open_file(
                FilePurpose::ImportGtcJson,
                &FilePurpose::ImportGtcJson.filters(),
            ),
            Some(ToolsFileRequest::Export) => self.dialogs.save_file_named(
                FilePurpose::ExportCustomTools,
                &FilePurpose::ExportCustomTools.filters(),
                "custom_tools.json",
            ),
            None => {}
        }
        self.sync_libraries();
    }

    /// Gives the speeds and feeds calculator the libraries again once one changed.
    fn sync_libraries(&mut self) {
        if self.tools_manager.take_changed() {
            let materials = self
                .cam_tools
                .borrow_mut()
                .speeds_feeds_mut()
                .materials
                .clone();
            self.cam_tools
                .borrow_mut()
                .speeds_feeds_mut()
                .replace_libraries(self.tools_manager.tools(), materials);
        }
    }

    /// Keeps CNC Tools' search, filters and last opened tool in the settings.
    fn store_tools_ui_state(&mut self) {
        let mut config = self.state.settings.config().clone();
        if self.tools_manager.store_ui_state(&mut config)
            && let Err(error) = self.state.settings.apply(config)
        {
            tracing::debug!(%error, "CNC Tools filters not kept");
        }
    }

    /// Hands a library file dialog's answer to its window.
    ///
    /// # Returns
    /// Whether the answer was one of its dialogs'.
    pub(super) fn handle_library_answer(&mut self, answer: &DialogAnswer) -> bool {
        let tr = &self.state.translator;
        match answer {
            DialogAnswer::File {
                purpose: FilePurpose::ImportGtcPackage,
                path,
            } => self.tools_manager.import_gtc(path, GtcFile::Package, tr),
            DialogAnswer::File {
                purpose: FilePurpose::ImportGtcJson,
                path,
            } => self.tools_manager.import_gtc(path, GtcFile::Json, tr),
            DialogAnswer::File {
                purpose: FilePurpose::ExportCustomTools,
                path,
            } => self.tools_manager.export_to(path, tr),
            DialogAnswer::Cancelled {
                purpose:
                    FilePurpose::ImportGtcPackage
                    | FilePurpose::ImportGtcJson
                    | FilePurpose::ExportCustomTools,
            } => {}
            DialogAnswer::Unavailable {
                asked:
                    Asked::File(
                        FilePurpose::ImportGtcPackage
                        | FilePurpose::ImportGtcJson
                        | FilePurpose::ExportCustomTools,
                    ),
                reason,
            } => tracing::warn!(%reason, "tool library file dialog unavailable"),
            _ => return false,
        }
        self.sync_libraries();
        true
    }
}
