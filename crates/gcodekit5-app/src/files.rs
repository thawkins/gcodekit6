//! Saving files safely: write a temporary file beside the target, flush it to disk, then
//! rename it over the target, so a crash, a power loss or a full disk never leaves a
//! half-written file. Shared by both front ends (the egui editor, designer and CAM presets,
//! and the tool and material library back ends).
//!
//! Dependencies: `std` only.

use std::path::Path;

/// Writes `text` to `path` atomically ([`write_atomic_with`]).
///
/// # Arguments
/// * `path` - The file to write.
/// * `text` - Its new contents.
///
/// # Errors
/// The target is read-only, or the temporary file cannot be written or renamed.
pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    write_atomic_with(path, |temp| std::fs::write(temp, text))
}

/// Writes `path` through `write`, which writes a temporary file beside it, then renames the
/// temporary file over `path`. A read-only target is refused (the rename would replace it,
/// which a plain write refuses); an existing file's permissions are kept; the temporary file
/// is removed when anything fails.
///
/// # Arguments
/// * `path` - The file to write.
/// * `write` - Writes the whole contents to the temporary path it is given.
///
/// # Errors
/// The target is read-only, `write` fails, or the rename fails.
pub fn write_atomic_with(
    path: &Path,
    write: impl FnOnce(&Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    if let Ok(existing) = std::fs::metadata(path)
        && existing.permissions().readonly()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "target file is read-only",
        ));
    }
    let name = path.file_name().map_or_else(
        || "file".to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let temp = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    let result = write(&temp)
        // Flushed before the rename, so a power loss cannot leave the new name on an
        // empty file.
        .and_then(|()| std::fs::File::open(&temp)?.sync_all())
        .and_then(|()| match std::fs::metadata(path) {
            Ok(existing) => std::fs::set_permissions(&temp, existing.permissions()),
            Err(_) => Ok(()),
        })
        .and_then(|()| std::fs::rename(&temp, path));
    if result.is_err() {
        // The write error is the one to report; a leftover temp file is only logged.
        if let Err(error) = std::fs::remove_file(&temp) {
            tracing::debug!(%error, "temporary save file not removed");
        }
    }
    result
}
