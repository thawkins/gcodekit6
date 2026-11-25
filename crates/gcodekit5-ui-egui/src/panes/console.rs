//! The Console pane: the controller's traffic as a log, with a command entry.
//!
//! The log is a [`ConsolePanel`] (the model shared with the GTK front end) bounded to
//! [`MAX_CONSOLE_LINES`] messages, and drawn with `ScrollArea::show_rows` so only the rows in
//! view are laid out however much traffic arrives. It follows the newest line only while the
//! view is already at the bottom, so scrolling up to read stays put.
//!
//! The pane takes the app's machine events through [`Pane::machine_events`] (the same
//! once-per-frame fan-out the Machine Control pane reads, also while the tab is hidden): it
//! logs connection changes, errors and every received line except status reports, which
//! arrive many times a second and are shown in the DRO and status bar instead. A line typed
//! in the entry goes through [`MachineController::send_line`], the path of the Machine Control
//! command entry, and is echoed as `> line` once the controller accepts it; a refusal is shown
//! under the log. Up and Down in the entry walk the lines sent in this session.
//!
//! Ported from the GTK `ui/gtk/device_console.rs` view. Not ported: the persistent command
//! history file (the history lasts for the session), and the `ok` pairing of the GTK listener
//! (acknowledgements are not events here).
//!
//! Dependencies: `egui`, `gcodekit5-app` (console model, controller, translations).

use std::sync::{Arc, Mutex};

use gcodekit5_app::console::{ConsolePanel, MessageLevel};
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::{MachineController, MachineEvent};

use crate::machine::{MachineStatus, lock, worker_message};
use crate::workspace::{Pane, PaneContext, PaneId};

/// Messages the log keeps; older ones are dropped (the GTK console keeps 5000 lines too).
pub const MAX_CONSOLE_LINES: usize = 5000;

/// The log's prefix for a program the controller halted after a refused line.
const PROGRAM_HALTED: &str = n_!("Program halted (feed hold, then reset)");

/// The msgids the pane shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        PROGRAM_HALTED,
        n_!("Clear"),
        n_!("Type a command and press Enter..."),
        n_!("Device connected"),
        n_!("Device disconnected"),
        n_!("Error"),
    ]
}

/// The Console pane.
pub struct ConsolePane {
    controller: Arc<Mutex<MachineController>>,
    log: ConsolePanel,
    /// The command entry's text.
    input: String,
    /// The history line the entry shows (index into the log's history), while browsing.
    history_cursor: Option<usize>,
    /// The controller's refusal of the last line typed, shown until the next one.
    notice: Option<String>,
}

impl std::fmt::Debug for ConsolePane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConsolePane")
            .field("messages", &self.log.message_count())
            .field("notice", &self.notice)
            .finish_non_exhaustive()
    }
}

/// Whether `line` is a status report (`<Idle|MPos:...>`), which is not logged.
fn is_status_report(line: &str) -> bool {
    line.starts_with('<')
}

/// The level a received line is shown at.
fn received_level(line: &str) -> MessageLevel {
    let lower = line.to_ascii_lowercase();
    if lower.starts_with("error") || lower.starts_with("alarm") {
        MessageLevel::Error
    } else {
        MessageLevel::Info
    }
}

impl ConsolePane {
    /// A console whose entry sends through `controller`.
    ///
    /// # Arguments
    /// * `controller` - The app's shared machine controller.
    pub fn new(controller: Arc<Mutex<MachineController>>) -> Self {
        let log = ConsolePanel {
            max_messages: MAX_CONSOLE_LINES,
            ..ConsolePanel::new()
        };
        Self {
            controller,
            log,
            input: String::new(),
            history_cursor: None,
            notice: None,
        }
    }

    /// Logs `events` (oldest first); see the module docs for what is kept.
    ///
    /// # Arguments
    /// * `events` - Machine events since the last call.
    /// * `tr` - Translator for the messages the pane words itself.
    pub fn apply(&mut self, events: &[MachineEvent], tr: &Translator) {
        for event in events {
            match event {
                MachineEvent::Connected => self
                    .log
                    .add_message(MessageLevel::Success, tr.tr("Device connected")),
                MachineEvent::Disconnected => self
                    .log
                    .add_message(MessageLevel::Info, tr.tr("Device disconnected")),
                MachineEvent::Error(message) => self.log.add_message(
                    MessageLevel::Error,
                    format!("{}: {}", tr.tr("Error"), worker_message(message, tr)),
                ),
                MachineEvent::Sent(line) => self.log.add_command(line),
                MachineEvent::StreamHalted { reason } => self.log.add_message(
                    MessageLevel::Error,
                    format!("{}: {}", tr.tr(PROGRAM_HALTED), worker_message(reason, tr)),
                ),
                MachineEvent::Received(line) => {
                    let line = line.trim();
                    if !line.is_empty() && !is_status_report(line) {
                        self.log.add_message(received_level(line), line);
                    }
                }
                MachineEvent::StatusReport(_)
                | MachineEvent::StreamProgress { .. }
                | MachineEvent::StreamFinished { .. }
                | MachineEvent::StreamStopped { .. } => {}
            }
        }
    }

