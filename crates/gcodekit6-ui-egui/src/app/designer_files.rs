//! The app's side of the designer's file commands: which commands reach the design, the
//! dialogs they ask, the unsaved-changes question, Save from the quit prompt, and the
//! G-code the designer hands to the editor.
//!
//! Routing: File > Open, Save and Save As act on the design
//! while a designer pane (the canvas, Layers or Properties) is the active pane, otherwise on
//! the G-code editor. New 2D / New 3D, Import, Import Image, Export G-Code and Export SVG are
//! design commands and always reach the design.
//!
//! New and Open ask first when the design has unsaved changes: Save saves (asking where when
//! the design has no file) and goes on only once the save succeeded, Discard goes on, Cancel
//! (or a prompt that cannot be shown) does nothing. Save in the quit prompt works the same
//! way: the quit continues after a successful save, and a failed or cancelled save cancels
//! it. After every save the app's dirty cache and `AppState::dirty.designer` are cleared.
//!
//! The editor handoff ([`Handoff`]): the designer's Generate G-Code and Frame, and a CAM
//! tool's Send to Editor and Preview, replace the editor's program, and the editor forgets
//! its file, so the next editor Save asks where. Unsaved editor edits are
//! asked about first (Save / Discard / Cancel; they are never replaced
//! silently). While a job streams the request is refused before anything is generated: it
//! would swap the program under the running job, like File > Open. The refusal is shown on
//! the pane that asked. A CAM Preview then brings the visualizer to the front.
//!
//! A designer handoff also records the Designer's stock size and tool diameter in
//! [`super::AppState::stock_seed`], which the visualizer's stock removal simulation takes with
//! the new program.
//!
//! Dependencies: `gcodekit6-designer` (machine mode), `gcodekit6-app` (translations).

use std::path::Path;

use gcodekit6_app::UnsavedChangesChoice;
use gcodekit6_designer::designer_state::{DesignerState, MachineMode};

use super::{AfterSave, GcodeKitApp};
use crate::commands::Command;
use crate::dialogs::{Asked, ConfirmRequest, DialogAnswer, FilePurpose};
use crate::panes::cam_tools::CamDelivery;
use crate::panes::designer::ToEditor;
use crate::panes::visualizer::StockSeed;
use crate::workspace::PaneId;

/// A request to replace the editor's program, and who made it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handoff {
    /// The designer's Generate G-Code or Frame (generated when it is delivered).
    Designer(ToEditor),
    /// A CAM tool's Send to Editor or Preview (the program waits in the CAM pane).
    Cam(CamDelivery),
}

/// What waits for the answer to the editor's unsaved-changes prompt. There is one slot, so
/// only one such prompt is ever up and its answer cannot reach another request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EditorNext {
    /// File > Open of a G-code file.
    Open,
    /// A handoff to the editor.
    ToEditor(Handoff),
}

/// What runs once the design's unsaved changes are dealt with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DesignNext {
    /// Start a new design in this mode.
    New(MachineMode),
    /// Ask for a design file to open.
    Open,
    /// Go on quitting.
    Quit,
}

/// The design's pending steps: after the unsaved-changes question, and after a save dialog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct DesignFlow {
    /// New or Open waiting for the unsaved-changes answer.
    pub(super) after_confirm: Option<DesignNext>,
    /// What waits for the design's save dialog; `Some(None)` is a plain Save As.
    pub(super) after_save: Option<Option<DesignNext>>,
}

/// The msgids the app shows for the designer, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Stop the running job before sending G-code to the editor."),
        HANDOFF_DROPPED,
        HANDOFF_BEHIND_OPEN,
        OPEN_BEHIND_HANDOFF,
    ]
}

/// Said on the pane whose handoff was dropped because another waits for an answer.
const HANDOFF_DROPPED: &str =
    n_!("Another program is waiting to go to the editor; this one was not sent.");
