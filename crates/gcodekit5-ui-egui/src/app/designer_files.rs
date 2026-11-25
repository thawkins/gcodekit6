//! The app's side of the designer's file commands: which commands reach the design, the
//! dialogs they ask, the unsaved-changes question, Save from the quit prompt, and the
//! G-code the designer hands to the editor.
//!
//! Routing (GTK routed by the visible tab): File > Open, Save and Save As act on the design
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
//! its file (GTK cleared it too), so the next editor Save asks where. Unsaved editor edits are
//! asked about first (Save / Discard / Cancel; a deliberate change: GTK replaced them
//! silently). While a job streams the request is refused before anything is generated: it
//! would swap the program under the running job, like File > Open. The refusal is shown on
//! the pane that asked. A CAM Preview then brings the visualizer to the front.
//!
//! Dependencies: `gcodekit5-designer` (machine mode), `gcodekit5-app` (translations).

use std::path::Path;

use gcodekit5_app::UnsavedChangesChoice;
use gcodekit5_designer::designer_state::MachineMode;

use super::{AfterSave, GcodeKitApp};
use crate::commands::Command;
use crate::dialogs::{Asked, ConfirmRequest, DialogAnswer, FilePurpose};
use crate::panes::cam_tools::CamDelivery;
use crate::panes::designer::ToEditor;
use crate::workspace::PaneId;

/// A request to replace the editor's program, and who made it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handoff {
    /// The designer's Generate G-Code or Frame (generated when it is delivered).
    Designer(ToEditor),
    /// A CAM tool's Send to Editor or Preview (the program waits in the CAM pane).
    Cam(CamDelivery),
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
    /// A handoff waiting for the editor's unsaved-changes answer.
    pub(super) editor_after_confirm: Option<Handoff>,
}

/// The msgids the app shows for the designer, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Stop the running job before sending G-code to the editor."),
        HANDOFF_DROPPED,
    ]
}

/// Said on the pane whose handoff was dropped because another waits for an answer.
const HANDOFF_DROPPED: &str =
    n_!("Another program is waiting to go to the editor; this one was not sent.");

/// `path` with `extension` when it has none (GTK's default extensions).
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
            self.design.after_confirm = Some(next);
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
        self.design.after_save = Some(then);
        let name = self.design_file_name("gckd");
        self.ask_file(FilePurpose::SaveDesign, Some(&name));
    }

    /// GTK's suggested file name: the design's file name with `extension`, else
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
        let filters = purpose.filters();
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
        self.design.after_save == Some(Some(DesignNext::Quit))
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
                        let then = self.design.after_save.take().flatten();
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
                    | FilePurpose::SaveCamPreset => return false,
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
                if self.design.after_save.take() == Some(Some(DesignNext::Quit)) {
                    self.cancel_quit();
                }
            }
            DialogAnswer::Confirm {
                request: ConfirmRequest::UnsavedDesign,
                choice,
            } if self.design.after_confirm.is_some() => {
                let Some(next) = self.design.after_confirm.take() else {
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
            } if self.design.editor_after_confirm.is_some() => {
                let Some(what) = self.design.editor_after_confirm.take() else {
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
            } if self.design.editor_after_confirm.is_some() => {
                // Without the answer the editor's edits could be lost: send nothing.
                tracing::warn!(%reason, "unsaved-changes prompt unavailable; G-code not sent");
                self.design.editor_after_confirm = None;
            }
            DialogAnswer::Unavailable {
                asked: Asked::Confirm(ConfirmRequest::UnsavedDesign),
                reason,
            } if self.design.after_confirm.is_some() => {
                // Without the answer the unsaved design could be lost: do nothing.
                tracing::warn!(%reason, "unsaved-changes prompt unavailable; design kept");
                self.design.after_confirm = None;
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
    /// editor edits it asks first (Save / Discard / Cancel, as the controller ruled: GTK
    /// replaced them silently).
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

    /// Delivers one handoff, or asks first about the editor's unsaved edits.
    fn deliver_handoff(&mut self, handoff: Handoff) {
        if self.refuse_editor_gcode_during_job(handoff) {
            return;
        }
        if self.design.editor_after_confirm.is_some() {
            // One question at a time: a second request while one waits is dropped, said on
            // its pane, and its program discarded so a later answer cannot deliver it.
            tracing::warn!(?handoff, "editor handoff already waiting; request dropped");
            let message = self.state.translator.tr(HANDOFF_DROPPED).into_owned();
            self.drop_handoff(handoff, message);
            return;
        }
        if self.editor.borrow().buffer().is_dirty() {
            self.design.editor_after_confirm = Some(handoff);
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
                designer.take_editor_gcode()
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
            let shown = self.workspace.apply(craft_ui::docking::Action::Activate {
                panel: PaneId::Visualizer,
            });
            if let Err(error) = shown {
                tracing::warn!(?error, "visualizer not brought to the front");
            }
        }
    }
}
