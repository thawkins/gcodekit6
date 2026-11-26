//! The command entry: one G-code or `$` line typed by the user, sent to the controller with
//! Enter or the send icon button (named Send Command). Refused by the controller while a
//! program streams.
//!
//! While the entry has the keyboard, typed text stays in it (a bare `!` is text, not feed
//! hold); the feed hold and soft reset chords still work (see [`crate::commands`]).
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations).

use gcodekit5_app::i18n::Translator;

use crate::icons::{Icon, icon_only_button};
use crate::machine::MachineStatus;

/// The msgids this section shows.
pub(super) const MSGIDS: &[&str] = &[n_!("Enter G-code command..."), n_!("Send Command")];

/// The command entry.
#[derive(Debug, Default)]
pub(super) struct DirectSender {
    text: String,
}

impl DirectSender {
    /// Draws the entry; returns the line to send when the user sent one.
    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        tr: &Translator,
        status: &MachineStatus,
    ) -> Option<String> {
        let enabled = status.connected && !status.streaming;
        let mut send = false;
        ui.add_enabled_ui(enabled, |ui| {
            ui.horizontal(|ui| {
                let hint = tr.tr("Enter G-code command...");
                // Room for the send button (icon, padding and the gap before it), so the row
                // never grows its column.
                let spacing = ui.spacing();
                let button = crate::icons::ICON_SIZE
                    + 2.0 * spacing.button_padding.x
                    + 2.0 * spacing.item_spacing.x;
                let edit = egui::TextEdit::singleline(&mut self.text)
                    .hint_text(hint.as_ref())
                    .desired_width(ui.available_width() - button);
                let response = ui.add(edit);
                // Named by its hint, which says what to type; egui leaves an entry unnamed.
                response.widget_info(|| {
                    let mut info =
                        egui::WidgetInfo::text_edit(enabled, &self.text, &self.text, hint.as_ref());
                    info.label = Some(hint.clone().into_owned());
                    info
                });
                let entered =
                    response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let clicked = icon_only_button(ui, Icon::Send, &tr.tr("Send Command")).clicked();
                send = entered || clicked;
                if entered {
                    response.request_focus();
                }
            });
        });
        if send && !self.text.trim().is_empty() {
            Some(std::mem::take(&mut self.text))
        } else {
            None
        }
    }
}
