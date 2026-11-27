//! Error handling for GCodeKit5
//!
//! Provides the unified `Error` and `Result` types used across the application.
//!
//! All error types use `thiserror` for ergonomic error handling.

use thiserror::Error;

/// Main error type for GCodeKit5
///
/// A unified error type that can represent any error from all layers.
/// This is the primary error type used in public APIs.
#[derive(Error, Debug)]
pub enum Error {
    /// Standard I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Create an error from a string message
    pub fn other(msg: impl Into<String>) -> Self {
        Error::Other(msg.into())
    }
}

/// Result type using Error
pub type Result<T> = std::result::Result<T, Error>;
