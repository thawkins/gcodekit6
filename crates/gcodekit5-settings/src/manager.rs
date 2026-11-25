//! Settings Manager
//!
//! Locations of the configuration files, and loading and saving [`Config`]
//! through the craft-settings store (`config.toml`). The first load imports
//! the GTK build's `config.json` and data files once
//! ([`crate::migrate::import_legacy`]); `config.json` is never written again.

use crate::config::Config;
use crate::migrate::{self, ConfigDirs};
use craft_settings::SettingsStore;
use craft_settings::store::CONFIG_FILE;
use gcodekit5_core::{Error, Result};
use std::path::PathBuf;

/// Locations of the application's configuration files.
pub struct SettingsManager;

impl SettingsManager {
    /// The legacy (GTK build) config directory, which also holds the device,
    /// tool and material files.
    pub fn config_directory() -> Result<PathBuf> {
        // %APPDATA% on Windows, ~/Library/Application Support on macOS and
        // $XDG_CONFIG_HOME (default ~/.config) on Linux.
        dirs::config_dir()
            .map(|dir| dir.join("gcodekit5"))
            .ok_or_else(|| Error::other("No configuration directory for this platform"))
    }

    /// The legacy and craft-settings folders on this platform.
    ///
    /// # Errors
    /// Returns an error when there is no platform config folder, or when
    /// `GCODEKIT5_CONFIG_DIR` is set but empty (nothing is read or written).
    pub fn config_dirs() -> Result<ConfigDirs> {
        ConfigDirs::platform()
            .ok_or_else(|| Error::other("No settings folder (none on this platform, or isolated)"))
    }

    /// The settings file (`config.toml` in the craft-settings folder).
    pub fn config_file_path() -> Result<PathBuf> {
        Ok(Self::config_dirs()?.config.join(CONFIG_FILE))
    }

    /// Ensure the legacy config directory exists.
    pub fn ensure_config_dir() -> Result<PathBuf> {
        let dir = Self::config_directory()?;
        std::fs::create_dir_all(&dir)
            .map_err(|e| Error::other(format!("Failed to create config directory: {}", e)))?;
        Ok(dir)
    }

    /// Loads the settings from `dirs`, importing the legacy files first when
    /// that has not happened yet. Never fails: problems give defaults (for the
    /// affected part) and are described in the returned message.
    ///
    /// # Arguments
    /// * `dirs` - the legacy and craft-settings folders
    ///
    /// # Returns
    /// The settings, and a description of any problem to show the user.
    pub fn load(dirs: &ConfigDirs) -> (Config, Option<String>) {
        let (store, report, migration) = migrate::open_store(dirs);
        let mut problems = Vec::new();
        if let Err(e) = migration {
            problems.push(e.to_string());
        }
        if let Some(moved) = &report.broken_moved_to {
            problems.push(format!(
                "{CONFIG_FILE} was damaged and moved to {}",
                moved.display()
            ));
        }
        problems.extend(
            report
                .section_errors
                .iter()
                .map(|e| format!("[{}] reset to defaults: {}", e.section, e.message)),
        );
        if let Some(reason) = &report.read_only {
            problems.push(format!("settings are read-only: {reason}"));
        }
        let error = (!problems.is_empty()).then(|| problems.join("\n"));
        (store.get().clone(), error)
    }

    /// Saves `config` to `config.toml` in `dirs.config` (atomic write, the
    /// previous file kept as `config.toml.bak`, unknown keys and comments
    /// preserved).
    ///
    /// # Arguments
    /// * `dirs` - the legacy and craft-settings folders
    /// * `config` - the settings to save
    ///
    /// # Errors
    /// Returns an error when the folder is read-only or the write fails.
    pub fn save(dirs: &ConfigDirs, config: &Config) -> Result<()> {
        let (mut store, report) = SettingsStore::<Config>::open_at(&dirs.config);
        if let Some(reason) = report.read_only {
            return Err(Error::other(format!("settings are read-only: {reason}")));
        }
        store.set(config.clone());
        store
            .flush()
            .map_err(|e| Error::other(format!("Failed to save settings: {e}")))
    }
}
