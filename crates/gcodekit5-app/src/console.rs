//! Console/Output Panel - Task 73
//!
//! A bounded log of controller responses and command echoes for the console pane.

use std::collections::VecDeque;

/// Console message level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageLevel {
    /// Debug message
    Debug,
    /// Info message
    Info,
    /// Warning message
    Warning,
    /// Error message
    Error,
    /// Success message
    Success,
}

impl std::fmt::Display for MessageLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Debug => write!(f, "DEBUG"),
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARN"),
            Self::Error => write!(f, "ERR"),
            Self::Success => write!(f, "OK"),
        }
    }
}

/// Console message
#[derive(Debug, Clone)]
pub struct ConsoleMessage {
    /// Message level
    pub level: MessageLevel,
    /// Message text
    pub text: String,
    /// Is command echo
    pub is_command: bool,
}

impl ConsoleMessage {
    /// Create new message
    pub fn new(level: MessageLevel, text: impl Into<String>) -> Self {
        Self {
            level,
            text: text.into(),
            is_command: false,
        }
    }

    /// Create command message
    pub fn command(text: impl Into<String>) -> Self {
        let mut msg = Self::new(MessageLevel::Info, text);
        msg.is_command = true;
        msg
    }
}

/// Console/Output panel: a bounded log of console messages.
#[derive(Debug)]
pub struct ConsolePanel {
    /// All messages
    pub messages: VecDeque<ConsoleMessage>,
    /// Maximum messages to keep
    pub max_messages: usize,
}

impl ConsolePanel {
    /// Create new console panel
    pub fn new() -> Self {
        Self {
            messages: VecDeque::new(),
            max_messages: 1000,
        }
    }

    /// Add message to console
    pub fn add_message(&mut self, level: MessageLevel, text: impl Into<String>) {
        self.push(ConsoleMessage::new(level, text));
    }

    /// Add command message
    pub fn add_command(&mut self, command: impl Into<String>) {
        self.push(ConsoleMessage::command(command));
    }

    /// Append `msg`, dropping the oldest message past `max_messages`.
    fn push(&mut self, msg: ConsoleMessage) {
        self.messages.push_back(msg);
        if self.messages.len() > self.max_messages {
            self.messages.pop_front();
        }
    }

    /// Clear all messages
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Get message count (total)
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

impl Default for ConsolePanel {
    fn default() -> Self {
        Self::new()
    }
}
