//! One-shot carry-over of the user's folders from the GCodeKit5 name to GCodeKit6.
//!
//! Before the rename the settings, data files, command history and the Windows log lived
//! in `<base>/gcodekit5`; they now live in `<base>/gcodekit6`. On startup
//! [`migrate_renamed_folders`] copies each old folder to its new name when the new one does
//! not exist yet. The old folder is never modified, so a GCodeKit5 build keeps working. The
//! copy goes to `<base>/gcodekit6.migrating` first and is renamed into place, so a failed
//! copy is retried on the next start instead of leaving a half-filled folder.
//!
//! Inside Flatpak the platform folders are per app (`~/.var/app/<app-id>/{config,data}`), so
//! the GCodeKit5 sandbox's folders are not under the new bases. [`flatpak_folders`] names
//! them as extra sources; the manifest grants read-only access to
//! `~/.var/app/com.github.thawkins.gcodekit5` so they can be read.
//!
//! Also resolves the `GCODEKIT6_CONFIG_DIR` override with `GCODEKIT5_CONFIG_DIR` as a
//! fallback ([`config_dir_var`]).
//!
//! Depends on `dirs` for the platform folders, `craft-settings` for the override variable
//! name and `tracing` for the report.

use crate::migrate::{APP_ID, SettingsError};
use craft_settings::paths;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The folder name and override-variable stem used before the rename to GCodeKit6.
pub const OLD_APP_ID: &str = "gcodekit5";

/// The Flatpak app ID used before the rename to GCodeKit6.
pub const OLD_FLATPAK_APP_ID: &str = "com.github.thawkins.gcodekit5";

/// The file Flatpak places at the root of every sandbox.
pub const FLATPAK_INFO: &str = "/.flatpak-info";

/// Suffix of the folder a copy is staged in before it is renamed into place.
const STAGING_SUFFIX: &str = ".migrating";

/// The outcome of carrying one folder over.
#[derive(Debug)]
pub struct FolderMigration {
    /// The pre-rename folder (`<base>/gcodekit5`).
    pub from: PathBuf,
    /// The new folder (`<base>/gcodekit6`).
    pub to: PathBuf,
    /// `Ok(true)` when the folder was copied, `Ok(false)` when there was nothing to do.
    pub result: Result<bool, SettingsError>,
}

impl FolderMigration {
    /// Writes the outcome to the log: copies at info, failures at warn, no-ops not at all.
    pub fn log(&self) {
        match &self.result {
            Ok(true) => tracing::info!(
                from = %self.from.display(),
                to = %self.to.display(),
                "copied the GCodeKit5 folder to its GCodeKit6 name"
            ),
            Ok(false) => {}
            Err(error) => tracing::warn!(
                from = %self.from.display(),
                to = %self.to.display(),
                %error,
                "could not copy the GCodeKit5 folder; starting without it"
            ),
        }
    }
}

/// Reads the settings-folder override variable `name`, falling back to the pre-rename
/// variable: `GCODEKIT6_CONFIG_DIR` unset means `GCODEKIT5_CONFIG_DIR` is used. Any other
/// variable is read as is.
///
/// # Arguments
/// * `name` - the variable craft-settings asks for
/// * `env` - reads one environment variable
///
/// # Returns
/// The value to use, or `None` when neither variable is set.
pub fn config_dir_var(name: &str, env: &dyn Fn(&str) -> Option<OsString>) -> Option<OsString> {
    let value = env(name);
    if value.is_some() || name != paths::env_var_name(APP_ID) {
        return value;
    }
    env(&paths::env_var_name(OLD_APP_ID))
}

/// The base folders whose `gcodekit5` child is carried over: the platform config folder
/// (settings, data files, command history), the local data folder (the Windows log) and,
/// without a config folder, the home folder (the tool library's fallback).
///
/// # Returns
/// The bases, or none when a settings-folder override variable is set: the user chose
/// where the settings live, and an empty value means nothing may be read or written.
pub fn platform_bases() -> Vec<PathBuf> {
    let env = |name: &str| std::env::var_os(name);
    if config_dir_var(&paths::env_var_name(APP_ID), &env).is_some() {
        return Vec::new();
    }
    let config = dirs::config_dir();
    let home = if config.is_none() {
        dirs::home_dir()
    } else {
        None
    };
    let mut bases: Vec<PathBuf> = Vec::new();
    for base in [config, dirs::data_local_dir(), home].into_iter().flatten() {
        if !bases.contains(&base) {
            bases.push(base);
        }
    }
    bases
}

