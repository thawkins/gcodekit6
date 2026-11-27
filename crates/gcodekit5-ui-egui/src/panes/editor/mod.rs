//! The G-code Editor pane: the program text with syntax highlighting, a line-number gutter,
//! find, and the line the streaming job is on.
//!
//! Files up to [`LARGE_FILE_LINES`] lines are edited in an `egui::TextEdit` whose layouter
//! highlights the text; the laid-out text is cached by content, so a frame without an edit
//! does not lay it out again. Larger files are shown read-only and virtualised: only the rows
//! in view are highlighted, laid out and painted, so a frame costs the same for a million
//! lines as for a hundred. "Edit anyway" switches a large file to the text field, at the cost
//! of laying the whole text out on each edit.
//!
//! Undo and redo are the text field's own (Ctrl+Z, Ctrl+Y / Ctrl+Shift+Z while it has the
//! keyboard). Find matches ignore ASCII case and wrap around; a match becomes the current
//! line and is scrolled into view. While a job streams, the line in progress becomes the
//! current line: [`Pane::machine_events`] maps the streamed line back to the editor line
//! ([`buffer::stream_line_map`]) and scrolls to it.
//!
//! The app owns the pane through a shared handle ([`SharedEditor`]): it opens and saves files
//! for the File commands and hands the text to the machine as the program to stream
//! ([`EditorPane::program_stale`], [`EditorPane::program_fed`]).
//!
//! Clear editor sits at the end of the
//! find row: it is disabled, and refused, while a job runs (the text is read-only then), and
//! asks first when the editor holds any G-code; the question is asked again at
//! Yes, which is refused if a job started meanwhile.
//!
//! Under the find row, the replace row: "Replace with ...", Replace (the current match,
//! then on to the next; with no current match it moves to the first) and Replace All. Both
//! are enabled when the query has matches and the replacement is not empty, and
//! are disabled, and refused, while a job runs or a large file is shown read-only. Each is
//! one step of the text field's undo: the text before it is recorded as an undo point (see
//! [`EditorPane::pending_undo`]) before the field draws the new text.
//!
//! There is no line-counter
//! tooltip, and the find row shows the match count only, not a match index ("2 of 5").
//!
//! Dependencies: `egui`, `anyhow`, `gcodekit5-app` (machine events, translations).

pub mod buffer;
pub mod find;
pub mod gutter;
pub mod highlight;
mod view;

use std::cell::RefCell;
use std::ops::Range;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{FontId, Galley};
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::MachineEvent;

pub use buffer::EditorBuffer;
use highlight::HighlightStyle;

use crate::dialogs::{ConfirmChoice, ConfirmRequest};
use crate::icons::{Icon, icon_only_button};
use crate::workspace::{Pane, PaneContext, PaneId};

/// Files with more lines than this open read-only in the virtualised view.
pub const LARGE_FILE_LINES: usize = 50_000;

/// How long after the last keystroke the edited text is handed to the machine.
pub const PROGRAM_SETTLE: Duration = Duration::from_millis(300);

/// Frames a scroll request is retried.
const SCROLL_TRIES: u8 = 2;

/// Font size of the editor's text.
const FONT_SIZE: f32 = 13.0;

/// Shown while a job runs: the text cannot be changed.
const READ_ONLY_DURING_JOB: &str = n_!("Read-only while a job is running.");

/// The msgids the pane shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Search..."),
        n_!("Previous Match"),
        n_!("Next Match"),
        n_!("Matches:"),
        n_!("Untitled"),
        n_!("Line:"),
        n_!("Edit anyway"),
        n_!("Large file: shown read-only to stay fast."),
        n_!("Could not open file"),
        n_!("Stop the running job before opening a file."),
        n_!("Could not save file"),
        n_!("Cannot stream this program"),
        READ_ONLY_DURING_JOB,
        n_!("Clear editor"),
        n_!("Save G-code"),
        n_!("Replace with ..."),
        n_!("Replace"),
        n_!("Replace All"),
    ]
}

