//! Materials Manager Backend
//!
//! The material library the Materials Manager edits and its custom materials file, plus
//! the id conversions the GTK window uses.

use crate::tools_manager_backend::{CustomLibraryFile, storage_dir};
use gcodekit5_core::data::materials::{Material, MaterialCategory, MaterialId, MaterialLibrary};
use gcodekit5_core::types::aliases::BoxedError;
use std::path::{Path, PathBuf};

/// The file, in the settings folder of the GTK build, that holds the custom materials.
pub const MATERIALS_FILE: &str = "custom_materials.json";

/// The material library (the standard materials plus the user's custom materials) and the
/// file the custom materials are kept in.
///
/// Saves are atomic (a temporary file renamed over the target). When the file exists but
/// cannot be read, the error is kept ([`MaterialsManagerBackend::load_error`]) and the next
/// write first copies the file to `custom_materials.json.bak-<time>`
/// ([`MaterialsManagerBackend::last_backup`]), so an unreadable library is never lost; if
/// that copy fails, nothing is written.
pub struct MaterialsManagerBackend {
    library: MaterialLibrary,
    file: CustomLibraryFile,
}

impl MaterialsManagerBackend {
    /// Creates the backend with the standard library plus the custom materials
    /// saved in [`MaterialsManagerBackend::default_storage_path`].
    pub fn new() -> Self {
        Self::with_storage_path(Self::default_storage_path())
    }

    /// Creates the backend with custom materials loaded from, and saved to,
    /// `storage_path` instead of the user's configuration directory.
    ///
    /// Tests use this so they never read or overwrite the user's library.
    ///
    /// # Arguments
    /// * `storage_path` - the JSON file holding the custom materials
    pub fn with_storage_path(storage_path: PathBuf) -> Self {
        let mut library = gcodekit5_core::data::materials::init_standard_library();
        let (file, custom_materials) = CustomLibraryFile::open(storage_path, "materials");
        for material in custom_materials {
            library.add_material(material);
        }
        Self { library, file }
    }

    /// The custom materials file both builds use: `custom_materials.json` in the GTK
    /// build's settings folder ([`gcodekit5_settings::SettingsManager::config_directory`]),
    /// or in `~/gcodekit5` on a platform without a configuration folder.
    pub fn default_storage_path() -> PathBuf {
        storage_dir().join(MATERIALS_FILE)
    }

    /// The file the custom materials are kept in.
    pub fn storage_path(&self) -> &Path {
        self.file.path()
    }

    /// Why the custom materials file could not be read when the backend was created, if it
    /// could not.
    pub fn load_error(&self) -> Option<&str> {
        self.file.load_error()
    }

    /// Where a save copied the unreadable custom materials file before replacing it, if
    /// one did.
    pub fn last_backup(&self) -> Option<&Path> {
        self.file.last_backup()
    }

    fn save_to_file(&mut self) -> Result<(), BoxedError> {
        // Only custom materials are saved.
        let mut custom_materials: Vec<&Material> = self
            .library
            .get_all_materials()
            .into_iter()
            .filter(|m| m.custom)
            .collect();
        custom_materials.sort_by(|a, b| a.id.0.cmp(&b.id.0));
        let json = serde_json::to_string_pretty(&custom_materials)?;
        self.file.save(&json)
    }

    /// Adds `material` (replacing one with the same id) and writes the custom materials
    /// file; a write error is only logged. Prefer [`MaterialsManagerBackend::save_material`].
    pub fn add_material(&mut self, material: Material) {
        if let Err(e) = self.save_material(material) {
            tracing::warn!("Failed to save materials: {}", e);
        }
    }

    /// Adds `material`, replacing any material with the same id, and writes the custom
    /// materials file.
    ///
    /// # Errors
    /// The write error message. The library is then restored to its previous state, so
    /// memory never holds an edit that is not on disk.
    pub fn save_material(&mut self, material: Material) -> Result<(), String> {
        let id = material.id.clone();
        let previous = self.library.remove_material(&id);
        self.library.add_material(material);
        if let Err(e) = self.save_to_file() {
            self.library.remove_material(&id);
            if let Some(previous) = previous {
                self.library.add_material(previous);
            }
            return Err(e.to_string());
        }
        Ok(())
    }

    /// Removes the material with `id` and writes the custom materials file. A custom
    /// material that replaced a standard one gives way to the standard material again, as
    /// after a restart.
    ///
    /// # Returns
    /// The removed material, or `None` when no material has that id.
    ///
    /// # Errors
    /// The write error message. The material is then put back.
    pub fn remove_material(&mut self, id: &MaterialId) -> Result<Option<Material>, String> {
        let removed = self.library.remove_material(id);
        let standard = gcodekit5_core::data::materials::init_standard_library()
            .get_material(id)
            .cloned();
        if let Some(standard) = standard.clone() {
            self.library.add_material(standard);
        }
        if let Err(e) = self.save_to_file() {
            if standard.is_some() {
                self.library.remove_material(id);
            }
            if let Some(material) = removed {
                self.library.add_material(material);
            }
            return Err(e.to_string());
        }
        Ok(removed)
    }

    /// The material with `id`, if the library has one.
    pub fn get_material(&self, id: &MaterialId) -> Option<&Material> {
        self.library.get_material(id)
    }

    /// The materials whose name or subcategory matches `query` (every material for an
    /// empty query).
    pub fn search_materials(&self, query: &str) -> Vec<&Material> {
        if query.is_empty() {
            self.library.get_all_materials()
        } else {
            self.library.search_by_name(query)
        }
    }

    /// Every material, standard and custom (unordered).
    pub fn get_all_materials(&self) -> Vec<&Material> {
        self.library.get_all_materials()
    }
}

impl Default for MaterialsManagerBackend {
    fn default() -> Self {
        Self::new()
    }
}

pub fn string_to_category(s: &str) -> Option<MaterialCategory> {
    match s {
        "Wood" => Some(MaterialCategory::Wood),
        "Engineered Wood" => Some(MaterialCategory::EngineeredWood),
        "Plastic" => Some(MaterialCategory::Plastic),
        "Non-Ferrous Metal" => Some(MaterialCategory::NonFerrousMetal),
        "Ferrous Metal" => Some(MaterialCategory::FerrousMetal),
        "Composite" => Some(MaterialCategory::Composite),
        "Stone and Ceramic" => Some(MaterialCategory::StoneAndCeramic),
        _ => None,
    }
}
