//! The app's transient flows: the quit being decided, what waits for the editor's save
//! dialog and unsaved-changes prompt, the design's New / Open / Save steps, and the last
//! dialog answer no feature took. [`Flows`] owns that state; the steps that drive it stay
//! in `app.rs` and `designer_files.rs`.
//!
//! Dependencies: the app's dialogs and the documents' dirty flags.

use super::designer_files::{DesignFlow, EditorNext};
use super::{AfterSave, DirtyState};
use crate::dialogs::{ConfirmRequest, DialogAnswer};

/// Where the quit decision stands.
#[derive(Debug, Default)]
pub(super) struct QuitFlow {
    /// The quit prompt queued or on screen, while a quit is being decided.
    pub(super) asking: Option<ConfirmRequest>,
    /// The user agreed to stop the running job for this quit.
    pub(super) stop_job: bool,
    /// The documents whose changes the user chose to discard for this quit.
    pub(super) discarded: DirtyState,
    /// The user chose to discard the CNC Tools page's unsaved tool for this quit.
    pub(super) discarded_tool: bool,
    /// The user chose to discard the Materials Manager's unsaved material for this quit.
    pub(super) discarded_material: bool,
    /// The user chose to discard the Device Manager's unsaved profile for this quit.
    pub(super) discarded_device: bool,
    /// Quitting was decided: the app sent `Close` and lets the close request through.
    pub(super) confirmed: bool,
}

/// The prompts and dialogs in flight, and what each answer goes on to do.
#[derive(Debug, Default)]
pub(super) struct Flows {
    /// The quit being decided, if any.
    pub(super) quit: QuitFlow,
    /// What waits for the editor's save dialog to be answered.
    pub(super) after_save: Option<AfterSave>,
    /// What waits for the editor's unsaved-changes prompt (File > Open or a handoff); one
    /// at a time.
    pub(super) editor_prompt: Option<EditorNext>,
    /// The design's New / Open / Save waiting for a dialog.
    pub(super) design: DesignFlow,
    /// The last dialog answer handled, whether or not a feature took it (for tests).
    pub(super) last_dialog_answer: Option<DialogAnswer>,
}
