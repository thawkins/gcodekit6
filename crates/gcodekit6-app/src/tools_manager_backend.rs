//! CNC Tools Manager Backend
//!
//! This module provides backend logic for the CNC Tools Manager UI,
//! including persistence for custom tools.

use gcodekit6_core::data::gtc_import::{GtcImportResult, GtcImporter};
use gcodekit6_core::data::tools::{
    ShankType, Tool, ToolCoating, ToolCuttingParams, ToolId, ToolLibrary, ToolMaterial, ToolType,
};
use gcodekit6_core::types::aliases::BoxedError;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedTool {
    pub id: ToolId,
    /// The tool number; files written before it was stored load as 0.
    #[serde(default)]
    pub number: u32,
    pub name: String,
    pub description: String,
    pub tool_type: ToolType,

    pub diameter: f32,
    pub shaft_diameter: Option<f32>,
    pub length: f32,
    pub flute_length: f32,
    pub flutes: u32,
    pub corner_radius: Option<f32>,
    pub tip_angle: Option<f32>,

    pub material: ToolMaterial,
    pub coating: Option<ToolCoating>,
    pub shank: ShankType,

    pub params: ToolCuttingParams,

    pub manufacturer: Option<String>,
    pub part_number: Option<String>,
    pub cost: Option<f32>,
    pub notes: String,
    pub custom: bool,
}

impl From<&Tool> for PersistedTool {
    fn from(t: &Tool) -> Self {
        Self {
            id: t.id.clone(),
            number: t.number,
            name: t.name.clone(),
            description: t.description.clone(),
            tool_type: t.tool_type,
            diameter: t.diameter,
            shaft_diameter: t.shaft_diameter,
            length: t.length,
            flute_length: t.flute_length,
            flutes: t.flutes,
            corner_radius: t.corner_radius,
            tip_angle: t.tip_angle,
            material: t.material,
            coating: t.coating,
            shank: t.shank,
            params: t.params.clone(),
            manufacturer: t.manufacturer.clone(),
            part_number: t.part_number.clone(),
            cost: t.cost,
            notes: t.notes.clone(),
            custom: t.custom,
        }
    }
}

impl From<PersistedTool> for Tool {
    fn from(t: PersistedTool) -> Self {
        let mut tool = Tool::new(t.id, t.number, t.name, t.tool_type, t.diameter, t.length);
        tool.description = t.description;
        tool.shaft_diameter = t.shaft_diameter;
        tool.flute_length = t.flute_length;
        tool.flutes = t.flutes;
        tool.corner_radius = t.corner_radius;
        tool.tip_angle = t.tip_angle;
        tool.material = t.material;
        tool.coating = t.coating;
        tool.shank = t.shank;
        tool.params = t.params;
        tool.manufacturer = t.manufacturer;
        tool.part_number = t.part_number;
        tool.cost = t.cost;
        tool.notes = t.notes;
        tool.custom = t.custom;
        tool
    }
}

/// The file, in the settings folder, that holds the custom tools.
pub const TOOLS_FILE: &str = "custom_tools.json";

/// The tool library (the standard tools plus the user's custom tools) and the file the
/// custom tools are kept in.
///
/// Saves are atomic (a temporary file renamed over the target). When the file exists but
/// cannot be read, the error is kept ([`ToolsManagerBackend::load_error`]) and the next
/// write first copies the file to `custom_tools.json.bak-<time>`
/// ([`ToolsManagerBackend::last_backup`]), so an unreadable library is never lost; if that
/// copy fails, nothing is written.
pub struct ToolsManagerBackend {
    library: ToolLibrary,
    file: CustomLibraryFile,
}

impl ToolsManagerBackend {
    /// Creates the backend with the standard library plus the custom tools
    /// saved in [`ToolsManagerBackend::default_storage_path`].
    pub fn new() -> Self {
        Self::with_storage_path(Self::default_storage_path())
    }

    /// Creates the backend with custom tools loaded from, and saved to,
    /// `storage_path` instead of the user's configuration directory.
    ///
    /// Tests use this so they never read or overwrite the user's library.
    ///
    /// # Arguments
    /// * `storage_path` - the JSON file holding the custom tools
    pub fn with_storage_path(storage_path: PathBuf) -> Self {
        let mut library = gcodekit6_core::data::tools::init_standard_library();
        let (file, custom_tools) = CustomLibraryFile::open::<PersistedTool>(storage_path, "tools");
        for tool in custom_tools {
            library.add_tool(tool.into());
        }
        Self { library, file }
    }

    /// The custom tools file: `custom_tools.json` in the settings folder
    /// ([`gcodekit6_settings::SettingsManager::config_directory`]), or in
    /// `~/gcodekit6` on a platform without a configuration folder.
    pub fn default_storage_path() -> PathBuf {
        storage_dir().join(TOOLS_FILE)
    }

