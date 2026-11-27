//! G-Code command type and sequence numbering

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// A G-Code line with its sequence number
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcodeCommand {
    /// G-Code line (e.g., "G00 X10.5 Y20.3 Z0.0")
    pub line: String,
    /// Command numbering (0-based, sequential)
    pub sequence_number: u32,
}

impl GcodeCommand {
    /// Create a new G-Code command with sequence number 0
    pub fn new(line: impl Into<String>) -> Self {
        Self {
            line: line.into(),
            sequence_number: 0,
        }
    }

    /// Create a new G-Code command with sequence number
    pub fn with_sequence(line: impl Into<String>, sequence: u32) -> Self {
        let mut cmd = Self::new(line);
        cmd.sequence_number = sequence;
        cmd
    }
}

impl Default for GcodeCommand {
    fn default() -> Self {
        Self::new("")
    }
}

/// Command numbering generator for sequential tracking
#[derive(Clone)]
pub struct CommandNumberGenerator {
    counter: Arc<AtomicU32>,
}

impl CommandNumberGenerator {
    /// Create a new command number generator
    pub fn new() -> Self {
        Self {
            counter: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Get the next command number
    pub fn next(&self) -> u32 {
        self.counter.fetch_add(1, Ordering::SeqCst)
    }

    /// Get current command count without incrementing
    pub fn current(&self) -> u32 {
        self.counter.load(Ordering::SeqCst)
    }

    /// Reset the counter
    pub fn reset(&self) {
        self.counter.store(0, Ordering::SeqCst);
    }
}

impl Default for CommandNumberGenerator {
    fn default() -> Self {
        Self::new()
    }
}
