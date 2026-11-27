//! Settings Manager
//!
//! Locations of the configuration files, and loading and saving [`Config`]
//! through the craft-settings store (`config.toml`). The first load imports
//! the GTK build's `config.json` and data files once
//! ([`crate::migrate::import_legacy`]); `config.json` is never written again.
//!
//! A [`SettingsManager`] value holds the loaded settings and the folders they
//! came from, for a front end that edits them and applies the result
//! ([`SettingsManager::open`], [`SettingsManager::apply`]).

use crate::config::Config;
use crate::migrate::{self, ConfigDirs};
use craft_settings::SettingsStore;
use craft_settings::store::CONFIG_FILE;
use gcodekit5_core::{Error, Result};
use std::path::PathBuf;

/// Locations of the application's configuration files, and (as a value) the
/// loaded settings with the folders they are saved to.
#[derive(Debug, Clone)]
pub struct SettingsManager {
    /// Where the settings are saved; `None` when there is no settings folder.
    dirs: Option<ConfigDirs>,
    /// The settings as last loaded or applied.
    config: Config,
}

impl SettingsManager {
    /// Loads the settings from `dirs` ([`SettingsManager::load`]) and keeps
    /// the folders for [`SettingsManager::apply`].
    ///
    /// # Arguments
    /// * `dirs` - the legacy and craft-settings folders
    ///
    /// # Returns
    /// The manager, and a description of any problem to show the user.
    pub fn open(dirs: ConfigDirs) -> (Self, Option<String>) {
        let (config, problem) = Self::load(&dirs);
        (
            Self {
                dirs: Some(dirs),
                config,
            },
            problem,
        )
    }

    /// A manager over `config` with no settings folder: applying fails, so
    /// nothing is written (no platform folder, headless use).
    ///
    /// # Arguments
    /// * `config` - the settings to hold
    pub fn detached(config: Config) -> Self {
        Self { dirs: None, config }
    }

    /// The settings as last loaded or applied.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// The folders the settings are saved to, if any.
    pub fn dirs(&self) -> Option<&ConfigDirs> {
        self.dirs.as_ref()
    }

    /// Saves `config` ([`SettingsManager::save`]) and, once it is written,
    /// keeps it as the current settings.
    ///
    /// # Arguments
    /// * `config` - the settings to save
    ///
    /// # Errors
    /// Returns an error when there is no settings folder or the save fails;
    /// the current settings are then unchanged.
    pub fn apply(&mut self, config: Config) -> Result<()> {
        let dirs = self
            .dirs
            .as_ref()
            .ok_or_else(|| Error::other("No settings folder; settings are not saved"))?;
        Self::save(dirs, &config)?;
        self.config = config;
        Ok(())
    }

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