    /// The file the custom tools are kept in.
    pub fn storage_path(&self) -> &Path {
        self.file.path()
    }

    /// Why the custom tools file could not be read when the backend was created, if it
    /// could not.
    pub fn load_error(&self) -> Option<&str> {
        self.file.load_error()
    }

    /// Where a save copied the unreadable custom tools file before replacing it, if one did.
    pub fn last_backup(&self) -> Option<&Path> {
        self.file.last_backup()
    }

    /// The custom tools as the file holds them.
    fn custom_json(&self) -> Result<String, BoxedError> {
        // Only custom tools are saved.
        let mut custom_tools: Vec<PersistedTool> = self
            .library
            .get_all_tools()
            .into_iter()
            .filter(|t| t.custom)
            .map(PersistedTool::from)
            .collect();
        custom_tools.sort_by(|a, b| a.id.0.cmp(&b.id.0));
        Ok(serde_json::to_string_pretty(&custom_tools)?)
    }

    fn save_to_file(&mut self) -> Result<(), BoxedError> {
        let json = self.custom_json()?;
        self.file.save(&json)
    }

    /// Adds `tool`, replacing any tool with the same id, and writes the custom
    /// tools file.
    ///
    /// # Errors
    /// The write error message. The library is then restored to its previous
    /// state, so memory never holds an edit that is not on disk.
    pub fn save_tool(&mut self, tool: Tool) -> Result<(), String> {
        let previous = self.library.remove_tool(&tool.id);
        let id = tool.id.clone();
        self.library.add_tool(tool);
        if let Err(e) = self.save_to_file() {
            self.library.remove_tool(&id);
            if let Some(previous) = previous {
                self.library.add_tool(previous);
            }
            return Err(e.to_string());
        }
        Ok(())
    }

    /// Removes the tool with `id` and writes the custom tools file. A custom tool that
    /// replaced a standard tool gives way to the standard tool again, as after a restart.
    ///
    /// # Returns
    /// The removed tool, or `None` when no tool has that id.
    ///
    /// # Errors
    /// The write error message. The tool is then put back, so memory never
    /// differs from what is on disk.
    pub fn remove_tool(&mut self, id: &ToolId) -> Result<Option<Tool>, String> {
        let removed = self.library.remove_tool(id);
        let standard = gcodekit6_core::data::tools::init_standard_library()
            .get_tool(id)
            .cloned();
        if let Some(standard) = standard.clone() {
            self.library.add_tool(standard);
        }
        if let Err(e) = self.save_to_file() {
            if standard.is_some() {
                self.library.remove_tool(id);
            }
            if let Some(tool) = removed {
                self.library.add_tool(tool);
            }
            return Err(e.to_string());
        }
        Ok(removed)
    }

    /// The tool with `id`, if the library has one.
    pub fn get_tool(&self, id: &ToolId) -> Option<&Tool> {
        self.library.get_tool(id)
    }

    /// Every tool, standard and custom (unordered).
    pub fn get_all_tools(&self) -> Vec<&Tool> {
        self.library.get_all_tools()
    }

    /// Writes the custom tools to `path` in the custom tools file's format.
    ///
    /// # Errors
    /// The tools cannot be serialised or the file cannot be written.
    pub fn export_custom_tools<P: AsRef<Path>>(&self, path: P) -> Result<(), BoxedError> {
        let json = self.custom_json()?;
        write_atomic(path.as_ref(), &json)?;
        Ok(())
    }

    /// Removes every custom and imported tool and deletes the custom tools file (an
    /// unreadable file is backed up first). The standard tools they replaced come back.
    ///
    /// # Errors
    /// The backup or the delete failed; the library is then unchanged.
    pub fn reset_custom_tools(&mut self) -> Result<(), BoxedError> {
        self.file.remove()?;
        self.library = gcodekit6_core::data::tools::init_standard_library();
        Ok(())
    }

    /// Import tools from a GTC package (.zip file)
    ///
    /// # Errors
    /// The package cannot be read, or the custom tools file cannot be written (the
    /// imported tools are then dropped again).
    pub fn import_gtc_package<P: AsRef<Path>>(
        &mut self,
        zip_path: P,
    ) -> Result<GtcImportResult, BoxedError> {
        let mut importer = GtcImporter::new(self.next_tool_number());
        let result = importer.import_from_zip(zip_path)?;
        self.add_imported(&result)?;
        Ok(result)
    }

    /// Import tools from a GTC JSON file
    ///
    /// # Errors
    /// The file cannot be read, or the custom tools file cannot be written (the imported
    /// tools are then dropped again).
    pub fn import_gtc_json<P: AsRef<Path>>(
        &mut self,
        json_path: P,
    ) -> Result<GtcImportResult, BoxedError> {
        let mut importer = GtcImporter::new(self.next_tool_number());
        let result = importer.import_from_json(json_path)?;
        self.add_imported(&result)?;
        Ok(result)
    }