/// Said on the pane whose handoff was refused because File > Open waits for an answer.
const HANDOFF_BEHIND_OPEN: &str =
    n_!("A file is waiting to open in the editor; this program was not sent.");
/// Said on the editor when File > Open is refused because a handoff waits for an answer.
pub(super) const OPEN_BEHIND_HANDOFF: &str =
    n_!("Answer the question about the editor's unsaved changes before opening a file.");

/// The Designer's stock, if it has one, and tool, for the visualizer's stock removal
/// simulation.
///
/// # Arguments
/// * `state` - The Designer's state.
/// * `serial` - Tags this program's seed.
fn stock_seed(state: &DesignerState, serial: u64) -> StockSeed {
    StockSeed {
        serial,
        stock: state
            .stock_material
            .as_ref()
            .map(|stock| [stock.width, stock.height, stock.thickness]),
        tool_diameter: state.tool_settings.tool_diameter as f32,
    }
}

/// `path` with `extension` when it has none (the default extensions).
fn with_default_extension(path: &Path, extension: &str) -> std::path::PathBuf {
    let mut path = path.to_path_buf();
    if path.extension().is_none() {
        path.set_extension(extension);
    }
    path
}

impl GcodeKitApp {
    /// Whether the design is the document the user works in: a designer pane is active.
    pub(super) fn designer_active(&self) -> bool {
        matches!(
            self.workspace.active_pane(),
            Some(PaneId::Designer | PaneId::DesignerLayers | PaneId::DesignerProperties)
        )
    }

    /// Runs a design command; returns `false` for a command that is not one (or, for Open,
    /// Save and Save As, when the design is not the active document).
    pub(super) fn dispatch_design(&mut self, cmd: Command) -> bool {
        match cmd {
            Command::FileNew => self.design_then(DesignNext::New(MachineMode::Laser2D)),
            Command::FileNew3d => self.design_then(DesignNext::New(MachineMode::Cnc3D)),
            Command::FileOpen if self.designer_active() => self.design_then(DesignNext::Open),
            Command::FileSave if self.designer_active() => self.save_design(None),
            Command::FileSaveAs if self.designer_active() => self.ask_save_design(None),
            Command::FileImport => self.ask_file(FilePurpose::ImportDesign, None),
            Command::FileImportImage => {
                let cnc = self.designer.borrow().state().machine_mode() == MachineMode::Cnc3D;
                if cnc {
                    let tr = &self.state.translator;
                    let message = tr.tr("Raster image import is only available in 2D mode");
                    self.designer.borrow_mut().set_notice(message.into_owned());
                } else {
                    self.ask_file(FilePurpose::ImportImage, None);
                }
            }
            Command::FileExportGcode => {
                // Refused before asking where (a cheap check, no generation), so no dialog is
                // shown for nothing.
                let refusal = self.designer.borrow().export_check().err();
                match refusal {
                    Some(refusal) => {
                        let message = refusal.message(&self.state.translator, true);
                        self.designer.borrow_mut().set_notice(message);
                    }
                    None => {
                        let name = self.design_file_name("nc");
                        self.ask_file(FilePurpose::ExportGcode, Some(&name));
                    }
                }
            }
            Command::FileExportSvg => {
                let name = self.design_file_name("svg");
                self.ask_file(FilePurpose::ExportSvg, Some(&name));
            }
            _ => return false,
        }
        true
    }

    /// Runs `next` now, or first asks about the design's unsaved changes.
    fn design_then(&mut self, next: DesignNext) {
        if self.designer.borrow().is_modified() {
            self.flows.design.after_confirm = Some(next);
            self.dialogs.confirm(ConfirmRequest::UnsavedDesign);
        } else {
            self.run_design_next(next);
        }
    }

    /// Runs `next` once the design is saved or its changes discarded.
    fn run_design_next(&mut self, next: DesignNext) {
        match next {
            DesignNext::New(mode) => {
                self.designer
                    .borrow_mut()
                    .new_design(mode, &self.state.translator);
                self.sync_designer();
            }
            DesignNext::Open => self.ask_file(FilePurpose::OpenDesign, None),
            DesignNext::Quit => self.continue_quit(),
        }
    }