/// The laid-out text of the text field, kept while the text and style stay the same.
#[derive(Debug)]
struct GalleyCache {
    hash: u64,
    style: HighlightStyle,
    galley: Arc<Galley>,
}

/// The G-code Editor pane.
#[derive(Debug)]
pub struct EditorPane {
    buffer: EditorBuffer,
    /// The user chose to edit a large file in the text field.
    edit_large: bool,
    /// A job is streaming: the text is read-only so its lines keep matching the stream.
    locked: bool,
    find_query: String,
    /// Byte range of the last find match.
    find_match: Option<Range<usize>>,
    /// The last find match is still to be selected in the text field.
    select_match: bool,
    /// Match count of `(query, generation)`, so it is not recounted every frame.
    find_count: Option<(String, u64, usize)>,
    /// What Replace and Replace All put in place of a match.
    replace_text: String,
    /// The text before a replace, to record as the text field's undo point before the field
    /// next draws (so the replace is one undo step).
    pending_undo: Option<String>,
    /// A line to scroll into view, and the frames left to try (the scroll area clamps an
    /// offset to the previous frame's content, so the first try can fall short).
    scroll_to: Option<(usize, u8)>,
    /// The longest line's length, by text generation.
    longest: Option<(u64, usize)>,
    /// The lines in view on the last frame.
    visible: Range<usize>,
    galley_cache: Option<GalleyCache>,
    /// The generation of the text last handed to the machine.
    fed: Option<u64>,
    /// When the text was last edited.
    last_edit: Option<Instant>,
    /// For each streamed line of the program handed to the machine, its editor line.
    stream_map: Vec<usize>,
    /// The text field's cursor (char index) on the last frame.
    last_cursor: Option<usize>,
    /// An error to show under the text until the next file operation.
    notice: Option<String>,
    /// A confirmation the app is to ask for (Clear editor).
    confirm: Option<ConfirmRequest>,
}

impl Default for EditorPane {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorPane {
    /// An empty editor with no file.
    pub fn new() -> Self {
        Self {
            buffer: EditorBuffer::default(),
            edit_large: false,
            locked: false,
            find_query: String::new(),
            find_match: None,
            select_match: false,
            find_count: None,
            replace_text: String::new(),
            pending_undo: None,
            scroll_to: None,
            longest: None,
            visible: 0..0,
            galley_cache: None,
            // Nothing to hand to the machine until a file is opened or text typed.
            fed: Some(0),
            last_edit: None,
            stream_map: Vec::new(),
            last_cursor: None,
            notice: None,
            confirm: None,
        }
    }

    /// Opens `path` in the editor, replacing its text.
    ///
    /// # Arguments
    /// * `path` - The G-code file.
    ///
    /// # Errors
    /// The file cannot be read; the editor keeps its text.
    pub fn load(&mut self, path: &Path) -> anyhow::Result<()> {
        let buffer = EditorBuffer::load(path)?;
        *self = Self {
            buffer,
            find_query: std::mem::take(&mut self.find_query),
            replace_text: std::mem::take(&mut self.replace_text),
            fed: None,
            scroll_to: Some((0, SCROLL_TRIES)),
            ..Self::new()
        };
        Ok(())
    }

    /// The editor's document.
    pub fn buffer(&self) -> &EditorBuffer {
        &self.buffer
    }

    /// The editor's document, for edits made outside the pane.
    pub fn buffer_mut(&mut self) -> &mut EditorBuffer {
        &mut self.buffer
    }

    /// Whether the file is too large for the text field ([`LARGE_FILE_LINES`]).
    pub fn is_large(&self) -> bool {
        self.buffer.line_count() > LARGE_FILE_LINES
    }

    /// The lines (0-based) in view on the last frame.
    pub fn visible_lines(&self) -> Range<usize> {
        self.visible.clone()
    }

    /// Shows `message` under the text until the next file is opened.
    ///
    /// # Arguments
    /// * `message` - The message, translated.
    pub fn set_notice(&mut self, message: String) {
        self.notice = Some(message);
    }

