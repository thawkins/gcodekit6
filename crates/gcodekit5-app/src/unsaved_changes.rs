//! The user's answer to an unsaved-changes prompt, shared by the GTK and egui front ends.
//!
//! The prompt itself is toolkit code (the GTK crate's `show_unsaved_changes`, the egui
//! crate's `Dialogs::confirm`); this module holds only the answer and the button order.
//!
//! No dependencies.

/// The user's answer to an unsaved-changes prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsavedChangesChoice {
    /// Save the work, then carry on with the pending action.
    Save,
    /// Carry on with the pending action without saving.
    Discard,
    /// Abandon the pending action and keep the work as it is.
    Cancel,
}

impl UnsavedChangesChoice {
    /// Button order of the prompt, left to right.
    pub const BUTTONS: [Self; 3] = [Self::Cancel, Self::Discard, Self::Save];

    /// Choice for the index of the button pressed in the prompt.
    ///
    /// # Arguments
    /// * `index` - The pressed button's index, or `None` when the dialog was
    ///   dismissed with Escape or by closing it.
    ///
    /// # Returns
    /// The matching choice; dismissing the dialog or an unknown index is
    /// [`UnsavedChangesChoice::Cancel`], so nothing is lost by accident.
    pub fn from_button_index(index: Option<i32>) -> Self {
        index
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| Self::BUTTONS.get(i).copied())
            .unwrap_or(Self::Cancel)
    }
}
