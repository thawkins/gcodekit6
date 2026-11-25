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
    replace_with(path, |temp| {
        // The handle that wrote the data is the one synced: Windows' FlushFileBuffers
        // (behind `sync_all`) needs a handle with write access.
        let mut file = std::fs::File::create(temp)?;
        std::io::Write::write_all(&mut file, text.as_bytes())?;
        file.sync_all()
    })
}

/// Writes `path` through `write`, which writes a temporary file beside it, then flushes the
/// temporary file to disk and renames it over `path`. A read-only target is refused (the rename would replace it,
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
    replace_with(path, |temp| {
        write(temp)?;
        // `write` wrote through a handle of its own, so the file is reopened for flushing
        // with write access: Windows' FlushFileBuffers (behind `sync_all`) refuses a
        // read-only handle.
        std::fs::OpenOptions::new()
            .write(true)
            .open(temp)?
            .sync_all()
    })
}

/// Refuses a read-only target, runs `write_and_sync` on a temporary path beside `path`
/// (it must leave the whole contents flushed to disk), keeps the target's permissions and
/// renames the temporary file over `path`; the temporary file is removed when anything
/// fails. The parent folder is not synced, so after a power loss the rename itself may be
/// lost, leaving the previous complete file.
fn replace_with(
    path: &Path,
    write_and_sync: impl FnOnce(&Path) -> std::io::Result<()>,
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
    // Flushed before the rename, so a power loss cannot leave the new name on an empty file.
    let result = write_and_sync(&temp)
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
