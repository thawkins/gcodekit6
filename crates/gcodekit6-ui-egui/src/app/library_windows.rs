//! The app's side of the CNC Tools and Materials pages: the GTC import and
//! export file dialogs CNC Tools asks for, handing a changed library to the speeds and
//! feeds calculator at once, and keeping CNC Tools' search and filters in the settings
//! whenever they change.
//!
//! Dependencies: the app's dialogs, settings and windows.

use super::GcodeKitApp;
use crate::dialogs::{Asked, ConfirmRequest, DialogAnswer, FilePurpose};
use crate::windows::tools_manager::{GtcFile, ToolsFileRequest};
use crate::windows::{MaterialsManagerWindow, ToolsManagerWindow};
use crate::workspace::Page;
use gcodekit6_app::UnsavedChangesChoice;

impl GcodeKitApp {
    /// The CNC Tools page.
    pub fn tools_manager(&self) -> &ToolsManagerWindow {
        &self.managers.tools_manager
    }

    /// The CNC Tools page, to drive it directly (tests).
    pub fn tools_manager_mut(&mut self) -> &mut ToolsManagerWindow {
        &mut self.managers.tools_manager
    }

    /// The Materials page.
    pub fn materials_manager(&self) -> &MaterialsManagerWindow {
        &self.managers.materials_manager
    }

    /// The Materials page, to drive it directly (tests).
    pub fn materials_manager_mut(&mut self) -> &mut MaterialsManagerWindow {
        &mut self.managers.materials_manager
    }

    /// Shows the file dialog CNC Tools asked for, and hands a changed library to the speeds
    /// and feeds calculator.
    pub(super) fn show_library_windows(&mut self) {
        self.store_tools_ui_state();
        match self.managers.tools_manager.take_file_request() {
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
        let tools = self.managers.tools_manager.take_changed();
        let materials = self.managers.materials_manager.take_changed();
        if tools || materials {
            self.cam_tools
                .borrow_mut()
                .speeds_feeds_mut()
                .replace_libraries(
                    self.managers.tools_manager.tools(),
                    self.managers.materials_manager.materials(),
                );
        }
    }

    /// Keeps CNC Tools' search, filters and last opened tool in the settings whenever they
    /// change (a failed write is not retried until they change again).
    fn store_tools_ui_state(&mut self) {
        if !self.managers.tools_manager.ui_state_changed() {
            return;
        }
        let mut config = self.state.settings.config().clone();
        self.managers.tools_manager.store_ui_state(&mut config);
        if let Err(error) = self.state.settings.apply(config) {
            tracing::debug!(%error, "CNC Tools filters not kept");
        }
    }

    /// Acts on the quit flow's question about an unsaved tool or material: Save saves it
    /// and goes on quitting (a refused save stops the quit and shows the page with the
    /// reason), Discard goes on without it, Cancel stops the quit and shows the page.
    pub(super) fn library_quit_answer(
        &mut self,
        request: ConfirmRequest,
        choice: UnsavedChangesChoice,
    ) {
        let tools = request == ConfirmRequest::UnsavedTool;
        let page = if tools {
            Page::CncTools
        } else {
            Page::Materials
        };
        match choice {
            UnsavedChangesChoice::Save => {
                let tr = &self.state.translator;
                let saved = if tools {
                    self.managers.tools_manager.save_for_quit(tr)
                } else {
                    self.managers.materials_manager.save_for_quit(tr)
                };
                self.sync_libraries();
                if saved {
                    self.continue_quit();
                } else {
                    self.show_page(page);
                    self.cancel_quit();
                }
            }
            UnsavedChangesChoice::Discard => {
                if tools {
                    self.flows.quit.discarded_tool = true;
                } else {
                    self.flows.quit.discarded_material = true;
                }
                self.continue_quit();
            }
            UnsavedChangesChoice::Cancel => {
                self.show_page(page);
                self.cancel_quit();
            }
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
            } => self
                .managers
                .tools_manager
                .import_gtc(path, GtcFile::Package, tr),
            DialogAnswer::File {
                purpose: FilePurpose::ImportGtcJson,
                path,
            } => self
                .managers
                .tools_manager
                .import_gtc(path, GtcFile::Json, tr),
            DialogAnswer::File {
                purpose: FilePurpose::ExportCustomTools,
                path,
            } => self.managers.tools_manager.export_to(path, tr),
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