    /// One more than the highest tool number in the library.
    fn next_tool_number(&self) -> u32 {
        self.library
            .get_all_tools()
            .iter()
            .map(|t| t.number)
            .max()
            .unwrap_or(0)
            + 1
    }

    /// Adds the imported tools and saves; on a failed save the library is put back.
    fn add_imported(&mut self, result: &GtcImportResult) -> Result<(), BoxedError> {
        let previous = self.library.clone();
        for tool in &result.imported_tools {
            self.library.add_tool(tool.clone());
        }
        if let Err(e) = self.save_to_file() {
            self.library = previous;
            return Err(e);
        }
        Ok(())
    }
}

impl Default for ToolsManagerBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// The settings folder, or `~/gcodekit6` without a platform config folder.
pub(crate) fn storage_dir() -> PathBuf {
    gcodekit6_settings::SettingsManager::config_directory()
        .ok()
        .or_else(|| dirs::home_dir().map(|home| home.join("gcodekit6")))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// A custom library file (custom tools or custom materials): a JSON list read once when
/// the library is created and replaced atomically on every save.
///
/// When the file exists but cannot be read, the error is kept ([`Self::load_error`]) and the
/// next write or removal first copies the file to `<file>.bak-<time>` ([`Self::last_backup`]),
/// so an unreadable library is never lost; if that copy fails, nothing is written.
pub(crate) struct CustomLibraryFile {
    path: PathBuf,
    /// What the file holds ("tools", "materials"), for the log.
    kind: &'static str,
    load_error: Option<String>,
    needs_backup: bool,
    last_backup: Option<PathBuf>,
}

impl CustomLibraryFile {
    /// Reads the entries in `path` (none when it does not exist or cannot be read).
    ///
    /// # Arguments
    /// * `path` - The JSON file holding the custom entries.
    /// * `kind` - What the file holds ("tools", "materials"), for the log.
    ///
    /// # Returns
    /// The file and the entries it held.
    pub(crate) fn open<T: DeserializeOwned>(path: PathBuf, kind: &'static str) -> (Self, Vec<T>) {
        let mut entries = Vec::new();
        let mut load_error = None;
        if path.exists() {
            match read_entries(&path) {
                Ok(read) => entries = read,
                Err(e) => {
                    tracing::error!(
                        path = %path.display(),
                        error = %e,
                        "custom {kind} file not readable; it is backed up before the next save"
                    );
                    load_error = Some(e.to_string());
                }
            }
        }
        let needs_backup = load_error.is_some();
        let file = Self {
            path,
            kind,
            load_error,
            needs_backup,
            last_backup: None,
        };
        (file, entries)
    }

    /// The file's path.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Why the file could not be read when it was opened, if it could not.
    pub(crate) fn load_error(&self) -> Option<&str> {
        self.load_error.as_deref()
    }

    /// Where the unreadable file was copied before it was first replaced, if it was.
    pub(crate) fn last_backup(&self) -> Option<&Path> {
        self.last_backup.as_deref()
    }

    /// Writes `json` over the file (atomically), backing an unreadable file up first.
    ///
    /// # Errors
    /// The backup or the write failed.
    pub(crate) fn save(&mut self, json: &str) -> Result<(), BoxedError> {
        self.backup_if_needed()?;
        write_atomic(&self.path, json)?;
        Ok(())
    }

    /// Deletes the file (a missing file is fine), backing an unreadable file up first.
    ///
    /// # Errors
    /// The backup or the delete failed.
    pub(crate) fn remove(&mut self) -> Result<(), BoxedError> {
        self.backup_if_needed()?;
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Copies an unreadable file aside before it is first replaced.
    fn backup_if_needed(&mut self) -> Result<(), BoxedError> {
        if !self.needs_backup {
            return Ok(());
        }
        if self.path.exists() {
            let backup = backup_path(&self.path);
            std::fs::copy(&self.path, &backup)?;
            tracing::warn!(
                backup = %backup.display(),
                "backed up unreadable custom {} file",
                self.kind
            );
            self.last_backup = Some(backup);
        }
        self.needs_backup = false;
        Ok(())
    }
}

/// The JSON list in `path`.
fn read_entries<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, BoxedError> {
    let contents = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&contents)?)
}

/// `<file>.bak-<seconds>.<millis>` beside `path`.
fn backup_path(path: &Path) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!(
        "{name}.bak-{}.{:03}",
        stamp.as_secs(),
        stamp.subsec_millis()
    ))
}

/// Writes `text` to `path` with the shared atomic write ([`crate::files::write_atomic`]),
/// creating the folder first.
pub(crate) fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    crate::files::write_atomic(path, text)
}
