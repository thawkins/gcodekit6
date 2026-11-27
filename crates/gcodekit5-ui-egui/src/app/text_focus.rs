//! The text field the user was typing in, tracked across the frames a menu takes to open,
//! so Edit > Cut / Copy / Paste chosen from a menu act on that field's text.
//!
//! Dependencies: `egui`, `craft-menu` (whether the menu bar is up).

/// The text field with the keyboard, and the field to give the focus back to.
#[derive(Debug, Default)]
pub(super) struct TextFocus {
    /// The text field the user was typing in, kept while a menu is up (a click on the menu
    /// bar takes the focus from it) so Edit > Cut / Copy / Paste act on its text.
    pub(super) field: Option<egui::Id>,
    /// Frames [`TextFocus::field`] is kept after its field lost the focus with no menu up:
    /// the press and release that open a menu.
    grace: u8,
    /// The text field to give the focus back to at the start of the next frame.
    pub(super) refocus: Option<egui::Id>,
}

impl TextFocus {
    /// Keeps [`TextFocus::field`]: the text field with the keyboard, kept while a menu of
    /// the bar is up and for the frames that open one.
    ///
    /// # Arguments
    /// * `ctx` - The egui context of this frame.
    pub(super) fn track(&mut self, ctx: &egui::Context) {
        let menu_up = craft_menu::bar::is_open(ctx) || craft_menu::bar::is_focused(ctx);
        if ctx.text_edit_focused() {
            self.field = ctx.memory(|m| m.focused());
            self.grace = 2;
        } else if menu_up {
            self.grace = 2;
        } else if self.grace > 0 {
            self.grace -= 1;
        } else {
            self.field = None;
        }
    }
}
