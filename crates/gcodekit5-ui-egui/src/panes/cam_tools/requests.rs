//! What a CAM page asks the app for: a preset file to load or save, shapes for the designer
//! and a folder to choose.
//!
//! Load and Save read and write a page's input as a JSON preset in GTK's file format, so
//! presets saved by either front end load in the other: the pane asks
//! ([`CamToolsPane::take_preset_request`]), the app shows the file dialog and hands the
//! answer back ([`CamToolsPane::load_preset`], [`CamToolsPane::save_preset`]). A preset that
//! cannot be read or written leaves the page as it was, with a notice.
//!
//! The jigsaw and tabbed box pages hand their shapes to the designer
//! ([`CamToolsPane::take_designer_request`]); the Gerber page's Browse asks for a folder
//! ([`CamToolsPane::take_folder_request`]).
//!
//! Dependencies: `gcodekit5-app` (translations).

use std::path::Path;

use gcodekit5_app::i18n::Translator;

use super::common::{CamAction, CamError};
use super::{CamTool, CamToolsPane};

/// A page's Load / Save, waiting for the app's file dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresetRequest {
    /// Choose a preset file to load.
    Load,
    /// Choose where to save; the dialog suggests `file_name` (GTK's).
    Save {
        /// The suggested file name.
        file_name: &'static str,
    },
}

/// A Load / Save waiting for its file: the page, and for a Save the preset's text.
#[derive(Debug)]
pub(super) struct PendingPreset {
    /// The page.
    pub(super) tool: CamTool,
    /// The preset to write (`None` for a Load).
    pub(super) json: Option<String>,
}

/// One outline a page hands to the designer.
#[derive(Clone, Debug, PartialEq)]
pub struct DesignerPath {
    /// The outline's points, in millimetres.
    pub points: Vec<(f64, f64)>,
    /// Whether the outline closes back to its first point.
    pub closed: bool,
}

/// The shapes a page hands to the designer.
#[derive(Clone, Debug, PartialEq)]
pub struct DesignerPaths {
    /// The outlines, in the order the page made them.
    pub paths: Vec<DesignerPath>,
    /// Whether the designer groups them so they move together.
    pub group: bool,
}

impl CamToolsPane {
    /// Takes the Load / Save request; the app answers it with a file dialog.
    pub fn take_preset_request(&mut self) -> Option<PresetRequest> {
        self.preset_request.take()
    }

    /// Loads the preset at `path` into the page that asked; a file that cannot be read or is
    /// not that page's preset leaves the page as it was, with a notice.
    ///
    /// # Arguments
    /// * `path` - The chosen file.
    /// * `tr` - Translator for the current UI language.
    pub fn load_preset(&mut self, path: &Path, tr: &Translator) {
        let Some(PendingPreset { tool, .. }) = self.preset.take() else {
            return;
        };
        let loaded = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|json| match tool {
                CamTool::DrillPress => self.drill_press.apply_preset(&json),
                CamTool::SpoilboardSurfacing => self.spoilboard_surfacing.apply_preset(&json),
                CamTool::SpoilboardGrid => self.spoilboard_grid.apply_preset(&json),
                CamTool::Jigsaw => self.jigsaw.apply_preset(&json),
                CamTool::TabbedBox => self.tabbed_box.apply_preset(&json),
                // The calculator has no presets (GTK's had none).
                CamTool::SpeedsFeeds => Ok(()),
            });
        match loaded {
            Ok(()) => self.notice = None,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "CAM preset not loaded");
                self.notice = Some(format!("{}: {}: {e}", tr.tr("Load Error"), path.display()));
            }
        }
    }

    /// Writes the preset the Save made to `path` (atomically); a failure is shown as a
    /// notice.
    ///
    /// # Arguments
    /// * `path` - The chosen file.
    /// * `tr` - Translator for the current UI language.
    pub fn save_preset(&mut self, path: &Path, tr: &Translator) {
        let Some(json) = self.preset.take().and_then(|p| p.json) else {
            return;
        };
        if let Err(e) = crate::files::write_atomic(path, &json) {
            tracing::error!(path = %path.display(), error = %e, "CAM preset not saved");
            self.notice = Some(format!("{}: {}: {e}", tr.tr("Save Error"), path.display()));
        }
    }

    /// Forgets the Load / Save whose file dialog was closed without a file.
    pub fn cancel_preset(&mut self) {
        self.preset = None;
    }

    /// Asks for `tool`'s Load or Save; a Save that cannot be made says why.
    pub(super) fn ask_preset(&mut self, tool: CamTool, action: CamAction, tr: &Translator) {
        if action == CamAction::LoadPreset {
            self.preset = Some(PendingPreset { tool, json: None });
            self.preset_request = Some(PresetRequest::Load);
            return;
        }
        let (saved, file_name) = match tool {
            CamTool::DrillPress => (self.drill_press.preset_json(tr), "drill_params.json"),
            CamTool::SpoilboardSurfacing => (
                self.spoilboard_surfacing.preset_json(tr),
                "surfacing_params.json",
            ),
            CamTool::SpoilboardGrid => (self.spoilboard_grid.preset_json(tr), "grid_params.json"),
            CamTool::Jigsaw => (self.jigsaw.preset_json(tr), "puzzle_params.json"),
            CamTool::TabbedBox => (self.tabbed_box.preset_json(tr), "box_params.json"),
            CamTool::SpeedsFeeds => return,
        };
        match saved {
            Ok(json) => {
                self.preset = Some(PendingPreset {
                    tool,
                    json: Some(json),
                });
                self.preset_request = Some(PresetRequest::Save { file_name });
            }
            Err(e) => self.notice = Some(format!("{}: {e}", tr.tr("Save Error"))),
        }
    }

    /// Takes the shapes a page asked to add to the design; the app adds them and brings the
    /// designer to the front.
    pub fn take_designer_request(&mut self) -> Option<DesignerPaths> {
        self.designer_request.take()
    }

    /// Takes the Browse request; the app answers it with a folder dialog.
    pub fn take_folder_request(&mut self) -> bool {
        std::mem::take(&mut self.folder_request)
    }

    /// Makes `tool`'s shapes for the designer, or shows why not.
    pub(super) fn send_to_designer(&mut self, tool: CamTool, tr: &Translator) {
        let made = match tool {
            CamTool::Jigsaw => self.jigsaw.generate(),
            CamTool::TabbedBox => self.tabbed_box.generate_shapes(),
            CamTool::DrillPress
            | CamTool::SpoilboardSurfacing
            | CamTool::SpoilboardGrid
            | CamTool::SpeedsFeeds => Err(CamError::Invalid),
        };
        match made {
            Ok(paths) => {
                self.notice = None;
                self.designer_request = Some(paths);
            }
            Err(error) => self.notice = Some(error.message(tr)),
        }
    }
}