    /// The log as text, oldest first: received lines as they came, sent lines as `> line`.
    pub fn lines(&self) -> Vec<String> {
        self.log.messages.iter().map(display_text).collect()
    }

    /// Sends the entry's line and logs it; a refusal is kept as the notice.
    fn send(&mut self) {
        let line = self.input.trim().to_string();
        if line.is_empty() {
            return;
        }
        let result = lock(&self.controller).send_line(&line);
        match result {
            Ok(()) => {
                // The echo (`> line`) arrives as `MachineEvent::Sent` once it is written.
                self.log.add_to_history(&line);
                self.input.clear();
                self.history_cursor = None;
                self.notice = None;
            }
            Err(error) => self.notice = Some(error.to_string()),
        }
    }

    /// Moves the entry to the previous (`older`) or next history line.
    fn browse_history(&mut self, older: bool) {
        let history = self.log.get_history();
        let next = match (self.history_cursor, older) {
            (_, _) if history.is_empty() => return,
            (None, true) => history.len() - 1,
            (Some(i), true) => i.saturating_sub(1),
            (None, false) => return,
            (Some(i), false) if i + 1 < history.len() => i + 1,
            (Some(_), false) => {
                self.history_cursor = None;
                self.input.clear();
                return;
            }
        };
        self.history_cursor = Some(next);
        self.input.clone_from(&history[next]);
    }

    /// Draws the command entry, enabled while connected and no program streams.
    fn show_entry(&mut self, ui: &mut egui::Ui, tr: &Translator, status: &MachineStatus) {
        let enabled = status.connected && !status.streaming;
        ui.add_enabled_ui(enabled, |ui| {
            let hint = tr.tr("Type a command and press Enter...");
            let edit = egui::TextEdit::singleline(&mut self.input)
                .hint_text(hint.as_ref())
                .desired_width(f32::INFINITY);
            let response = ui.add(edit);
            // Named by its hint, which says what to type; egui leaves an entry unnamed.
            response.widget_info(|| {
                let mut info =
                    egui::WidgetInfo::text_edit(enabled, &self.input, &self.input, hint.as_ref());
                info.label = Some(hint.clone().into_owned());
                info
            });
            if response.has_focus() {
                if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                    self.browse_history(true);
                } else if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                    self.browse_history(false);
                }
            }
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.send();
                response.request_focus();
            }
        });
    }
}

/// The text of one log row.
fn display_text(message: &gcodekit5_app::console::ConsoleMessage) -> String {
    if message.is_command {
        format!("> {}", message.text)
    } else {
        message.text.clone()
    }
}

/// The colour of a log row; the default text colour for plain traffic.
fn level_colour(ui: &egui::Ui, level: MessageLevel) -> Option<egui::Color32> {
    let visuals = ui.visuals();
    match level {
        MessageLevel::Error => Some(visuals.error_fg_color),
        MessageLevel::Warning => Some(visuals.warn_fg_color),
        MessageLevel::Success => Some(visuals.hyperlink_color),
        MessageLevel::Debug => Some(visuals.weak_text_color()),
        MessageLevel::Info => None,
    }
}

impl Pane for ConsolePane {
    fn id(&self) -> PaneId {
        PaneId::Console
    }

    fn title(&self, tr: &Translator) -> String {
        tr.tr(PaneId::Console.title_msgid()).into_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext) {
        let tr = ctx.tr;
        let status = ctx
            .machine()
            .map(|m| m.status().clone())
            .unwrap_or_default();
        egui::Panel::bottom("gcodekit5-console-input")
            .show_separator_line(false)
            .show(ui, |ui| {
                if let Some(notice) = &self.notice {
                    ui.colored_label(ui.visuals().error_fg_color, notice);
                }
                ui.horizontal(|ui| {
                    if ui.button(tr.tr("Clear")).clicked() {
                        self.log.clear();
                    }
                });
                self.show_entry(ui, tr, &status);
            });
        let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
        let rows = self.log.messages.len();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show_rows(ui, row_height, rows, |ui, range| {
                for message in self.log.messages.range(range) {
                    let mut text = egui::RichText::new(display_text(message)).monospace();
                    if let Some(colour) = level_colour(ui, message.level) {
                        text = text.color(colour);
                    }
                    ui.add(egui::Label::new(text).truncate());
                }
            });
    }

    fn machine_events(&mut self, events: &[MachineEvent], tr: &Translator) {
        self.apply(events, tr);
    }
}