    /// Sets what find looks for.
    ///
    /// # Arguments
    /// * `query` - The text to find, ASCII case ignored.
    pub fn set_find_query(&mut self, query: &str) {
        query.clone_into(&mut self.find_query);
        self.find_match = None;
    }

    /// Moves to the next match of the find query, wrapping around.
    pub fn find_next(&mut self) {
        let from = self.find_match.as_ref().map_or(0, |m| m.end);
        let found = find::find_next(self.buffer.text(), &self.find_query, from);
        self.show_match(found);
    }

    /// Moves to the previous match of the find query, wrapping around.
    pub fn find_previous(&mut self) {
        let before = self
            .find_match
            .as_ref()
            .map_or(self.buffer.text().len(), |m| m.start);
        let found = find::find_previous(self.buffer.text(), &self.find_query, before);
        self.show_match(found);
    }

    /// Sets what Replace and Replace All put in place of a match.
    ///
    /// # Arguments
    /// * `text` - The replacement.
    pub fn set_replace_text(&mut self, text: &str) {
        text.clone_into(&mut self.replace_text);
    }

    /// Whether the text may be changed: no job runs and no large file is shown read-only.
    fn editable(&self) -> bool {
        !self.locked && (self.edit_large || !self.is_large())
    }

    /// Whether a replace is refused; while a job runs the notice says why.
    fn replace_refused(&mut self, tr: &Translator) -> bool {
        if self.locked {
            self.notice = Some(tr.tr(READ_ONLY_DURING_JOB).into_owned());
        }
        !self.editable() || self.find_query.is_empty() || self.replace_text.is_empty()
    }

    /// Replace: the current match becomes the replacement and the next match is selected;
    /// with no current match the next one is selected. Refused while a job runs.
    ///
    /// # Arguments
    /// * `tr` - Translator for the refusal.
    pub fn replace(&mut self, tr: &Translator) {
        if self.replace_refused(tr) {
            return;
        }
        let text = self.buffer.text();
        let current = self.find_match.clone().filter(|range| {
            text.get(range.clone())
                .is_some_and(|m| m.eq_ignore_ascii_case(&self.find_query))
        });
        let Some(range) = current else {
            self.find_next();
            return;
        };
        let mut new = String::with_capacity(text.len());
        new.push_str(&text[..range.start]);
        new.push_str(&self.replace_text);
        new.push_str(&text[range.end..]);
        let after = range.start + self.replace_text.len();
        self.set_replaced(new);
        let found = find::find_next(self.buffer.text(), &self.find_query, after);
        self.show_match(found);
    }

    /// Replace All: every match becomes the replacement, as one edit and one undo step.
    /// Refused while a job runs.
    ///
    /// # Arguments
    /// * `tr` - Translator for the refusal.
    pub fn replace_all(&mut self, tr: &Translator) {
        if self.replace_refused(tr) {
            return;
        }
        let (new, replaced) =
            find::replace_all(self.buffer.text(), &self.find_query, &self.replace_text);
        if replaced > 0 {
            self.set_replaced(new);
            self.find_match = None;
            self.select_match = false;
        }
    }

    /// Puts `text` in place of the old as an edit, keeping the old for the undo point.
    fn set_replaced(&mut self, text: String) {
        let old = std::mem::replace(self.buffer.text_mut(), text);
        self.buffer.mark_edited();
        // An earlier replace not yet drawn keeps its own text as the undo point.
        self.pending_undo.get_or_insert(old);
        self.last_edit = Some(Instant::now());
    }

    fn show_match(&mut self, found: Option<Range<usize>>) {
        if let Some(range) = &found {
            let line = self.buffer.line_of_byte(range.start);
            self.buffer.set_current_line(line);
            self.scroll_to = Some((line, SCROLL_TRIES));
        }
        self.select_match = found.is_some();
        self.find_match = found;
    }

    /// Whether the text changed since it was last handed to the machine.
    pub fn program_stale(&self) -> bool {
        self.fed != Some(self.buffer.generation())
    }

