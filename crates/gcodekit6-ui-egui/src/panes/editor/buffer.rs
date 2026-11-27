//! The editor's document: the G-code text, the file it belongs to, whether it has unsaved
//! changes, its line index and the current line.
//!
//! The line index (the byte offset where each line starts) is rebuilt once per edit, not per
//! frame, so the large-file view can find any line in O(1).
//!
//! [`stream_line_map`] maps the lines the machine streams back to editor lines: streaming
//! drops comments and empty lines (`prepare_program`), so streamed line `n` is not editor
//! line `n`.
//!
//! Dependencies: `anyhow` (I/O errors with the path), `gcodekit6-app` (program preparation).

use std::path::{Path, PathBuf};

use anyhow::Context;
use gcodekit6_app::machine::grbl_stream::prepare_program;

/// The G-code text being edited and the file it belongs to.
#[derive(Debug, Clone)]
pub struct EditorBuffer {
    text: String,
    path: Option<PathBuf>,
    dirty: bool,
    /// Byte offset of the start of every line; never empty.
    line_starts: Vec<usize>,
    /// The current line (0-based): the cursor's, a find match's or the streaming job's.
    current_line: usize,
    /// Bumped on every change of the text.
    generation: u64,
    /// The text was replaced as a whole (a file opened, the editor cleared, a program
    /// generated): the text field's undo history belongs to the old text and must go.
    new_content: bool,
}

impl Default for EditorBuffer {
    fn default() -> Self {
        Self {
            text: String::new(),
            path: None,
            dirty: false,
            line_starts: vec![0],
            current_line: 0,
            generation: 0,
            new_content: false,
        }
    }
}

/// The byte offset where each line of `text` starts.
fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect()
}

impl EditorBuffer {
    /// Reads `path` into a clean buffer that saves back to it.
    ///
    /// # Arguments
    /// * `path` - The G-code file.
    ///
    /// # Errors
    /// The file cannot be read or is not UTF-8.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        Ok(Self {
            line_starts: line_starts(&text),
            text,
            path: Some(path.to_path_buf()),
            new_content: true,
            ..Self::default()
        })
    }

    /// Writes the text to the buffer's file and marks it clean.
    ///
    /// # Errors
    /// The buffer has no file yet, or the file cannot be written.
    pub fn save(&mut self) -> anyhow::Result<()> {
        let path = self
            .path
            .clone()
            .context("the G-code has no file to save to")?;
        self.save_as(&path)
    }

    /// Writes the text to `path`, which becomes the buffer's file, and marks it clean.
    ///
    /// # Arguments
    /// * `path` - Where to save.
    ///
    /// # Errors
    /// The file cannot be written.
    pub fn save_as(&mut self, path: &Path) -> anyhow::Result<()> {
        crate::files::write_atomic(path, &self.text)
            .with_context(|| format!("cannot write {}", path.display()))?;
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(())
    }

    /// The G-code text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the text as an edit: the buffer becomes dirty, and the text field starts a
    /// new undo history.
    ///
    /// # Arguments
    /// * `text` - The new text.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.new_content = true;
        self.mark_edited();
    }

    /// Replaces the text with a program generated elsewhere (the designer's Generate G-Code
    /// or Frame): the buffer becomes dirty and forgets its file, so the next Save asks where
    /// instead of overwriting the file that was open.
    ///
    /// # Arguments
    /// * `text` - The generated program.
    pub fn replace_with_generated(&mut self, text: String) {
        self.text = text;
        self.path = None;
        self.new_content = true;
        self.mark_edited();
    }

    /// Whether the text was replaced as a whole since the last call (see
    /// [`EditorBuffer::set_text`]); the flag is cleared.
    pub(crate) fn take_new_content(&mut self) -> bool {
        std::mem::take(&mut self.new_content)
    }

    /// The text for a widget to edit in place; call [`EditorBuffer::mark_edited`] after it
    /// changed.
    pub(crate) fn text_mut(&mut self) -> &mut String {
        &mut self.text
    }

    /// Records an edit made through [`EditorBuffer::text_mut`]: rebuilds the line index and
    /// marks the buffer dirty.
    pub(crate) fn mark_edited(&mut self) {
        self.line_starts = line_starts(&self.text);
        self.current_line = self.current_line.min(self.line_count() - 1);
        self.dirty = true;
        self.generation += 1;
    }

    /// The file the buffer saves to, if it has one.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Whether the text changed since it was loaded or saved.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// The current line, 0-based.
    pub fn current_line(&self) -> usize {
        self.current_line
    }

    /// Makes `line` (0-based, clamped to the text) the current line.
    ///
    /// # Arguments
    /// * `line` - The line.
    pub fn set_current_line(&mut self, line: usize) {
        self.current_line = line.min(self.line_count() - 1);
    }

    /// Counts the text's lines; a trailing newline starts an empty last line, as in a text
    /// view.
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Line `index` (0-based) without its line ending; empty past the end.
    ///
    /// # Arguments
    /// * `index` - The line.
    pub fn line(&self, index: usize) -> &str {
        let Some(&start) = self.line_starts.get(index) else {
            return "";
        };
        let end = self
            .line_starts
            .get(index + 1)
            .map_or(self.text.len(), |next| next - 1);
        self.text[start..end].trim_end_matches('\r')
    }

    /// The line holding byte offset `byte`.
    ///
    /// # Arguments
    /// * `byte` - A byte offset into the text.
    pub fn line_of_byte(&self, byte: usize) -> usize {
        self.line_starts.partition_point(|&start| start <= byte) - 1
    }

    /// The byte offset where `line` (0-based, clamped to the text) starts.
    ///
    /// # Arguments
    /// * `line` - The line.
    pub fn line_start(&self, line: usize) -> usize {
        self.line_starts
            .get(line)
            .copied()
            .unwrap_or_else(|| self.line_starts.last().copied().unwrap_or(0))
    }

    /// Bumped on every edit; tells whether the text changed since a value was taken.
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// For each line the machine streams from `text` (in order), the editor line (0-based) it
/// came from. Streaming drops comments and lines left empty, as `prepare_program` does.
///
/// # Arguments
/// * `text` - The program text.
pub fn stream_line_map(text: &str) -> Vec<usize> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                false
            } else if trimmed.contains(['(', ';']) {
                // The comment rules live in `prepare_program`; ask it about this one line.
                prepare_program(line, false, 0).is_ok_and(|kept| !kept.is_empty())
            } else {
                true
            }
        })
        .map(|(index, _)| index)
        .collect()
}
