//! The Materials manager's rules, shared by the GTK and egui windows: the edit form (every
//! field GTK's Basic Info, Properties, Machining, Safety and Notes tabs edit) with its
//! validation and messages, the list's search and category filter, and the list order.
//!
//! Error and label strings are English msgids for the caller to translate.
//!
//! Dependencies: `gcodekit5-core` (material types).

use gcodekit5_core::data::materials::{
    Abrasiveness, ChipType, HazardLevel, HeatSensitivity, Material, MaterialCategory, MaterialId,
    SurfaceFinishability,
};

/// The categories in GTK's order (also the list's sort order).
pub const CATEGORIES: [MaterialCategory; 7] = [
    MaterialCategory::Wood,
    MaterialCategory::EngineeredWood,
    MaterialCategory::Plastic,
    MaterialCategory::NonFerrousMetal,
    MaterialCategory::FerrousMetal,
    MaterialCategory::Composite,
    MaterialCategory::StoneAndCeramic,
];

/// The chip types in GTK's order.
pub const CHIP_TYPES: [ChipType; 4] = [
    ChipType::Continuous,
    ChipType::Segmented,
    ChipType::Granular,
    ChipType::Small,
];

/// The heat sensitivities in GTK's order.
pub const HEAT_SENSITIVITIES: [HeatSensitivity; 3] = [
    HeatSensitivity::Low,
    HeatSensitivity::Moderate,
    HeatSensitivity::High,
];

/// The abrasiveness levels in GTK's order.
pub const ABRASIVENESS: [Abrasiveness; 3] = [
    Abrasiveness::Low,
    Abrasiveness::Moderate,
    Abrasiveness::High,
];

/// The surface finishes in GTK's order.
pub const SURFACE_FINISHES: [SurfaceFinishability; 4] = [
    SurfaceFinishability::Excellent,
    SurfaceFinishability::Good,
    SurfaceFinishability::Fair,
    SurfaceFinishability::Rough,
];

/// The hazard levels in GTK's order.
pub const HAZARD_LEVELS: [HazardLevel; 4] = [
    HazardLevel::None,
    HazardLevel::Minimal,
    HazardLevel::Moderate,
    HazardLevel::High,
];

/// The density range GTK's spin button allows (kg/m3).
pub const DENSITY_RANGE: std::ops::RangeInclusive<f32> = 0.0..=20000.0;

/// The machinability range GTK's spin button allows.
pub const MACHINABILITY_RANGE: std::ops::RangeInclusive<u8> = 1..=10;

/// The msgid of a category's name.
pub fn category_label(category: MaterialCategory) -> &'static str {
    match category {
        MaterialCategory::Wood => n_!("Wood"),
        MaterialCategory::EngineeredWood => n_!("Engineered Wood"),
        MaterialCategory::Plastic => n_!("Plastic"),
        MaterialCategory::NonFerrousMetal => n_!("Non-Ferrous Metal"),
        MaterialCategory::FerrousMetal => n_!("Ferrous Metal"),
        MaterialCategory::Composite => n_!("Composite"),
        MaterialCategory::StoneAndCeramic => n_!("Stone and Ceramic"),
    }
}

/// The msgid of a chip type's name.
pub fn chip_type_label(chip_type: ChipType) -> &'static str {
    match chip_type {
        ChipType::Continuous => n_!("Continuous"),
        ChipType::Segmented => n_!("Segmented"),
        ChipType::Granular => n_!("Granular"),
        ChipType::Small => n_!("Small"),
    }
}

/// The msgid of a heat sensitivity's name.
pub fn heat_sensitivity_label(heat: HeatSensitivity) -> &'static str {
    match heat {
        HeatSensitivity::Low => n_!("Low"),
        HeatSensitivity::Moderate => n_!("Moderate"),
        HeatSensitivity::High => n_!("High"),
    }
}

/// The msgid of an abrasiveness level's name.
pub fn abrasiveness_label(abrasiveness: Abrasiveness) -> &'static str {
    match abrasiveness {
        Abrasiveness::Low => n_!("Low"),
        Abrasiveness::Moderate => n_!("Moderate"),
        Abrasiveness::High => n_!("High"),
    }
}