    /// Saves the design to its file, or asks where when it has none; `then` runs once it is
    /// saved (the quit prompt's Save, or New / Open after Save).
    pub(super) fn save_design(&mut self, then: Option<DesignNext>) {
        let path = self.designer.borrow().design_path().map(Path::to_path_buf);
        match path {
            Some(path) => {
                let saved = self
                    .designer
                    .borrow_mut()
                    .save_design(&path, &self.state.translator);
                self.after_design_saving(saved, then);
            }
            None => self.ask_save_design(then),
        }
    }

    /// Asks where to save the design; `then` waits for the answer.
    fn ask_save_design(&mut self, then: Option<DesignNext>) {
        self.flows.design.after_save = Some(then);
        let name = self.design_file_name("gckd");
        self.ask_file(FilePurpose::SaveDesign, Some(&name));
    }

    /// The suggested file name: the design's file name with `extension`, else
    /// `untitled_design`.
    fn design_file_name(&self, extension: &str) -> String {
        let stem = self
            .designer
            .borrow()
            .design_path()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "untitled_design".to_string());
        format!("{stem}.{extension}")
    }

    /// Queues a file dialog for `purpose` with its filters (and a suggested name for a save).
    fn ask_file(&mut self, purpose: FilePurpose, file_name: Option<&str>) {
        let filters = match purpose {
            FilePurpose::ImportDesign => crate::dialogs::import_design_filters(
                self.state.settings.config().ui.enable_stl_import,
            ),
            _ => purpose.filters(),
        };
        match file_name {
            Some(name) => self.dialogs.save_file_named(purpose, &filters, name),
            None => self.dialogs.open_file(purpose, &filters),
        }
    }

    /// Clears the dirty flags after a successful save and runs `then`; a failed save that a
    /// quit waited for cancels the quit (the notice says why).
    fn after_design_saving(&mut self, saved: bool, then: Option<DesignNext>) {
        if saved {
            self.designer_dirty = false;
            self.state.dirty.designer = false;
        }
        match (saved, then) {
            (true, Some(next)) => self.run_design_next(next),
            (false, Some(DesignNext::Quit)) => self.cancel_quit(),
            (false, Some(DesignNext::New(_) | DesignNext::Open)) | (_, None) => {}
        }
    }

    /// Whether a quit waits for the design's save dialog.
    pub(super) fn design_saving_for_quit(&self) -> bool {
        self.flows.design.after_save == Some(Some(DesignNext::Quit))
    }

    /// Acts on a dialog answer that belongs to the design.
    ///
    /// # Returns
    /// Whether the answer was the design's.
    pub(super) fn handle_design_answer(&mut self, answer: &DialogAnswer) -> bool {
        let tr = self.state.translator;
        match answer {
            DialogAnswer::File { purpose, path } => {
                let mut designer = self.designer.borrow_mut();
                match purpose {
                    FilePurpose::OpenDesign => {
                        designer.open_design(path, &tr);
                    }
                    FilePurpose::SaveDesign => {
                        let path = with_default_extension(path, "gckd");
                        let saved = designer.save_design(&path, &tr);
                        drop(designer);
                        let then = self.flows.design.after_save.take().flatten();
                        self.after_design_saving(saved, then);
                    }
                    FilePurpose::ImportDesign => {
                        designer.import_file(path, &tr);
                    }
                    FilePurpose::ImportImage => {
                        designer.import_image(path, &tr);
                    }
                    FilePurpose::ExportGcode => {
                        designer.write_gcode(&with_default_extension(path, "nc"), &tr);
                    }
                    FilePurpose::ExportSvg => {
                        designer.write_svg(&with_default_extension(path, "svg"), &tr);
                    }
                    FilePurpose::OpenGcode
                    | FilePurpose::SaveGcode
                    | FilePurpose::LoadCamPreset
                    | FilePurpose::SaveCamPreset
                    | FilePurpose::OpenGerberFolder
                    | FilePurpose::OpenEngravingImage
                    | FilePurpose::ExportFirmwareSettings
                    | FilePurpose::ImportFirmwareSettings
                    | FilePurpose::ImportGtcPackage
                    | FilePurpose::ImportGtcJson
                    | FilePurpose::ExportCustomTools => return false,
                }
            }
            DialogAnswer::Cancelled {
                purpose: FilePurpose::SaveDesign,
            }
            | DialogAnswer::Unavailable {
                asked: Asked::File(FilePurpose::SaveDesign),
                ..
            } => {
                if let DialogAnswer::Unavailable { reason, .. } = answer {
                    tracing::warn!(%reason, "design save dialog unavailable");
                }
                if self.flows.design.after_save.take() == Some(Some(DesignNext::Quit)) {
                    self.cancel_quit();
                }
            }
            DialogAnswer::Confirm {
                request: ConfirmRequest::UnsavedDesign,
                choice,
            } if self.flows.design.after_confirm.is_some() => {
                let Some(next) = self.flows.design.after_confirm.take() else {
                    return true;
                };
                match choice.as_unsaved() {
                    UnsavedChangesChoice::Save => self.save_design(Some(next)),
                    UnsavedChangesChoice::Discard => self.run_design_next(next),
                    UnsavedChangesChoice::Cancel => {}
                }
            }
            DialogAnswer::Confirm {
                request: ConfirmRequest::UnsavedGcode,
                choice,
            } if matches!(self.flows.editor_prompt, Some(EditorNext::ToEditor(_))) => {
                let Some(EditorNext::ToEditor(what)) = self.flows.editor_prompt.take() else {
                    return true;
                };
                match choice.as_unsaved() {
                    UnsavedChangesChoice::Save => self.save_editor(Some(AfterSave::ToEditor(what))),
                    UnsavedChangesChoice::Discard => self.send_to_editor(what),
                    UnsavedChangesChoice::Cancel => {}
                }
            }
            DialogAnswer::Unavailable {
                asked: Asked::Confirm(ConfirmRequest::UnsavedGcode),
                reason,
            } if matches!(self.flows.editor_prompt, Some(EditorNext::ToEditor(_))) => {
                // Without the answer the editor's edits could be lost: send nothing.
                tracing::warn!(%reason, "unsaved-changes prompt unavailable; G-code not sent");
                self.flows.editor_prompt = None;
            }
            DialogAnswer::Unavailable {
                asked: Asked::Confirm(ConfirmRequest::UnsavedDesign),
                reason,
            } if self.flows.design.after_confirm.is_some() => {
                // Without the answer the unsaved design could be lost: do nothing.
                tracing::warn!(%reason, "unsaved-changes prompt unavailable; design kept");
                self.flows.design.after_confirm = None;
            }
            DialogAnswer::Cancelled { .. }
            | DialogAnswer::Unavailable { .. }
            | DialogAnswer::Confirm { .. }
            | DialogAnswer::QuitConfirm { .. } => return false,
        }
        true
    }

    /// Runs the designer's Generate G-Code / Frame request and a CAM tool's Send to Editor /
    /// Preview: refused while a job streams (before anything is generated); with unsaved
    /// editor edits it asks first (Save / Discard / Cancel; never replaced
    /// silently).
    pub(super) fn deliver_editor_gcode(&mut self) {
        let designer = self
            .designer
            .borrow_mut()
            .take_editor_request()
            .map(Handoff::Designer);
        let cam = self
            .cam_tools
            .borrow_mut()
            .take_editor_request()
            .map(Handoff::Cam);
        for handoff in [designer, cam].into_iter().flatten() {
            self.deliver_handoff(handoff);
        }
    }

    /// Adds the shapes a CAM page generated to the design and brings the designer to the
    /// front.
    pub(super) fn deliver_designer_shapes(&mut self) {
        let Some(generated) = self.cam_tools.borrow_mut().take_designer_request() else {
            return;
        };
        self.designer.borrow_mut().add_generated_paths(&generated);
        self.workspace.show_pane(PaneId::Designer);
    }

    /// Delivers one handoff, or asks first about the editor's unsaved edits.
    fn deliver_handoff(&mut self, handoff: Handoff) {
        if self.refuse_editor_gcode_during_job(handoff) {
            return;
        }
        if let Some(waiting) = self.flows.editor_prompt {
            // One question at a time: a request while one waits (another handoff's or File >
            // Open's) is dropped, said on its pane, and its program discarded so a later
            // answer cannot deliver it.
            tracing::warn!(
                ?handoff,
                ?waiting,
                "editor prompt already up; handoff dropped"
            );
            let msgid = match waiting {
                EditorNext::Open => HANDOFF_BEHIND_OPEN,
                EditorNext::ToEditor(_) => HANDOFF_DROPPED,
            };
            let message = self.state.translator.tr(msgid).into_owned();
            self.drop_handoff(handoff, message);
            return;
        }
        if self.editor.borrow().buffer().is_dirty() {
            self.flows.editor_prompt = Some(EditorNext::ToEditor(handoff));
            self.dialogs.confirm(ConfirmRequest::UnsavedGcode);
        } else {
            self.send_to_editor(handoff);
        }
    }

    /// While a job streams, replacing the editor's program would swap it under the job:
    /// says so on the pane that asked and returns `true`.
    fn refuse_editor_gcode_during_job(&mut self, handoff: Handoff) -> bool {
        if !self.state.job_running() {
            return false;
        }
        let message = self
            .state
            .translator
            .tr("Stop the running job before sending G-code to the editor.")
            .into_owned();
        self.drop_handoff(handoff, message);
        true
    }

    /// Discards `handoff`'s program and says `message` on the pane that asked.
    fn drop_handoff(&mut self, handoff: Handoff, message: String) {
        match handoff {
            Handoff::Designer(_) => {
                let mut designer = self.designer.borrow_mut();
                designer.take_editor_gcode();
                designer.set_notice(message);
            }
            Handoff::Cam(_) => {
                let mut cam = self.cam_tools.borrow_mut();
                cam.take_editor_gcode();
                cam.set_notice(message);
            }
        }
    }

    /// Replaces the editor's program with the handoff's (generating the designer's now);
    /// the editor forgets its file, so its next Save asks where. A CAM Preview then brings
    /// the visualizer to the front.
    pub(super) fn send_to_editor(&mut self, handoff: Handoff) {
        // A job may have started while the question was up.
        if self.refuse_editor_gcode_during_job(handoff) {
            return;
        }
        let tr = self.state.translator;
        let gcode = match handoff {
            Handoff::Designer(what) => {
                let mut designer = self.designer.borrow_mut();
                match what {
                    ToEditor::Generate => designer.generate_to_editor(&tr),
                    ToEditor::Frame => designer.frame_to_editor(),
                }
                let gcode = designer.take_editor_gcode();
                if gcode.is_some() {
                    self.state.stock_seed = Some(stock_seed(
                        designer.state(),
                        self.state
                            .stock_seed
                            .map_or(1, |s| s.serial.wrapping_add(1)),
                    ));
                }
                gcode
            }
            Handoff::Cam(_) => self.cam_tools.borrow_mut().take_editor_gcode(),
        };
        let Some(gcode) = gcode else {
            return;
        };
        self.editor
            .borrow_mut()
            .buffer_mut()
            .replace_with_generated(gcode);
        self.sync_editor(true);
        if handoff == Handoff::Cam(CamDelivery::Preview) {
            self.workspace.show_pane(PaneId::Visualizer);
        }
    }
}
