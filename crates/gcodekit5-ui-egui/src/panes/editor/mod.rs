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
//! Ported from the GTK `ui/gtk/editor.rs` (GtkSourceView). Not ported: replace and replace
//! all, and the line-counter tooltip; the GTK find box's match index ("2 of 5") is shown as
//! the match count only.
//!
//! Dependencies: `egui`, `anyhow`, `gcodekit5-app` (machine events, translations).

pub mod buffer;
pub mod find;
pub mod gutter;
pub mod highlight;

use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::text::{CCursor, CCursorRange, LayoutJob};
use egui::{FontId, Galley, TextBuffer};
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::MachineEvent;

pub use buffer::EditorBuffer;
use highlight::HighlightStyle;

use crate::workspace::{Pane, PaneContext, PaneId};

/// Files with more lines than this open read-only in the virtualised view.
pub const LARGE_FILE_LINES: usize = 50_000;

/// How long after the last keystroke the edited text is handed to the machine.
pub const PROGRAM_SETTLE: Duration = Duration::from_millis(300);

/// Frames a scroll request is retried.
const SCROLL_TRIES: u8 = 2;

/// Font size of the editor's text.
const FONT_SIZE: f32 = 13.0;

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
        n_!("Could not save file"),
        n_!("Cannot stream this program"),
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
    find_query: String,
    /// Byte range of the last find match.
    find_match: Option<Range<usize>>,
    /// The last find match is still to be selected in the text field.
    select_match: bool,
    /// Match count of `(query, generation)`, so it is not recounted every frame.
    find_count: Option<(String, u64, usize)>,
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
            find_query: String::new(),
            find_match: None,
            select_match: false,
            find_count: None,
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

    /// The find row: query, previous and next, match count.
    fn show_find(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        ui.horizontal(|ui| {
            let hint = tr.tr("Search...");
            let edit = egui::TextEdit::singleline(&mut self.find_query)
                .hint_text(hint.as_ref())
                .desired_width(200.0);
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
            if ui.button(tr.tr("Previous Match")).clicked() {
                self.find_previous();
            }
            if ui.button(tr.tr("Next Match")).clicked() || enter {
                self.find_next();
            }
            if enter {
                response.request_focus();
            }
            if !self.find_query.is_empty() {
                let generation = self.buffer.generation();
                let count = match &self.find_count {
                    Some((query, g, n)) if *query == self.find_query && *g == generation => *n,
                    _ => {
                        let n = find::count(self.buffer.text(), &self.find_query);
                        self.find_count = Some((self.find_query.clone(), generation, n));
                        n
                    }
                };
                ui.label(format!("{} {count}", tr.tr("Matches:")));
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
            if self.is_large() {
                ui.separator();
                ui.label(tr.tr("Large file: shown read-only to stay fast."));
                ui.checkbox(&mut self.edit_large, tr.tr("Edit anyway"));
            }
        });
        if let Some(notice) = &self.notice {
            ui.colored_label(ui.visuals().error_fg_color, notice);
        }
    }

    /// The scroll area both views share, scrolled to a requested line.
    fn scroll_area(&mut self, ui: &egui::Ui, row_height: f32) -> egui::ScrollArea {
        let mut area = egui::ScrollArea::both()
            .id_salt("gcodekit5-editor-text")
            .auto_shrink([false, false]);
        if let Some((line, tries)) = self.scroll_to {
            self.scroll_to = tries.checked_sub(1).map(|left| (line, left));
            let centre = line as f32 * row_height - ui.available_height() / 2.0;
            area = area.vertical_scroll_offset(centre.max(0.0));
        }
        area
    }

    /// The read-only view: only the rows in view are highlighted and painted.
    fn show_virtual(&mut self, ui: &mut egui::Ui, style: &HighlightStyle, row_height: f32) {
        let lines = self.buffer.line_count();
        let gutter_width = gutter::width(ui, &style.font, lines);
        let glyph = ui.fonts_mut(|f| f.glyph_width(&style.font, '0'));
        let longest = self.longest_line();
        let area = self.scroll_area(ui, row_height);
        let buffer = &self.buffer;
        let visible = area.show_viewport(ui, |ui, viewport| {
            let content = ui.max_rect();
            let size = gutter::size(
                gutter_width + glyph * (longest + 2) as f32,
                row_height,
                lines,
            );
            ui.allocate_rect(
                egui::Rect::from_min_size(content.min, size),
                egui::Sense::hover(),
            );
            let rows = gutter::Rows {
                column: egui::Rect::from_min_size(
                    content.min,
                    gutter::size(gutter_width, row_height, lines),
                ),
                row_height,
                band_right: content.left() + size.x.max(ui.clip_rect().width()),
            };
            let shown = viewport.translate(content.min.to_vec2());
            let range = gutter::visible_lines(shown, content.top(), row_height, lines);
            let painter = ui.painter();
            gutter::paint(
                painter,
                &rows,
                range.clone(),
                buffer.current_line(),
                &style.font,
                &style.syntax,
            );
            for line in range.clone() {
                let job = highlight::highlight(buffer.line(line), style);
                let galley = ui.fonts_mut(|f| f.layout_job(job));
                let pos = egui::pos2(
                    content.left() + gutter_width,
                    content.top() + line as f32 * row_height,
                );
                painter.galley(pos, galley, style.syntax.plain);
            }
            range
        });
        self.visible = visible.inner;
    }

    /// The longest line, in characters (bytes, for ASCII G-code); counted once per edit.
    fn longest_line(&mut self) -> usize {
        let generation = self.buffer.generation();
        match self.longest {
            Some((g, longest)) if g == generation => longest,
            _ => {
                let longest = (0..self.buffer.line_count())
                    .map(|i| self.buffer.line(i).len())
                    .max()
                    .unwrap_or(0);
                self.longest = Some((generation, longest));
                longest
            }
        }
    }

    /// Drops a scroll request once its line is in view.
    fn settle_scroll(&mut self) {
        if self
            .scroll_to
            .is_some_and(|(line, _)| self.visible.contains(&line))
        {
            self.scroll_to = None;
        }
    }

    /// The editable view: a text field highlighted through a cached layouter.
    ///
    /// # Arguments
    /// * `label` - The text field's accessible name.
    fn show_text_edit(
        &mut self,
        ui: &mut egui::Ui,
        style: &HighlightStyle,
        row_height: f32,
        label: &str,
    ) {
        let lines = self.buffer.line_count();
        let gutter_width = gutter::width(ui, &style.font, lines);
        let text_id = ui.make_persistent_id("gcodekit5-editor-text-edit");
        self.apply_find_selection(ui.ctx(), text_id);
        let area = self.scroll_area(ui, row_height);
        let Self {
            buffer,
            galley_cache,
            ..
        } = self;
        let current = buffer.current_line();
        let output = area.show_viewport(ui, |ui, viewport| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            ui.horizontal_top(|ui| {
                let content = ui.max_rect();
                let (column, _) = ui.allocate_exact_size(
                    gutter::size(gutter_width, row_height, lines),
                    egui::Sense::hover(),
                );
                let rows = gutter::Rows {
                    column,
                    row_height,
                    band_right: ui.clip_rect().right(),
                };
                let shown = viewport.translate(content.min.to_vec2());
                let range = gutter::visible_lines(shown, column.top(), row_height, lines);
                gutter::paint(
                    ui.painter(),
                    &rows,
                    range.clone(),
                    current,
                    &style.font,
                    &style.syntax,
                );
                let mut layouter = |ui: &egui::Ui, text: &dyn TextBuffer, _wrap: f32| {
                    cached_layout(ui, galley_cache, text.as_str(), style)
                };
                let edit = egui::TextEdit::multiline(buffer.text_mut())
                    .id(text_id)
                    .font(style.font.clone())
                    .frame(egui::Frame::NONE)
                    .margin(egui::Margin::ZERO)
                    .desired_width(f32::INFINITY)
                    .lock_focus(true)
                    .layouter(&mut layouter)
                    .show(ui);
                let response = edit.response.response;
                // Named after the pane; egui leaves a text field unnamed.
                response.widget_info(|| {
                    let text = buffer.text();
                    let mut info = egui::WidgetInfo::text_edit(true, text, text, "");
                    info.label = Some(label.to_string());
                    info
                });
                (response.changed(), edit.cursor_range, range)
            })
            .inner
        });
        let (changed, cursor, range) = output.inner;
        self.visible = range;
        if changed {
            self.buffer.mark_edited();
            self.last_edit = Some(Instant::now());
            ui.ctx().request_repaint_after(PROGRAM_SETTLE);
        }
        let cursor = cursor.map(|c| c.primary.index.0);
        if cursor.is_some() && cursor != self.last_cursor {
            self.last_cursor = cursor;
            if let Some(index) = cursor {
                let byte = char_to_byte(self.buffer.text(), index);
                let line = self.buffer.line_of_byte(byte);
                self.buffer.set_current_line(line);
            }
        }
    }

    /// Selects the find match in the text field, once, when one was just found.
    fn apply_find_selection(&mut self, ctx: &egui::Context, id: egui::Id) {
        let Some(range) = self.find_match.clone().filter(|_| self.select_match) else {
            return;
        };
        self.select_match = false;
        let text = self.buffer.text();
        let start = text[..range.start.min(text.len())].chars().count();
        let end = start + text[range.clone()].chars().count();
        let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
        let selection = CCursorRange::two(CCursor::new(start), CCursor::new(end));
        state.cursor.set_char_range(Some(selection));
        state.store(ctx, id);
        self.last_cursor = Some(end);
    }
}

/// The char index `index` of `text` as a byte offset.
fn char_to_byte(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map_or(text.len(), |(i, _)| i)
}

/// The highlighted layout of the whole `text`, from `cache` while text and style are
/// unchanged.
fn cached_layout(
    ui: &egui::Ui,
    cache: &mut Option<GalleyCache>,
    text: &str,
    style: &HighlightStyle,
) -> Arc<Galley> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    let hash = hasher.finish();
    if let Some(cached) = cache
        && cached.hash == hash
        && cached.style == *style
    {
        return Arc::clone(&cached.galley);
    }
    let mut job = LayoutJob::default();
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            job.append(
                "\n",
                0.0,
                egui::TextFormat::simple(style.font.clone(), style.syntax.plain),
            );
        }
        highlight::append_line(&mut job, line, style);
    }
    job.wrap.max_width = f32::INFINITY;
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    *cache = Some(GalleyCache {
        hash,
        style: style.clone(),
        galley: Arc::clone(&galley),
    });
    galley
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
        egui::Panel::top("gcodekit5-editor-find")
            .show_separator_line(false)
            .show(ui, |ui| self.show_find(ui, tr));
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
}
