//! The editor's two views: the editable text field (with its cached highlighted layout and
//! the find selection) and the read-only virtualised view of a large file, sharing one scroll
//! area and the gutter.
//!
//! Dependencies: `egui`, the sibling `buffer`, `gutter` and `highlight` modules.

use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;

use egui::text::{CCursor, CCursorRange, LayoutJob};
use egui::{Galley, TextBuffer};

use super::highlight::{self, HighlightStyle};
use super::{EditorPane, GalleyCache, PROGRAM_SETTLE, gutter};

impl EditorPane {
    /// The scroll area both views share, scrolled to a requested line.
    fn scroll_area(&mut self, ui: &egui::Ui, row_height: f32) -> egui::ScrollArea {
        let mut area = egui::ScrollArea::both()
            .id_salt("gcodekit6-editor-text")
            .auto_shrink([false, false]);
        if let Some((line, tries)) = self.scroll_to {
            self.scroll_to = tries.checked_sub(1).map(|left| (line, left));
            let centre = line as f32 * row_height - ui.available_height() / 2.0;
            area = area.vertical_scroll_offset(centre.max(0.0));
        }
        area
    }

    /// The read-only view: only the rows in view are highlighted and painted.
    pub(super) fn show_virtual(
        &mut self,
        ui: &mut egui::Ui,
        style: &HighlightStyle,
        row_height: f32,
    ) {
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
            let response = ui.allocate_rect(
                egui::Rect::from_min_size(content.min, size),
                egui::Sense::click(),
            );
            // A click in the gutter makes its line current.
            let clicked = response
                .clicked()
                .then(|| response.interact_pointer_pos())
                .flatten()
                .filter(|pos| pos.x < content.left() + gutter_width)
                .map(|pos| line_at(pos.y, content.top(), row_height, lines));
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
            (range, clicked)
        });
        let (range, clicked) = visible.inner;
        self.visible = range;
        if let Some(line) = clicked {
            self.buffer.set_current_line(line);
        }
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
    pub(super) fn settle_scroll(&mut self) {
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
    pub(super) fn show_text_edit(
        &mut self,
        ui: &mut egui::Ui,
        style: &HighlightStyle,
        row_height: f32,
        label: &str,
    ) {
        let lines = self.buffer.line_count();
        let gutter_width = gutter::width(ui, &style.font, lines);
        let text_id = ui.make_persistent_id("gcodekit6-editor-text-edit");
        self.reset_undo_for_new_content(ui.ctx(), text_id);
        self.apply_pending_undo(ui.ctx(), text_id);
        self.apply_find_selection(ui.ctx(), text_id);
        let area = self.scroll_area(ui, row_height);
        let editable = !self.locked;
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
                let (column, gutter_response) = ui.allocate_exact_size(
                    gutter::size(gutter_width, row_height, lines),
                    egui::Sense::click(),
                );
                let gutter_click = gutter_response
                    .clicked()
                    .then(|| gutter_response.interact_pointer_pos())
                    .flatten()
                    .map(|pos| line_at(pos.y, column.top(), row_height, lines));
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
                    .interactive(editable)
                    .layouter(&mut layouter)
                    .show(ui);
                let response = edit.response.response;
                // Named after the pane; egui leaves a text field unnamed. The value is left
                // empty: copying the whole program into the accessibility tree every frame
                // would cost as much as the text.
                response.widget_info(|| {
                    let mut info = egui::WidgetInfo::text_edit(editable, "", "", "");
                    info.label = Some(label.to_string());
                    info
                });
                (response.changed(), edit.cursor_range, range, gutter_click)
            })
            .inner
        });
        let (changed, cursor, range, gutter_click) = output.inner;
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
        if let Some(line) = gutter_click {
            self.select_line(ui.ctx(), text_id, line);
        }
    }

    /// Makes `line` current and puts the text cursor at its start (a gutter click).
    fn select_line(&mut self, ctx: &egui::Context, id: egui::Id, line: usize) {
        self.buffer.set_current_line(line);
        let line = self.buffer.current_line();
        let start = self.buffer.line_start(line);
        let index = self.buffer.text()[..start].chars().count();
        let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(index))));
        state.store(ctx, id);
        self.last_cursor = Some(index);
        ctx.memory_mut(|m| m.request_focus(id));
        ctx.request_repaint();
    }

    /// Drops the text field's undo history when the text was replaced as a whole (a file
    /// opened, the editor cleared, a program generated), so Ctrl+Z cannot bring the old
    /// text back into the new.
    fn reset_undo_for_new_content(&mut self, ctx: &egui::Context, id: egui::Id) {
        if !self.buffer.take_new_content() {
            return;
        }
        if let Some(mut state) = egui::TextEdit::load_state(ctx, id) {
            state.clear_undoer();
            state.store(ctx, id);
        }
    }

    /// Records the text before a replace as an undo point of the text field, so one undo
    /// (Ctrl+Z) brings it back whole; any typing before it becomes its own undo step.
    fn apply_pending_undo(&mut self, ctx: &egui::Context, id: egui::Id) {
        let Some(old) = self.pending_undo.take() else {
            return;
        };
        let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
        let cursor = state.cursor.char_range().unwrap_or_default();
        let mut undoer = state.undoer();
        undoer.add_undo(&(cursor, old));
        state.set_undoer(undoer);
        state.store(ctx, id);
    }

    /// Selects the find match in the text field, once, when one was just found.
    fn apply_find_selection(&mut self, ctx: &egui::Context, id: egui::Id) {
        let Some(range) = self.find_match.clone().filter(|_| self.select_match) else {
            return;
        };
        self.select_match = false;
        let text = self.buffer.text();
        let (Some(before), Some(matched)) = (text.get(..range.start), text.get(range)) else {
            return;
        };
        let start = before.chars().count();
        let end = start + matched.chars().count();
        let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
        let selection = CCursorRange::two(CCursor::new(start), CCursor::new(end));
        state.cursor.set_char_range(Some(selection));
        state.store(ctx, id);
        self.last_cursor = Some(end);
    }
}

/// The line whose row holds screen height `y`, for rows of `row_height` from `top`.
fn line_at(y: f32, top: f32, row_height: f32, lines: usize) -> usize {
    let row = ((y - top) / row_height).max(0.0) as usize;
    row.min(lines.saturating_sub(1))
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
