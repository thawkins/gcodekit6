//! The line-number gutter left of the editor's text, and the current-line band behind it.
//!
//! Only the rows in view are painted, so the cost does not grow with the file.
//!
//! Dependencies: `egui`, [`crate::theme`] (colours).

use std::ops::Range;

use egui::{Align2, FontId, Painter, Pos2, Rect, pos2, vec2};

use crate::theme::Syntax;

/// The gutter's width for a text of `line_count` lines: room for the widest number.
///
/// # Arguments
/// * `ui` - For the font's glyph width.
/// * `font` - The gutter's font.
/// * `line_count` - Lines in the text.
pub fn width(ui: &egui::Ui, font: &FontId, line_count: usize) -> f32 {
    let digits = line_count.max(1).ilog10() as usize + 1;
    let glyph = ui.fonts_mut(|f| f.glyph_width(font, '0'));
    glyph * digits.max(3) as f32 + 2.0 * glyph
}

/// Where the gutter paints.
#[derive(Debug, Clone, Copy)]
pub struct Rows {
    /// The gutter's column; its top is line 0's top.
    pub column: Rect,
    /// Height of one line.
    pub row_height: f32,
    /// Full width of the text area, for the current-line band.
    pub band_right: f32,
}

impl Rows {
    /// The top of line `line`.
    fn top(&self, line: usize) -> f32 {
        self.column.top() + line as f32 * self.row_height
    }
}

/// Paints the numbers of `lines` and the band behind `current`.
///
/// # Arguments
/// * `painter` - Paints in the scrolled content.
/// * `rows` - Where the rows are.
/// * `lines` - The lines in view.
/// * `current` - The current line, banded when in view.
/// * `font` - The gutter's font.
/// * `syntax` - Colours.
pub fn paint(
    painter: &Painter,
    rows: &Rows,
    lines: Range<usize>,
    current: usize,
    font: &FontId,
    syntax: &Syntax,
) {
    if lines.contains(&current) {
        let top = rows.top(current);
        let band = Rect::from_min_max(
            pos2(rows.column.left(), top),
            pos2(rows.band_right, top + rows.row_height),
        );
        painter.rect_filled(band, 0.0, syntax.current_line_bg);
    }
    let right = rows.column.right() - font.size * 0.5;
    for line in lines {
        let pos = Pos2::new(right, rows.top(line));
        painter.text(
            pos,
            Align2::RIGHT_TOP,
            (line + 1).to_string(),
            font.clone(),
            syntax.gutter,
        );
    }
}

/// The lines whose rows meet the vertical span `visible` of content that starts at
/// `content_top`, clamped to `line_count`.
///
/// # Arguments
/// * `visible` - The visible part of the content, in the content's coordinates.
/// * `content_top` - Where line 0 starts.
/// * `row_height` - Height of one line.
/// * `line_count` - Lines in the text.
pub fn visible_lines(
    visible: Rect,
    content_top: f32,
    row_height: f32,
    line_count: usize,
) -> Range<usize> {
    let row = |y: f32| ((y - content_top) / row_height).max(0.0) as usize;
    let first = row(visible.top()).min(line_count);
    let last = (row(visible.bottom()) + 1).min(line_count);
    first..last
}

/// The size a gutter column of `width` takes for `line_count` lines.
///
/// # Arguments
/// * `width` - From [`width`].
/// * `row_height` - Height of one line.
/// * `line_count` - Lines in the text.
pub fn size(width: f32, row_height: f32, line_count: usize) -> egui::Vec2 {
    vec2(width, row_height * line_count as f32)
}
