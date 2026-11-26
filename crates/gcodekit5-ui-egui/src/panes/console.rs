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
//! under the log. Up and Down in the entry walk the command history, GTK's
//! [`CommandHistory`]: kept across sessions in GTK's `command_history.json` when the app has
//! a settings folder (see [`ConsolePane::with_history_file`]), at most
//! [`gcodekit5_app::command_history::MAX_HISTORY`] commands. The header
//! has GTK's icon buttons: Clear console, Copy last error (see [`ConsolePane::last_error`])
//! and Help. As GTK, the end of a job is logged: "Job Completed" when every line is
//! acknowledged, "Job stopped:" with the lines done when it ends early, and "Work completed
//! in: m:ss" (from the job's start) when the controller reports the program's end
//! (GRBL 1.1 and grblHAL send `[MSG:Pgm End]` for M2 or M30; `Program End` is matched too).
//! A command typed after the job (not a realtime byte) forgets its start, so an M2 sent by
//! hand later reports no stale time.
//!
//! Ported from the GTK `ui/gtk/device_console.rs` view. Not ported: the `ok` pairing of the
//! GTK listener (acknowledgements are not events here).
//!
//! Dependencies: `egui`, `gcodekit5-app` (console model, controller, translations).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gcodekit5_app::command_history::CommandHistory;
use gcodekit5_app::console::{ConsolePanel, MessageLevel};
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::{MachineController, MachineEvent};

use crate::icons::{Icon, icon_only_button};
use crate::machine::{MachineStatus, lock, protocol_mismatch_text, worker_message};
use crate::windows::help::{self, HelpTopic};
use crate::workspace::{Pane, PaneContext, PaneId};

/// Messages the log keeps; older ones are dropped (the GTK console keeps 5000 lines too).
pub const MAX_CONSOLE_LINES: usize = 5000;

/// The log's prefix for a program the controller halted after a refused line.
const PROGRAM_HALTED: &str = n_!("Program halted (feed hold, then reset)");

/// The msgids the pane shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        PROGRAM_HALTED,
        n_!("Device Console"),
        n_!("Clear console"),
        n_!("Copy last error"),
        n_!("Type a command and press Enter..."),
        n_!("Device connected"),
        n_!("Device disconnected"),
        n_!("Error"),
        JOB_COMPLETED,
        JOB_STOPPED,
        WORK_COMPLETED,
    ]
}

/// GTK's log line when every line of a job is acknowledged.
const JOB_COMPLETED: &str = n_!("Job Completed");
/// GTK's log prefix when a job ends early; the lines done follow.
const JOB_STOPPED: &str = n_!("Job stopped:");
/// GTK's log prefix when the controller reports the program's end; `m:ss` follows.
const WORK_COMPLETED: &str = n_!("\u{2705} Work completed in:");

/// Whether `line` is the controller's report that the program ended (M2/M30): GRBL 1.1 and
/// grblHAL send `[MSG:Pgm End]`; `Program End` is what GTK matched.
///
/// # Arguments
/// * `line` - A received line.
pub fn is_program_end(line: &str) -> bool {
    line.contains("Pgm End") || line.contains("Program End")
}

