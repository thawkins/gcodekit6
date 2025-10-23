//! Helpers for persistent app data storage
//!
//! Provides platform-appropriate data directory detection and simple
//! JSON read/write helpers for small structured data used by the app.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::io;
use std::path::PathBuf;
use tracing::debug;

/// Return the platform-appropriate persistent data directory for gcodekit6.
///
/// Linux: ~/.local/share/gcodekit6/
/// macOS: ~/Library/Application Support/gcodekit6/
/// Windows: %APPDATA%\gcodekit6\
pub fn data_dir() -> Option<PathBuf> {
    let base = dirs_next::data_dir()?;
    Some(base.join("gcodekit6"))
}

/// Ensure the persistent data directory exists, creating it if necessary.
pub fn ensure_data_dir() -> io::Result<PathBuf> {
    if let Some(dir) = data_dir() {
        fs::create_dir_all(&dir)?;
        Ok(dir)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine platform data directory",
        ))
    }
}

/// Write a serializable object as JSON to a named file in the data dir.
pub fn write_json<T: Serialize>(name: &str, value: &T) -> io::Result<()> {
    let dir = ensure_data_dir()?;
    // Defensive: ensure the directory exists (tests may set XDG_DATA_HOME to a temp dir)
    if !dir.exists() {
        debug!(dir = %dir.display(), "storage::write_json: data dir missing, creating");
        std::fs::create_dir_all(&dir)?;
    }
    debug!(dir = %dir.display(), exists = dir.exists(), "storage::write_json: data dir status");
    let path = dir.join(name);
    let tmp = path.with_extension("tmp");
    let s = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    // Log the paths involved so tests and CI can diagnose failures
    debug!(tmp = %tmp.display(), final = %path.display(), "storage::write_json: writing and renaming");
    fs::write(&tmp, &s)?;
    debug!(tmp = %tmp.display(), final = %path.display(), "storage::write_json: renaming tmp -> final");
    fs::rename(tmp, path)?;
    Ok(())
}

/// Read JSON from a named file in the data dir and deserialize it.
pub fn read_json<T: DeserializeOwned>(name: &str) -> io::Result<T> {
    let dir = ensure_data_dir()?;
    let path = dir.join(name);
    let data = fs::read(&path)?;
    let v = serde_json::from_slice(&data).map_err(io::Error::other)?;
    Ok(v)
}

/// Return the full path for a named file inside the data dir (without creating it).
pub fn file_path(name: &str) -> Option<PathBuf> {
    data_dir().map(|d| d.join(name))
}
