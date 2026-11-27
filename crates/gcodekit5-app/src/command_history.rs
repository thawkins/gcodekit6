//! # Command History
//!
//! The console's command history: the commands typed in the console entry, newest last,
//! with Up/Down navigation. It is kept across sessions in [`HISTORY_FILE`] (JSON) in the
//! settings folder, written through the shared atomic writer, and bounded to
//! [`MAX_HISTORY`] commands. The file is the legacy build's, so its history carries over.
//!
//! Dependencies: `serde`/`serde_json` (file format), `gcodekit5-core` (atomic write).

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::Path;

/// Most commands kept; older ones are dropped.
pub const MAX_HISTORY: usize = 250;

/// The history's file name in the settings folder (the legacy build's).
pub const HISTORY_FILE: &str = "command_history.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandHistory {
    commands: VecDeque<String>,
    #[serde(skip)]
    current_index: Option<usize>,
    #[serde(skip)]
    temp_input: String,
}

impl Default for CommandHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHistory {
    pub fn new() -> Self {
        Self {
            commands: VecDeque::new(),
            current_index: None,
            temp_input: String::new(),
        }
    }

    /// Number of commands in the history.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// The command at `index` (0 = oldest kept).
    pub fn get(&self, index: usize) -> Option<&str> {
        self.commands.get(index).map(String::as_str)
    }

    pub fn add(&mut self, command: String) {
        if command.trim().is_empty() {
            return;
        }

        // Don't add duplicates of the most recent command
        if let Some(last) = self.commands.back()
            && last == &command
        {
            return;
        }

        self.commands.push_back(command);

        // Trim to max size
        while self.commands.len() > MAX_HISTORY {
            self.commands.pop_front();
        }

        // Reset navigation
        self.current_index = None;
        self.temp_input.clear();
    }

    pub fn previous(&mut self, current_input: &str) -> Option<String> {
        if self.commands.is_empty() {
            return None;
        }

        match self.current_index {
            None => {
                // First time navigating up, save current input
                self.temp_input = current_input.to_string();
                self.current_index = Some(self.commands.len() - 1);
                Some(self.commands[self.commands.len() - 1].clone())
            }
            Some(idx) => {
                if idx > 0 {
                    self.current_index = Some(idx - 1);
                    Some(self.commands[idx - 1].clone())
                } else {
                    // Already at oldest, stay there
                    Some(self.commands[idx].clone())
                }
            }
        }
    }

    // Custom `new()` does not match Default semantics (requires controller ref).
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<String> {
        match self.current_index {
            None => None,
            Some(idx) => {
                if idx < self.commands.len() - 1 {
                    self.current_index = Some(idx + 1);
                    Some(self.commands[idx + 1].clone())
                } else {
                    // Back to current input
                    self.current_index = None;
                    Some(self.temp_input.clone())
                }
            }
        }
    }

    pub fn reset_navigation(&mut self) {
        self.current_index = None;
        self.temp_input.clear();
    }

    /// The history saved in `path`; empty when the file is missing or cannot be parsed. A
    /// file holding more than [`MAX_HISTORY`] commands keeps the newest.
    ///
    /// # Arguments
    /// * `path` - The history file.
    pub fn load_from(path: &Path) -> Self {
        let Ok(contents) = fs::read_to_string(path) else {
            return Self::new();
        };
        match serde_json::from_str::<Self>(&contents) {
            Ok(mut history) => {
                while history.commands.len() > MAX_HISTORY {
                    history.commands.pop_front();
                }
                history
            }
            Err(error) => {
                tracing::warn!(path = %path.display(), %error, "command history unreadable");
                Self::new()
            }
        }
    }

    /// Saves the history to `path` through the shared atomic writer, creating its folder
    /// first when missing.
    ///
    /// # Arguments
    /// * `path` - The history file.
    ///
    /// # Errors
    /// The folder could not be created, or the JSON encoding or the write failed.
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        gcodekit5_core::files::write_atomic(path, &json)
    }
}
