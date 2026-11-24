//! Settings storage on `craft-settings` and the one-shot import of the GTK
//! front end's files (D3).
//!
//! The settings ([`Config`]) live in `config.toml`, written by
//! `craft-settings` (atomic save, `config.toml.bak` backup, broken-file
//! recovery, schema migration). The GTK build kept them in `config.json` and
//! its user data (machine profiles, tool library, materials, command history)
//! in JSON files beside it, all in `<platform config dir>/gcodekit5/`.
//!
//! [`import_legacy`] runs once: it converts `config.json` into `config.toml`,
//! copies the data files into the new folder when it differs from the old one,
//! and leaves a marker so later runs do nothing. Legacy files are never
//! modified or removed, so an older build keeps working after a rollback.
//!
//! Depends on `craft-settings` for the store and its folder rules.

use crate::config::Config;
use crate::manager::SettingsManager;
use craft_settings::paths::{self, Probe};
use craft_settings::{LoadReport, Location, Portable, SettingsStore};
use std::path::{Path, PathBuf};

/// Application id: the settings folder name and the `GCODEKIT5_CONFIG_DIR`
/// override variable. Matches the GTK build's folder, so on every platform the
/// new files land beside the legacy ones.
pub const APP_ID: &str = "gcodekit5";

/// The GTK build's settings file, in the legacy folder.
pub const LEGACY_CONFIG_FILE: &str = "config.json";

/// The GTK build's data files (machine profiles, tool library, materials,
/// console history), carried over unchanged.
pub const LEGACY_DATA_FILES: &[&str] = &[
    "devices.json",
    "custom_tools.json",
    "custom_materials.json",
    "command_history.json",
];

/// Written into the new folder once the import has run.
pub const MARKER_FILE: &str = "legacy-import.done";

impl craft_settings::Settings for Config {
    const APP_ID: &'static str = APP_ID;
    const SCHEMA_VERSION: u32 = 1;
}

/// Errors from importing or storing settings.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    /// The settings store could not save.
    #[error(transparent)]
    Store(#[from] craft_settings::SettingsError),
    /// The settings folder cannot be written.
    #[error("settings are read-only: {0}")]
    ReadOnly(String),
    /// A legacy file exists but could not be parsed.
    #[error("cannot import {path}: {message}")]
    Legacy {
        /// The legacy file.
        path: PathBuf,
        /// Why it was rejected.
        message: String,
    },
    /// A file could not be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The file or folder involved.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
}

/// What [`import_legacy`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationReport {
    /// This many legacy files (settings and data) were carried over.
    Imported {
        /// Number of files imported.
        items: usize,
    },
    /// There were no legacy files, or the new folder already had them all.
    NothingToImport,
    /// The import already ran in this folder.
    AlreadyMigrated,
}

/// The folders the import reads from and writes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDirs {
    /// The GTK build's folder (`config.json` and the data files).
    pub legacy: PathBuf,
    /// The craft-settings folder (`config.toml` and the data files).
    pub config: PathBuf,
}

impl ConfigDirs {
    /// Folders given explicitly (tests, a `--config-dir` flag).
    ///
    /// # Arguments
    /// * `legacy` - the folder holding the GTK build's files
    /// * `config` - the folder for `config.toml` and the data files
    pub fn new(legacy: impl Into<PathBuf>, config: impl Into<PathBuf>) -> Self {
        Self {
            legacy: legacy.into(),
            config: config.into(),
        }
    }

    /// The real folders on this platform.
    ///
    /// # Returns
    /// `None` when there is no platform config folder, or when
    /// `GCODEKIT5_CONFIG_DIR` is set but empty (isolated: nothing is read or
    /// written).
    pub fn platform() -> Option<Self> {
        let legacy = SettingsManager::config_directory().ok()?;
        let resolved = paths::resolve(
            APP_ID,
            &Portable::DEFAULT,
            &Location::Auto,
            &Probe::default(),
        );
        Some(Self::new(legacy, resolved.dir?))
    }
}

