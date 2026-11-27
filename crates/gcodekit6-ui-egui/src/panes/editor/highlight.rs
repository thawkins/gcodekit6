//! G-code syntax highlighting: one line becomes a `LayoutJob` whose sections are coloured by
//! class (G/M words, coordinates, line numbers, comments, the rest) from [`crate::theme`].
//!
//! Highlighting is per line, so
//! the large-file view highlights only the rows in view.
//!
//! Dependencies: `egui` (layout jobs), [`crate::theme`] (colours).

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId};

use crate::theme::Syntax;

/// The font and colours lines are highlighted with.
#[derive(Debug, Clone, PartialEq)]
pub struct HighlightStyle {
    /// The (monospace) font of the text.
    pub font: FontId,
    /// The colour of each class.
    pub syntax: Syntax,
}

impl HighlightStyle {
    /// A style drawing in `font` with the colours `syntax`.
    ///
    /// # Arguments
    /// * `font` - The font of the text.
    /// * `syntax` - The colour of each class.
    pub fn new(font: FontId, syntax: Syntax) -> Self {
        Self { font, syntax }
    }
}

/// What a run of text is, for its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Command,
    Coordinate,
    LineNumber,
    Comment,
    Plain,
}

impl Class {
    fn colour(self, syntax: &Syntax) -> Color32 {
        match self {
            Class::Command => syntax.command,
            Class::Coordinate => syntax.coordinate,
            Class::LineNumber => syntax.line_number,
            Class::Comment => syntax.comment,
            Class::Plain => syntax.plain,
        }
    }

    /// The class of a word starting with `letter`.
    fn of_word(letter: u8) -> Self {
        match letter.to_ascii_uppercase() {
            b'G' | b'M' => Class::Command,
            b'X' | b'Y' | b'Z' | b'A' | b'B' | b'C' | b'I' | b'J' | b'K' | b'R' => {
                Class::Coordinate
            }
            b'N' => Class::LineNumber,
            _ => Class::Plain,
        }
    }
}

/// The byte length of the token at the start of `rest` and its class.
fn token(rest: &str) -> (usize, Class) {
    let bytes = rest.as_bytes();
    match bytes[0] {
        b';' => (rest.len(), Class::Comment),
        b'(' => (
            rest.find(')').map_or(rest.len(), |end| end + 1),
            Class::Comment,
        ),
        letter if letter.is_ascii_alphabetic() => {
            let value = bytes[1..]
                .iter()
                .take_while(|b| b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'+'))
                .count();
            let class = if value > 0 {
                Class::of_word(letter)
            } else {
                Class::Plain
            };
            (1 + value, class)
        }
        _ => {
            // Anything else (spaces, stray symbols, non-ASCII) up to the next token start.
            let end = rest
                .char_indices()
                .skip(1)
                .find(|(_, c)| c.is_ascii_alphabetic() || matches!(c, ';' | '('))
                .map_or(rest.len(), |(i, _)| i);
            (end, Class::Plain)
        }
    }
}

/// Appends `line` to `job`, highlighted; adjacent runs of one class share a section.
///
/// # Arguments
/// * `job` - The job to extend.
/// * `line` - One line of G-code, without its line ending.
/// * `style` - Font and colours.
pub fn append_line(job: &mut LayoutJob, line: &str, style: &HighlightStyle) {
    let mut rest = line;
    let mut run: Option<(usize, Class)> = None;
    while !rest.is_empty() {
        let (len, class) = token(rest);
        let start = line.len() - rest.len();
        match run {
            Some((_, current)) if current == class => {}
            Some((run_start, current)) => {
                push(job, &line[run_start..start], current, style);
                run = Some((start, class));
            }
            None => run = Some((start, class)),
        }
        rest = &rest[len..];
    }
    if let Some((run_start, class)) = run {
        push(job, &line[run_start..], class, style);
    }
}

fn push(job: &mut LayoutJob, text: &str, class: Class, style: &HighlightStyle) {
    job.append(
        text,
        0.0,
        TextFormat::simple(style.font.clone(), class.colour(&style.syntax)),
    );
}

/// `line` as a highlighted layout job.
///
/// # Arguments
/// * `line` - One line of G-code, without its line ending.
/// * `style` - Font and colours.
///
/// # Returns
/// A job with one section per run of a class.
pub fn highlight(line: &str, style: &HighlightStyle) -> LayoutJob {
    let mut job = LayoutJob::default();
    append_line(&mut job, line, style);
    job
}