    /// Whether typing has paused for [`PROGRAM_SETTLE`], so the text can be handed to the
    /// machine without re-preparing it on every keystroke.
    ///
    /// # Arguments
    /// * `now` - The current time.
    pub fn edit_settled(&self, now: Instant) -> bool {
        self.last_edit
            .is_none_or(|edited| now.duration_since(edited) >= PROGRAM_SETTLE)
    }

    /// Records that the text was handed to the machine as the program to stream, so the
    /// streamed lines can be followed in the editor.
    pub fn program_fed(&mut self) {
        self.fed = Some(self.buffer.generation());
        self.stream_map = buffer::stream_line_map(self.buffer.text());
        self.notice = None;
    }

    /// The notice shown under the text, if any.
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// Clears the notice under the text (after a successful file operation).
    pub fn clear_notice(&mut self) {
        self.notice = None;
    }

    /// Whether the text is read-only because a job was streaming when the pane was last
    /// drawn.
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Records that the machine refused the text as a program, with `notice` shown.
    ///
    /// # Arguments
    /// * `notice` - Why, translated.
    pub fn program_rejected(&mut self, notice: String) {
        self.fed = Some(self.buffer.generation());
        self.stream_map.clear();
        self.notice = Some(notice);
    }

    /// Clear editor: refused while a job runs; with only whitespace (or nothing) the text is
    /// cleared at once, otherwise the app is asked to confirm first.
    ///
    /// # Arguments
    /// * `tr` - Translator for the refusal.
    pub fn request_clear(&mut self, tr: &Translator) {
        if self.locked {
            self.notice = Some(tr.tr(READ_ONLY_DURING_JOB).into_owned());
            return;
        }
        if self.buffer.text().trim().is_empty() {
            if !self.buffer.text().is_empty() {
                self.clear();
            }
            return;
        }
        self.confirm = Some(ConfirmRequest::ClearEditor);
    }

    /// Removes all text: an edit, so the file is unsaved and the program is handed over again.
    fn clear(&mut self) {
        self.buffer.set_text(String::new());
        self.find_match = None;
        self.select_match = false;
        self.last_edit = Some(Instant::now());
        self.scroll_to = Some((0, SCROLL_TRIES));
    }

    /// Follows the streaming job: the line in progress becomes the current line.
    fn follow_stream(&mut self, acked: usize, total: usize) {
        // Only while the streamed program is the one this editor handed over.
        if total == 0 || total != self.stream_map.len() {
            return;
        }
        let line = self.stream_map[acked.min(total - 1)];
        if line != self.buffer.current_line() {
            self.buffer.set_current_line(line);
            self.scroll_to = Some((line, SCROLL_TRIES));
        }
    }

    fn style(ui: &egui::Ui) -> HighlightStyle {
        HighlightStyle::new(
            FontId::monospace(FONT_SIZE),
            crate::theme::palette(ui.ctx().theme()).syntax,
        )
    }

    /// The find row: query, previous and next (chevron icon buttons named "Previous Match"
    /// and "Next Match"), match count, then the Clear editor and Save G-code buttons. The
    /// query takes the room the rest leaves, so the row fits a narrow editor column.
    ///
    /// # Returns
    /// Whether Save G-code was clicked (File > Save for the editor).
    fn show_find(&mut self, ui: &mut egui::Ui, tr: &Translator) -> bool {
        let mut save = false;
        ui.horizontal(|ui| {
            let hint = tr.tr("Search...");
            // Four icon buttons and the match count.
            let rest = 4.0 * 36.0 + 110.0;
            let edit = egui::TextEdit::singleline(&mut self.find_query)
                .hint_text(hint.as_ref())
                .desired_width((ui.available_width() - rest).clamp(80.0, 200.0));
            let response = ui.add(edit);
            response.widget_info(|| {
                let mut info = egui::WidgetInfo::text_edit(
                    true,
                    &self.find_query,
                    &self.find_query,
                    hint.as_ref(),
                );
                info.label = Some(hint.clone().into_owned());
                info
            });
            if response.changed() {
                self.find_match = None;
            }
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if icon_only_button(ui, Icon::JogUp, &tr.tr("Previous Match")).clicked() {
                self.find_previous();
            }
            if icon_only_button(ui, Icon::JogDown, &tr.tr("Next Match")).clicked() || enter {
                self.find_next();
            }
            if enter {
                response.request_focus();
            }
            if !self.find_query.is_empty() {
                let count = self.match_count();
                ui.label(format!("{} {count}", tr.tr("Matches:")));
            }
            let clear = ui
                .add_enabled_ui(!self.locked, |ui| {
                    icon_only_button(ui, Icon::Clear, &tr.tr("Clear editor"))
                })
                .inner;
            if clear.clicked() {
                self.request_clear(tr);
            }
            save = icon_only_button(ui, Icon::Save, &tr.tr("Save G-code")).clicked();
        });
        self.show_replace(ui, tr);
        save
    }

