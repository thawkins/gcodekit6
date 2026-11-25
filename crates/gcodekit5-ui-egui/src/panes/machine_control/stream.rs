//! The job control section: start, pause, resume and stop streaming the loaded program, with
//! a progress bar of the lines the controller acknowledged.
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations).

use gcodekit5_app::i18n::Translator;

use crate::machine::{MachineStatus, Progress};

/// The msgids this section shows.
pub(super) const MSGIDS: &[&str] = &[
    n_!("Job Control"),
    n_!("Send"),
    n_!("Send G-code from the editor"),
    n_!("Pause"),
    n_!("Pause streaming (!)"),
    n_!("Resume"),
    n_!("Resume streaming (~)"),
    n_!("Stop"),
    n_!("No program loaded"),
];

/// What the user asked for in the section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    /// Start streaming the loaded program.
    Start,
    /// Feed hold, and stop feeding lines.
    Pause,
    /// Cycle start, and feed lines again.
    Resume,
    /// Soft reset and drop the rest of the program.
    Stop,
}

/// The progress bar's text: acknowledged lines out of the total.
pub(super) fn progress_text(progress: Progress) -> String {
    format!("{} / {}", progress.acked, progress.total)
}

/// Draws the section.
pub(super) fn show(
    ui: &mut egui::Ui,
    tr: &Translator,
    status: &MachineStatus,
    has_program: bool,
) -> Option<Action> {
    ui.strong(tr.tr("Job Control"));
    let mut action = None;
    ui.horizontal_wrapped(|ui| {
        let can_start = status.connected && !status.streaming && has_program;
        let hint = if has_program {
            tr.tr("Send G-code from the editor")
        } else {
            tr.tr("No program loaded")
        };
        let buttons = [
            (tr.tr("Send"), hint, can_start, Action::Start),
            (
                tr.tr("Pause"),
                tr.tr("Pause streaming (!)"),
                status.streaming,
                Action::Pause,
            ),
            (
                tr.tr("Resume"),
                tr.tr("Resume streaming (~)"),
                status.streaming,
                Action::Resume,
            ),
            (tr.tr("Stop"), tr.tr("Stop"), status.streaming, Action::Stop),
        ];
        for (text, hover, enabled, button_action) in buttons {
            if ui
                .add_enabled(enabled, egui::Button::new(text))
                .on_hover_text(hover)
                .on_disabled_hover_text(tr.tr("No program loaded"))
                .clicked()
            {
                action = Some(button_action);
            }
        }
    });
    if let Some(progress) = status.progress {
        let fraction = if progress.total == 0 {
            1.0
        } else {
            progress.acked as f32 / progress.total as f32
        };
        ui.add(egui::ProgressBar::new(fraction).text(progress_text(progress)));
    }
    action
}