/// The pre-rename Flatpak sandbox's folders and where each is copied to: its config and data
/// `gcodekit5` folders go to `gcodekit6` under the current config and data bases.
///
/// # Arguments
/// * `env` - reads one environment variable (`FLATPAK_ID`, `HOME`, `XDG_CONFIG_HOME`,
///   `XDG_DATA_HOME` and the settings-folder override)
/// * `flatpak_info` - the sandbox marker file; its presence also means running in Flatpak
///
/// # Returns
/// `(from, to)` pairs, or none outside Flatpak, without a home folder, or when a
/// settings-folder override variable is set.
pub fn flatpak_folders(
    env: &dyn Fn(&str) -> Option<OsString>,
    flatpak_info: &Path,
) -> Vec<(PathBuf, PathBuf)> {
    let in_flatpak = env("FLATPAK_ID").is_some() || flatpak_info.exists();
    if !in_flatpak || config_dir_var(&paths::env_var_name(APP_ID), env).is_some() {
        return Vec::new();
    }
    let Some(home) = env("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
    else {
        return Vec::new();
    };
    let old_root = home.join(".var/app").join(OLD_FLATPAK_APP_ID);
    let base = |var: &str, fallback: &str| {
        env(var)
            .filter(|dir| !dir.is_empty())
            .map_or_else(|| home.join(fallback), PathBuf::from)
    };
    [
        ("config", base("XDG_CONFIG_HOME", ".config")),
        ("data", base("XDG_DATA_HOME", ".local/share")),
    ]
    .into_iter()
    .map(|(kind, new_base)| (old_root.join(kind).join(OLD_APP_ID), new_base.join(APP_ID)))
    .collect()
}

/// Copies each `from` folder to its `to` folder, once (see [`flatpak_folders`]).
///
/// # Arguments
/// * `pairs` - `(from, to)` folders
///
/// # Returns
/// One entry per pair; failures are reported, never raised.
pub fn migrate_folder_pairs(pairs: &[(PathBuf, PathBuf)]) -> Vec<FolderMigration> {
    pairs
        .iter()
        .map(|(from, to)| FolderMigration {
            from: from.clone(),
            to: to.clone(),
            result: migrate_renamed_folder(from, to),
        })
        .collect()
}

/// Copies `<base>/gcodekit5` to `<base>/gcodekit6` for each base, once.
///
/// # Arguments
/// * `bases` - the parent folders to look in (see [`platform_bases`])
///
/// # Returns
/// One entry per base; failures are reported, never raised.
pub fn migrate_renamed_folders(bases: &[PathBuf]) -> Vec<FolderMigration> {
    bases
        .iter()
        .map(|base| {
            let from = base.join(OLD_APP_ID);
            let to = base.join(APP_ID);
            let result = migrate_renamed_folder(&from, &to);
            FolderMigration { from, to, result }
        })
        .collect()
}

/// Copies the folder `from` to `to` when `from` exists and `to` does not.
///
/// # Arguments
/// * `from` - the pre-rename folder
/// * `to` - the new folder
///
/// # Errors
/// [`SettingsError::Io`] when a file or folder cannot be read, created or renamed; `to`
/// is then not created.
fn migrate_renamed_folder(from: &Path, to: &Path) -> Result<bool, SettingsError> {
    if to.exists() || !from.is_dir() {
        return Ok(false);
    }
    let mut staging = to.as_os_str().to_owned();
    staging.push(STAGING_SUFFIX);
    let staging = PathBuf::from(staging);
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(io(parent))?;
    }
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(io(&staging))?;
    }
    copy_dir(from, &staging)?;
    std::fs::rename(&staging, to).map_err(io(to))?;
    Ok(true)
}

/// Recursively copies the folder `src` to `dest`, which must not exist.
fn copy_dir(src: &Path, dest: &Path) -> Result<(), SettingsError> {
    std::fs::create_dir(dest).map_err(io(dest))?;
    for entry in std::fs::read_dir(src).map_err(io(src))? {
        let path = entry.map_err(io(src))?.path();
        let Some(name) = path.file_name() else {
            continue;
        };
        let target = dest.join(name);
        if path.is_dir() {
            copy_dir(&path, &target)?;
        } else {
            std::fs::copy(&path, &target).map_err(io(&path))?;
        }
    }
    Ok(())
}

/// Maps an I/O error on `path` to [`SettingsError::Io`].
fn io(path: &Path) -> impl FnOnce(std::io::Error) -> SettingsError {
    let path = path.to_path_buf();
    move |source| SettingsError::Io { path, source }
}