/// Imports the GTK build's settings and data files into `dirs.config`, once.
///
/// `config.json` becomes `config.toml` unless one exists already; each data
/// file is copied unless the new folder already has it. Legacy files are left
/// in place. A marker in `dirs.config` makes every later call return
/// [`MigrationReport::AlreadyMigrated`]; it is not written when the legacy
/// settings could not be read, so a repaired file is imported next time.
///
/// # Arguments
/// * `dirs` - the legacy and new folders
///
/// # Errors
/// [`SettingsError::Legacy`] when `config.json` cannot be parsed,
/// [`SettingsError::ReadOnly`] / [`SettingsError::Store`] when `config.toml`
/// cannot be written, [`SettingsError::Io`] when a data file cannot be copied.
pub fn import_legacy(dirs: &ConfigDirs) -> Result<MigrationReport, SettingsError> {
    let marker = dirs.config.join(MARKER_FILE);
    if marker.exists() {
        return Ok(MigrationReport::AlreadyMigrated);
    }
    let legacy_config = dirs.legacy.join(LEGACY_CONFIG_FILE);
    let data_files: Vec<&str> = LEGACY_DATA_FILES
        .iter()
        .copied()
        .filter(|name| dirs.legacy.join(name).is_file())
        .collect();
    if !legacy_config.is_file() && data_files.is_empty() {
        return Ok(MigrationReport::NothingToImport);
    }

    let mut items = 0;
    if legacy_config.is_file() && import_config(&legacy_config, &dirs.config)? {
        items += 1;
    }
    for name in data_files {
        if copy_data_file(&dirs.legacy.join(name), &dirs.config.join(name))? {
            items += 1;
        }
    }

    std::fs::write(&marker, format!("{}\n", dirs.legacy.display())).map_err(|source| {
        SettingsError::Io {
            path: marker.clone(),
            source,
        }
    })?;
    Ok(if items == 0 {
        MigrationReport::NothingToImport
    } else {
        MigrationReport::Imported { items }
    })
}

/// Startup load path: runs [`import_legacy`], then opens the settings store.
///
/// The store always opens (defaults on any problem); the import result and the
/// store's load report say what to show the user.
///
/// # Arguments
/// * `dirs` - the legacy and new folders
pub fn open_store(
    dirs: &ConfigDirs,
) -> (
    SettingsStore<Config>,
    LoadReport,
    Result<MigrationReport, SettingsError>,
) {
    let migration = import_legacy(dirs);
    let (store, report) = SettingsStore::<Config>::open_at(&dirs.config);
    (store, report, migration)
}

/// Writes `legacy` (`config.json`) to `config.toml` in `dir` when that does
/// not exist yet. Returns whether it was imported.
fn import_config(legacy: &Path, dir: &Path) -> Result<bool, SettingsError> {
    let config = Config::load_from_file(legacy).map_err(|e| SettingsError::Legacy {
        path: legacy.to_path_buf(),
        message: e.to_string(),
    })?;
    if dir.join(craft_settings::store::CONFIG_FILE).exists() {
        return Ok(false);
    }
    let (mut store, report) = SettingsStore::<Config>::open_at(dir);
    if let Some(reason) = report.read_only {
        return Err(SettingsError::ReadOnly(reason.to_string()));
    }
    store.set(config);
    store.flush()?;
    Ok(true)
}

/// Copies a data file to `dest` unless it is already there (including when
/// both folders are the same). Returns whether `dest` now holds the legacy
/// data. The copy goes through a temporary file and a rename, so `dest` is
/// never left half written.
fn copy_data_file(src: &Path, dest: &Path) -> Result<bool, SettingsError> {
    if same_file(src, dest) {
        return Ok(true);
    }
    if dest.exists() {
        return Ok(false);
    }
    let io = |path: &Path| {
        let path = path.to_path_buf();
        move |source| SettingsError::Io { path, source }
    };
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(io(parent))?;
    }
    let mut tmp = dest.as_os_str().to_owned();
    tmp.push(".import-tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::copy(src, &tmp).map_err(io(&tmp))?;
    std::fs::rename(&tmp, dest).map_err(io(dest))?;
    Ok(true)
}

/// True when both paths name the same existing file.
fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