/// The msgid of a surface finish's name.
pub fn surface_finish_label(finish: SurfaceFinishability) -> &'static str {
    match finish {
        SurfaceFinishability::Excellent => n_!("Excellent"),
        SurfaceFinishability::Good => n_!("Good"),
        SurfaceFinishability::Fair => n_!("Fair"),
        SurfaceFinishability::Rough => n_!("Rough"),
    }
}

/// The msgid of a hazard level's name.
pub fn hazard_level_label(level: HazardLevel) -> &'static str {
    match level {
        HazardLevel::None => n_!("None"),
        HazardLevel::Minimal => n_!("Minimal"),
        HazardLevel::Moderate => n_!("Moderate"),
        HazardLevel::High => n_!("High"),
    }
}

/// The edit form: every field GTK's five tabs edit.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialForm {
    /// The id (only editable for a new material).
    pub id: String,
    /// The name.
    pub name: String,
    /// The category.
    pub category: MaterialCategory,
    /// The subcategory.
    pub subcategory: String,
    /// The description.
    pub description: String,
    /// Density (kg/m3), within [`DENSITY_RANGE`].
    pub density: f32,
    /// Machinability, within [`MACHINABILITY_RANGE`].
    pub machinability: u8,
    /// Tensile strength (MPa) as typed; empty for none.
    pub tensile_strength: String,
    /// Melting point (degrees C) as typed; empty for none.
    pub melting_point: String,
    /// The chip type.
    pub chip_type: ChipType,
    /// The heat sensitivity.
    pub heat_sensitivity: HeatSensitivity,
    /// The abrasiveness.
    pub abrasiveness: Abrasiveness,
    /// The surface finish.
    pub surface_finish: SurfaceFinishability,
    /// The dust hazard.
    pub dust_hazard: HazardLevel,
    /// The fume hazard.
    pub fume_hazard: HazardLevel,
    /// Whether coolant is required.
    pub coolant_required: bool,
    /// The notes.
    pub notes: String,
}

impl Default for MaterialForm {
    /// GTK's form for a new material (`clear_form`).
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            category: MaterialCategory::Wood,
            subcategory: String::new(),
            description: String::new(),
            density: 750.0,
            machinability: 7,
            tensile_strength: String::new(),
            melting_point: String::new(),
            chip_type: ChipType::Continuous,
            heat_sensitivity: HeatSensitivity::Low,
            abrasiveness: Abrasiveness::Low,
            surface_finish: SurfaceFinishability::Good,
            dust_hazard: HazardLevel::Minimal,
            fume_hazard: HazardLevel::None,
            coolant_required: false,
            notes: String::new(),
        }
    }
}

