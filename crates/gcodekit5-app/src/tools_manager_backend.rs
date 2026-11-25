//! CNC Tools Manager Backend
//!
//! This module provides backend logic for the CNC Tools Manager UI,
//! including persistence for custom tools.

use gcodekit5_core::data::gtc_import::{GtcImportResult, GtcImporter};
use gcodekit5_core::data::tools::{
    ShankType, Tool, ToolCoating, ToolCuttingParams, ToolId, ToolLibrary, ToolMaterial, ToolType,
};
use gcodekit5_core::types::aliases::BoxedError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedTool {
    pub id: ToolId,
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
        // Tool number is intentionally not persisted (it is not a device tool index).
        // Use 0 as a stable placeholder.
        let mut tool = Tool::new(t.id, 0, t.name, t.tool_type, t.diameter, t.length);
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

/// The file, in the settings folder of the GTK build, that holds the custom tools.
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
    storage_path: PathBuf,
    load_error: Option<String>,
    needs_backup: bool,
    last_backup: Option<PathBuf>,
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
        let mut library = gcodekit5_core::data::tools::init_standard_library();
        let mut load_error = None;
        if storage_path.exists() {
            match Self::load_from_file(&storage_path) {
                Ok(custom_tools) => {
                    for tool in custom_tools {
                        library.add_tool(tool);
                    }
                }
                Err(e) => {
                    tracing::error!(
                        path = %storage_path.display(),
                        error = %e,
                        "custom tools file not readable; it is backed up before the next save"
                    );
                    load_error = Some(e.to_string());
                }
            }
        }
        let needs_backup = load_error.is_some();
        Self {
            library,
            storage_path,
            load_error,
            needs_backup,
            last_backup: None,
        }
    }

    /// The custom tools file both builds use: `custom_tools.json` in the GTK build's
    /// settings folder ([`gcodekit5_settings::SettingsManager::config_directory`]), or in
    /// `~/gcodekit5` on a platform without a configuration folder.
    pub fn default_storage_path() -> PathBuf {
        storage_dir().join(TOOLS_FILE)
    }

    /// The file the custom tools are kept in.
    pub fn storage_path(&self) -> &Path {
        &self.storage_path
    }

    /// Why the custom tools file could not be read when the backend was created, if it
    /// could not.
    pub fn load_error(&self) -> Option<&str> {
        self.load_error.as_deref()
    }

    /// Where a save copied the unreadable custom tools file before replacing it, if one did.
    pub fn last_backup(&self) -> Option<&Path> {
        self.last_backup.as_deref()
    }

    fn load_from_file(path: &Path) -> Result<Vec<Tool>, BoxedError> {
        let contents = std::fs::read_to_string(path)?;
        let tools: Vec<PersistedTool> = serde_json::from_str(&contents)?;
        Ok(tools.into_iter().map(Into::into).collect())
    }

    /// The custom tools as the file holds them.
    fn custom_json(&self) -> Result<String, BoxedError> {
        // Only custom tools are saved (tool numbers are intentionally not persisted).
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
        self.backup_if_needed()?;
        write_atomic(&self.storage_path, &json)?;
        Ok(())
    }

    /// Copies an unreadable custom tools file aside before it is first replaced.
    fn backup_if_needed(&mut self) -> Result<(), BoxedError> {
        if !self.needs_backup {
            return Ok(());
        }
        if self.storage_path.exists() {
            let backup = backup_path(&self.storage_path);
            std::fs::copy(&self.storage_path, &backup)?;
            tracing::warn!(backup = %backup.display(), "backed up unreadable custom tools file");
            self.last_backup = Some(backup);
        }
        self.needs_backup = false;
        Ok(())
    }

    /// Adds `tool` (replacing one with the same id) and writes the custom tools file; a
    /// write error is only logged. Prefer [`ToolsManagerBackend::save_tool`].
    pub fn add_tool(&mut self, tool: Tool) {
        self.library.add_tool(tool);
        // Save custom tools to disk
        if let Err(e) = self.save_to_file() {
            tracing::warn!("Failed to save tools: {}", e);
        }
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
        let standard = gcodekit5_core::data::tools::init_standard_library()
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

    /// The tools whose name matches `query` (every tool for an empty query).
    pub fn search_tools(&self, query: &str) -> Vec<&Tool> {
        if query.is_empty() {
            self.library.get_all_tools()
        } else {
            self.library.search_by_name(query)
        }
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
        self.backup_if_needed()?;
        match std::fs::remove_file(&self.storage_path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        self.library = gcodekit5_core::data::tools::init_standard_library();
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

/// The tool type whose name is `s`, if any.
pub fn string_to_tool_type(s: &str) -> Option<ToolType> {
    ToolType::all().iter().find(|t| t.to_string() == s).cloned()
}

/// The tool material with GTK's combo text `s`, if any.
pub fn string_to_tool_material(s: &str) -> Option<ToolMaterial> {
    match s {
        "HSS" => Some(ToolMaterial::HSS),
        "Carbide" => Some(ToolMaterial::Carbide),
        "Coated Carbide" => Some(ToolMaterial::CoatedCarbide),
        "Diamond" => Some(ToolMaterial::Diamond),
        _ => None,
    }
}

/// The GTK build's settings folder, or `~/gcodekit5` without a platform config folder.
pub(crate) fn storage_dir() -> PathBuf {
    gcodekit5_settings::SettingsManager::config_directory()
        .ok()
        .or_else(|| dirs::home_dir().map(|home| home.join("gcodekit5")))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// `<file>.bak-<seconds>.<millis>` beside `path`.
pub(crate) fn backup_path(path: &Path) -> PathBuf {
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

/// Writes `text` to a temporary file beside `path` and renames it over `path`, creating
/// the folder first; the temporary file is removed when anything fails.
pub(crate) fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    let result = std::fs::write(&temp, text).and_then(|()| std::fs::rename(&temp, path));
    if result.is_err()
        && let Err(error) = std::fs::remove_file(&temp)
    {
        tracing::debug!(%error, "temporary file not removed");
    }
    result
}