/// `elapsed` as GTK's `m:ss`.
///
/// # Arguments
/// * `elapsed` - The time since the job started.
pub fn work_time(elapsed: std::time::Duration) -> String {
    let secs = elapsed.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// The Console pane.
pub struct ConsolePane {
    controller: Arc<Mutex<MachineController>>,
    log: ConsolePanel,
    /// The command entry's text.
    input: String,
    /// The commands typed in the entry, with the Up/Down position.
    history: CommandHistory,
    /// Where the history is kept across sessions; `None` keeps it for the session.
    history_file: Option<PathBuf>,
    /// The controller's refusal of the last line typed, shown until the next one.
    notice: Option<String>,
    /// Whether a job streams, so its first progress event marks its start.
    streaming: bool,
    /// When the last job started, until the controller reports the program's end.
    job_started: Option<std::time::Instant>,
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
            history: CommandHistory::new(),
            history_file: None,
            notice: None,
            streaming: false,
            job_started: None,
        }
    }

    /// The same console keeping its command history in `file` (GTK's
    /// `command_history.json`), loaded now and saved after each command; `None` keeps it for
    /// the session.
    ///
    /// # Arguments
    /// * `file` - The history file, if there is a settings folder.
    pub fn with_history_file(mut self, file: Option<PathBuf>) -> Self {
        self.history = file
            .as_deref()
            .map_or_else(CommandHistory::new, CommandHistory::load_from);
        self.history_file = file;
        self
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
                MachineEvent::Sent(line) => {
                    // A manual line after the job ends it: its program end is not the job's.
                    // Realtime bytes (`0x..`, a feed hold or resume) leave the job running.
                    if !self.streaming && !line.starts_with("0x") {
                        self.job_started = None;
                    }
                    self.log.add_command(line);
                }
                MachineEvent::StreamHalted { reason } => self.log.add_message(
                    MessageLevel::Error,
                    format!("{}: {}", tr.tr(PROGRAM_HALTED), worker_message(reason, tr)),
                ),
                MachineEvent::Received(line) => {
                    let line = line.trim();
                    if !line.is_empty() && !is_status_report(line) {
                        self.log.add_message(received_level(line), line);
                    }
                    if is_program_end(line)
                        && let Some(started) = self.job_started.take()
                    {
                        self.log.add_message(
                            MessageLevel::Success,
                            format!("{} {}", tr.tr(WORK_COMPLETED), work_time(started.elapsed())),
                        );
                    }
                }
                MachineEvent::StreamProgress { .. } => {
                    if !self.streaming {
                        self.streaming = true;
                        self.job_started = Some(std::time::Instant::now());
                    }
                }
                MachineEvent::StreamFinished { .. } => {
                    self.streaming = false;
                    self.log
                        .add_message(MessageLevel::Success, tr.tr(JOB_COMPLETED));
                }
                MachineEvent::StreamStopped { acked, total } => {
                    self.streaming = false;
                    self.job_started = None;
                    self.log.add_message(
                        MessageLevel::Warning,
                        format!("{} {acked} / {total}", tr.tr(JOB_STOPPED)),
                    );
                }
                MachineEvent::StatusReport(_) => {}
                MachineEvent::ProtocolMismatch {
                    configured,
                    detected,
                } => self.log.add_message(
                    MessageLevel::Error,
                    protocol_mismatch_text(*configured, *detected, tr),
                ),
            }
        }
    }

    /// The log as text, oldest first: received lines as they came, sent lines as `> line`.
    pub fn lines(&self) -> Vec<String> {
        self.log.messages.iter().map(display_text).collect()
    }

    /// The newest error or alarm line of the log, as GTK's Copy last error found it: the
    /// newest line (acknowledgements and status reports skipped) that mentions "error" or
    /// "alarm".
    pub fn last_error(&self) -> Option<String> {
        self.log.messages.iter().rev().find_map(|message| {
            let text = display_text(message);
            let line = text.trim();
            let skip = line.is_empty() || line == "ok" || line.starts_with('<');
            let low = line.to_lowercase();
            (!skip && (low.contains("error") || low.contains("alarm"))).then(|| line.to_owned())
        })
    }

    /// Draws the header: the title, then Clear console, Copy last error and Help at the
    /// right, as GTK's console header has them.
    fn show_header(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        ui.horizontal(|ui| {
            crate::theme::section_heading(ui, &tr.tr("Device Console"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // GTK's console header had the Machine Control help button.
                help::help_button(ui, HelpTopic::MachineControl, tr);
                if icon_only_button(ui, Icon::Copy, &tr.tr("Copy last error")).clicked()
                    && let Some(line) = self.last_error()
                {
                    ui.ctx().copy_text(line);
                }
                if icon_only_button(ui, Icon::Trash, &tr.tr("Clear console")).clicked() {
                    self.log.clear();
                }
            });
        });
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
                self.history.add(line);
                self.history.reset_navigation();
                if let Some(file) = &self.history_file
                    && let Err(error) = self.history.save_to(file)
                {
                    tracing::warn!(file = %file.display(), %error, "command history not saved");
                }
                self.input.clear();
                self.notice = None;
            }
            Err(error) => self.notice = Some(error.to_string()),
        }
    }

    /// Moves the entry to the previous (`older`) or next history line, as GTK's console:
    /// past the newest line the text typed before browsing comes back.
    fn browse_history(&mut self, older: bool) {
        let line = if older {
            self.history.previous(&self.input)
        } else {
            self.history.next()
        };
        if let Some(line) = line {
            self.input = line;
        }
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
        egui::Panel::top("gcodekit5-console-header")
            .show_separator_line(false)
            .show(ui, |ui| self.show_header(ui, tr));
        egui::Panel::bottom("gcodekit5-console-input")
            .show_separator_line(false)
            .show(ui, |ui| {
                if let Some(notice) = &self.notice {
                    // A controller refusal is kept in English; translated here.
                    ui.colored_label(ui.visuals().error_fg_color, worker_message(notice, tr));
                }
                ui.horizontal(|ui| {
                    // GTK's prompt before the entry.
                    ui.strong(">");
                    self.show_entry(ui, tr, &status);
                });
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