    /// The number of matches of the find query, counted once per query and edit.
    fn match_count(&mut self) -> usize {
        let generation = self.buffer.generation();
        match &self.find_count {
            Some((query, g, n)) if *query == self.find_query && *g == generation => *n,
            _ => {
                let n = find::count(self.buffer.text(), &self.find_query);
                self.find_count = Some((self.find_query.clone(), generation, n));
                n
            }
        }
    }

    /// The replace row: "Replace with ...", Replace and Replace All (icon buttons), enabled
    /// when the query has matches, the replacement is not empty and the text may be changed.
    fn show_replace(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        ui.horizontal(|ui| {
            let hint = tr.tr("Replace with ...");
            let rest = 2.0 * 36.0;
            let edit = egui::TextEdit::singleline(&mut self.replace_text)
                .hint_text(hint.as_ref())
                .desired_width((ui.available_width() - rest).clamp(80.0, 200.0));
            let response = ui.add(edit);
            response.widget_info(|| {
                let mut info = egui::WidgetInfo::text_edit(
                    true,
                    &self.replace_text,
                    &self.replace_text,
                    hint.as_ref(),
                );
                info.label = Some(hint.clone().into_owned());
                info
            });
            let enabled = self.editable()
                && !self.find_query.is_empty()
                && !self.replace_text.is_empty()
                && self.match_count() > 0;
            let (replace, replace_all) = ui
                .add_enabled_ui(enabled, |ui| {
                    (
                        icon_only_button(ui, Icon::Replace, &tr.tr("Replace")).clicked(),
                        icon_only_button(ui, Icon::Exchange, &tr.tr("Replace All")).clicked(),
                    )
                })
                .inner;
            if replace {
                self.replace(tr);
            }
            if replace_all {
                self.replace_all(tr);
            }
        });
    }

    /// The status row: file name, unsaved marker, current line, the large-file switch and
    /// any notice.
    fn show_status(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        ui.horizontal(|ui| {
            let name = self.buffer.path().and_then(Path::file_name).map_or_else(
                || tr.tr("Untitled").into_owned(),
                |n| n.to_string_lossy().into_owned(),
            );
            let dirty = if self.buffer.is_dirty() { " *" } else { "" };
            ui.label(format!("{name}{dirty}"));
            ui.separator();
            ui.label(format!(
                "{} {} / {}",
                tr.tr("Line:"),
                self.buffer.current_line() + 1,
                self.buffer.line_count()
            ));
            if self.locked {
                ui.separator();
                ui.label(tr.tr(READ_ONLY_DURING_JOB));
            }
            if self.is_large() {
                ui.separator();
                ui.label(tr.tr("Large file: shown read-only to stay fast."));
                crate::theme::checkbox(ui, &mut self.edit_large, tr.tr("Edit anyway"));
            }
        });
        if let Some(notice) = &self.notice {
            ui.colored_label(ui.visuals().error_fg_color, notice);
        }
    }
}

impl Pane for EditorPane {
    fn id(&self) -> PaneId {
        PaneId::Editor
    }