/// Parses an optional number; empty is `None`. Anything else must be a finite number above
/// 0: `NaN` and `inf` would be saved by `serde_json` as `null` and so vanish on reload, and a
/// negative tensile strength or melting point is not a material property.
fn optional(text: &str, error: &'static str) -> Result<Option<f32>, &'static str> {
    let text = text.trim();
    if text.is_empty() {
        Ok(None)
    } else {
        text.parse::<f32>()
            .ok()
            .filter(|v| v.is_finite() && *v > 0.0)
            .map(Some)
            .ok_or(error)
    }
}

impl MaterialForm {
    /// The form showing `material` (GTK's `write_form`).
    ///
    /// # Arguments
    /// * `material` - The material to edit.
    pub fn from_material(material: &Material) -> Self {
        Self {
            id: material.id.0.clone(),
            name: material.name.clone(),
            category: material.category,
            subcategory: material.subcategory.clone(),
            description: material.description.clone(),
            density: material.density,
            machinability: material.machinability_rating,
            tensile_strength: material
                .tensile_strength
                .map(|v| v.to_string())
                .unwrap_or_default(),
            melting_point: material
                .melting_point
                .map(|v| v.to_string())
                .unwrap_or_default(),
            chip_type: material.chip_type,
            heat_sensitivity: material.heat_sensitivity,
            abrasiveness: material.abrasiveness,
            surface_finish: material.surface_finish,
            dust_hazard: material.dust_hazard,
            fume_hazard: material.fume_hazard,
            coolant_required: material.coolant_required,
            notes: material.notes.clone(),
        }
    }

    /// The material the form describes (GTK's `read_form`), marked custom so it is saved.
    ///
    /// Fields the form does not edit (hardness, conductivity, cutting force, PPE, cutting
    /// parameters) are kept from `base`, the material being edited.
    ///
    /// # Arguments
    /// * `base` - The material being edited, or `None` for a new one (then the id is
    ///   checked: required, no spaces).
    ///
    /// # Errors
    /// The msgid of the first invalid field ("Name is required", ...).
    pub fn build(&self, base: Option<&Material>) -> Result<Material, &'static str> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(n_!("Name is required"));
        }
        let subcategory = self.subcategory.trim();
        if subcategory.is_empty() {
            return Err(n_!("Subcategory is required"));
        }
        let tensile_strength = optional(
            &self.tensile_strength,
            n_!("Tensile strength must be a number"),
        )?;
        let melting_point = optional(&self.melting_point, n_!("Melting point must be a number"))?;
        let mut material = match base {
            Some(base) => base.clone(),
            None => {
                let id = self.id.trim();
                if id.is_empty() {
                    return Err(n_!("ID is required"));
                }
                if id.contains(char::is_whitespace) {
                    return Err(n_!("ID must not contain spaces"));
                }
                Material::new(
                    MaterialId(id.to_string()),
                    name.to_string(),
                    self.category,
                    subcategory.to_string(),
                )
            }
        };
        material.name = name.to_string();
        material.category = self.category;
        material.subcategory = subcategory.to_string();
        material.description = self.description.clone();
        material.density = self
            .density
            .clamp(*DENSITY_RANGE.start(), *DENSITY_RANGE.end());
        material.machinability_rating = self
            .machinability
            .clamp(*MACHINABILITY_RANGE.start(), *MACHINABILITY_RANGE.end());
        material.tensile_strength = tensile_strength;
        material.melting_point = melting_point;
        material.chip_type = self.chip_type;
        material.heat_sensitivity = self.heat_sensitivity;
        material.abrasiveness = self.abrasiveness;
        material.surface_finish = self.surface_finish;
        material.dust_hazard = self.dust_hazard;
        material.fume_hazard = self.fume_hazard;
        material.coolant_required = self.coolant_required;
        material.notes = self.notes.clone();
        material.custom = true;
        Ok(material)
    }
}

/// The list's search text and category filter (GTK's sidebar).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MaterialFilter {
    /// Matches the name or subcategory, ignoring case.
    pub search: String,
    /// Only this category, or every category.
    pub category: Option<MaterialCategory>,
}

impl MaterialFilter {
    /// Whether `material` is listed.
    ///
    /// # Arguments
    /// * `material` - The material to test.
    pub fn matches(&self, material: &Material) -> bool {
        let q = self.search.to_lowercase();
        let found = q.is_empty()
            || material.name.to_lowercase().contains(&q)
            || material.subcategory.to_lowercase().contains(&q);
        found && self.category.is_none_or(|c| c == material.category)
    }

    /// The materials listed, by category (GTK's order) then name.
    ///
    /// # Arguments
    /// * `materials` - Every material.
    pub fn apply<'a>(
        &self,
        materials: impl IntoIterator<Item = &'a Material>,
    ) -> Vec<&'a Material> {
        let rank = |c: MaterialCategory| CATEGORIES.iter().position(|x| *x == c);
        let mut listed: Vec<&Material> =
            materials.into_iter().filter(|m| self.matches(m)).collect();
        listed.sort_by(|a, b| {
            rank(a.category)
                .cmp(&rank(b.category))
                .then_with(|| a.name.cmp(&b.name))
        });
        listed
    }
}

/// The English msgids of this module, for the translation test.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        n_!("Name is required"),
        n_!("Subcategory is required"),
        n_!("Tensile strength must be a number"),
        n_!("Melting point must be a number"),
        n_!("ID is required"),
        n_!("ID must not contain spaces"),
    ];
    ids.extend(CATEGORIES.map(category_label));
    ids.extend(CHIP_TYPES.map(chip_type_label));
    ids.extend(HEAT_SENSITIVITIES.map(heat_sensitivity_label));
    ids.extend(SURFACE_FINISHES.map(surface_finish_label));
    ids.extend(HAZARD_LEVELS.map(hazard_level_label));
    ids
}