    fn title(&self, tr: &Translator) -> String {
        tr.tr(PaneId::Editor.title_msgid()).into_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext) {
        let tr = ctx.tr;
        self.locked = ctx.machine().is_some_and(|m| m.job_running());
        let save = egui::Panel::top("gcodekit5-editor-find")
            .show_separator_line(false)
            .show(ui, |ui| self.show_find(ui, tr))
            .inner;
        // The Save button: File > Save for the editor.
        if save {
            ctx.request(crate::commands::Command::FileSave);
        }
        egui::Panel::bottom("gcodekit5-editor-status")
            .show_separator_line(false)
            .show(ui, |ui| self.show_status(ui, tr));
        let style = Self::style(ui);
        let row_height = ui.fonts_mut(|f| f.row_height(&style.font));
        if self.is_large() && !self.edit_large {
            self.show_virtual(ui, &style, row_height);
        } else {
            let label = self.title(tr);
            self.show_text_edit(ui, &style, row_height, &label);
        }
        self.settle_scroll();
    }

    fn take_confirm(&mut self) -> Option<ConfirmRequest> {
        self.confirm.take()
    }

    fn confirmed(
        &mut self,
        request: ConfirmRequest,
        choice: ConfirmChoice,
        ctx: &mut PaneContext,
    ) -> bool {
        if request != ConfirmRequest::ClearEditor {
            return false;
        }
        if choice == ConfirmChoice::Yes {
            // Checked again: a job may have started while the question was open.
            if ctx.machine().is_some_and(|m| m.job_running()) {
                self.notice = Some(ctx.tr.tr(READ_ONLY_DURING_JOB).into_owned());
            } else {
                self.clear();
            }
        }
        true
    }

    fn machine_events(&mut self, events: &[MachineEvent], _tr: &Translator) {
        for event in events {
            match event {
                MachineEvent::StreamProgress { acked, total, .. } => {
                    self.follow_stream(*acked, *total);
                }
                MachineEvent::Connected
                | MachineEvent::Disconnected
                | MachineEvent::StatusReport(_)
                | MachineEvent::Received(_)
                | MachineEvent::Sent(_)
                | MachineEvent::Error(_)
                | MachineEvent::StreamFinished { .. }
                | MachineEvent::StreamHalted { .. }
                | MachineEvent::ProtocolMismatch { .. }
                | MachineEvent::StreamStopped { .. } => {}
            }
        }
    }
}

/// The editor pane shared between the workspace, which draws it, and the app, which opens
/// and saves its files and hands its text to the machine.
#[derive(Debug, Clone)]
pub struct SharedEditor(pub Rc<RefCell<EditorPane>>);

impl Pane for SharedEditor {
    fn id(&self) -> PaneId {
        PaneId::Editor
    }

    fn title(&self, tr: &Translator) -> String {
        tr.tr(PaneId::Editor.title_msgid()).into_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext) {
        match self.0.try_borrow_mut() {
            Ok(mut pane) => pane.ui(ui, ctx),
            Err(error) => tracing::warn!(%error, "editor busy; not drawn this frame"),
        }
    }

    fn machine_events(&mut self, events: &[MachineEvent], tr: &Translator) {
        match self.0.try_borrow_mut() {
            Ok(mut pane) => pane.machine_events(events, tr),
            Err(error) => tracing::warn!(%error, "editor busy; machine events dropped"),
        }
    }

    fn take_confirm(&mut self) -> Option<ConfirmRequest> {
        self.0
            .try_borrow_mut()
            .ok()
            .and_then(|mut pane| pane.take_confirm())
    }

    fn confirmed(
        &mut self,
        request: ConfirmRequest,
        choice: ConfirmChoice,
        ctx: &mut PaneContext,
    ) -> bool {
        match self.0.try_borrow_mut() {
            Ok(mut pane) => pane.confirmed(request, choice, ctx),
            Err(error) => {
                tracing::warn!(%error, "editor busy; confirmation dropped");
                false
            }
        }
    }
}
