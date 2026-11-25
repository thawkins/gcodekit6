//! The CNC Tools manager's rules, shared by the GTK and egui windows: the edit form (every
//! field GTK's four tabs edit) with its validation and messages, the list's search and
//! filters, and the order tools are listed in.
//!
//! Lengths are millimetres (as GTK's form, which never followed the unit setting); the tip
//! angle is degrees. Error and label strings are English msgids for the caller to translate.
//!
//! Dependencies: `gcodekit5-core` (tool types).

use gcodekit5_core::data::tools::{ShankType, Tool, ToolCoating, ToolId, ToolMaterial, ToolType};

/// The tool types in GTK's form and filter order.
pub const TOOL_TYPES: [ToolType; 9] = [
    ToolType::EndMillFlat,
    ToolType::EndMillBall,
    ToolType::EndMillCornerRadius,
    ToolType::VBit,
    ToolType::DrillBit,
    ToolType::SpotDrill,
    ToolType::EngravingBit,
    ToolType::ChamferTool,
    ToolType::Specialty,
];

/// The tool materials in GTK's order.
pub const TOOL_MATERIALS: [ToolMaterial; 4] = [
    ToolMaterial::HSS,
    ToolMaterial::Carbide,
    ToolMaterial::CoatedCarbide,
    ToolMaterial::Diamond,
];

/// The coatings in GTK's order; `None` is GTK's "None".
pub const COATINGS: [Option<ToolCoating>; 5] = [
    None,
    Some(ToolCoating::TiN),
    Some(ToolCoating::TiAlN),
    Some(ToolCoating::DLC),
    Some(ToolCoating::AlOx),
];

/// The msgid of a tool type's name (GTK's labels).
pub fn tool_type_label(tool_type: ToolType) -> &'static str {
    match tool_type {
        ToolType::EndMillFlat => n_!("Flat End Mill"),
        ToolType::EndMillBall => n_!("Ball End Mill"),
        ToolType::EndMillCornerRadius => n_!("Corner Radius End Mill"),
        ToolType::VBit => n_!("V-Bit"),
        ToolType::DrillBit => n_!("Drill Bit"),
        ToolType::SpotDrill => n_!("Spot Drill"),
        ToolType::EngravingBit => n_!("Engraving Bit"),
        ToolType::ChamferTool => n_!("Chamfer Tool"),
        ToolType::Specialty => n_!("Specialty"),
    }
}

/// GTK's filter id of a tool type, as kept in the settings (`tools_manager_type_filter`).
pub fn tool_type_filter_id(tool_type: ToolType) -> &'static str {
    match tool_type {
        ToolType::EndMillFlat => "endmill_flat",
        ToolType::EndMillBall => "endmill_ball",
        ToolType::EndMillCornerRadius => "endmill_cr",
        ToolType::VBit => "vbit",
        ToolType::DrillBit => "drill",
        ToolType::SpotDrill => "spot",
        ToolType::EngravingBit => "engraving",
        ToolType::ChamferTool => "chamfer",
        ToolType::Specialty => "specialty",
    }
}

/// The tool type with GTK filter id `id`, if any (`"all"` and unknown ids have none).
pub fn tool_type_from_filter_id(id: &str) -> Option<ToolType> {
    TOOL_TYPES
        .into_iter()
        .find(|t| tool_type_filter_id(*t) == id)
}

/// The msgid of a tool material's name (GTK's form labels).
pub fn tool_material_label(material: ToolMaterial) -> &'static str {
    match material {
        ToolMaterial::HSS => "HSS",
        ToolMaterial::Carbide => n_!("Carbide"),
        ToolMaterial::CoatedCarbide => n_!("Coated Carbide"),
        ToolMaterial::Diamond => n_!("Diamond"),
    }
}

/// GTK's filter id of a tool material, as kept in the settings
/// (`tools_manager_material_filter`).
pub fn tool_material_filter_id(material: ToolMaterial) -> &'static str {
    match material {
        ToolMaterial::HSS => "HSS",
        ToolMaterial::Carbide => "Carbide",
        ToolMaterial::CoatedCarbide => "Coated Carbide",
        ToolMaterial::Diamond => "Diamond",
    }
}

/// The tool material with GTK filter id `id`, if any.
pub fn tool_material_from_filter_id(id: &str) -> Option<ToolMaterial> {
    TOOL_MATERIALS
        .into_iter()
        .find(|m| tool_material_filter_id(*m) == id)
}

/// The msgid of a coating's name; `None` is "None".
pub fn coating_label(coating: Option<ToolCoating>) -> &'static str {
    match coating {
        None => n_!("None"),
        Some(ToolCoating::TiN) => "TiN",
        Some(ToolCoating::TiAlN) => "TiAlN",
        Some(ToolCoating::DLC) => "DLC",
        Some(ToolCoating::AlOx) => "AlOx",
    }
}

/// The shank choices of GTK's form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShankChoice {
    /// A straight shank whose size is the shaft diameter.
    Derived,
    /// A collet.
    Collet,
    /// A tapered shank.
    Tapered,
}

impl ShankChoice {
    /// Every choice, in GTK's order.
    pub const ALL: [Self; 3] = [Self::Derived, Self::Collet, Self::Tapered];

    /// The msgid of the choice's name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Derived => n_!("Derived (Straight from shaft diameter)"),
            Self::Collet => n_!("Collet"),
            Self::Tapered => n_!("Tapered"),
        }
    }

    fn of(shank: ShankType) -> Self {
        match shank {
            ShankType::Straight(_) => Self::Derived,
            ShankType::Collet => Self::Collet,
            ShankType::Tapered => Self::Tapered,
        }
    }

    fn shank(self, shaft_diameter_mm: f32) -> ShankType {
        match self {
            // GTK's rule: tenths of a millimetre, truncated.
            Self::Derived => ShankType::Straight((shaft_diameter_mm * 10.0) as u32),
            Self::Collet => ShankType::Collet,
            Self::Tapered => ShankType::Tapered,
        }
    }
}

/// Whether a tool type has a corner radius (GTK enables the field only for it).
pub fn has_corner_radius(tool_type: ToolType) -> bool {
    tool_type == ToolType::EndMillCornerRadius
}

/// Whether a tool type has a tip angle (V-bits and drills).
pub fn has_tip_angle(tool_type: ToolType) -> bool {
    matches!(
        tool_type,
        ToolType::VBit | ToolType::DrillBit | ToolType::SpotDrill
    )
}

/// The edit form: every field GTK's Basic Info, Geometry, Manufacturer and Notes tabs edit,
/// lengths as typed (millimetres).
#[derive(Debug, Clone, PartialEq)]
pub struct ToolForm {
    /// The tool id (only editable for a new tool).
    pub id: String,
    /// The tool number, 1 to 999.
    pub number: u32,
    /// The name.
    pub name: String,
    /// The type.
    pub tool_type: ToolType,
    /// The material.
    pub material: ToolMaterial,
    /// The coating.
    pub coating: Option<ToolCoating>,
    /// The shank.
    pub shank: ShankChoice,
    /// Cutting diameter (mm).
    pub diameter: String,
    /// Overall length (mm).
    pub length: String,
    /// Flute length (mm).
    pub flute_length: String,
    /// Shaft diameter (mm).
    pub shaft_diameter: String,
    /// Number of flutes, 1 to 8.
    pub flutes: u32,
    /// Corner radius (mm); used only for a corner radius end mill.
    pub corner_radius: String,
    /// Tip angle (degrees); used only for V-bits and drills.
    pub tip_angle: String,
    /// The manufacturer.
    pub manufacturer: String,
    /// The part number.
    pub part_number: String,
    /// The description.
    pub description: String,
    /// The notes.
    pub notes: String,
}

impl Default for ToolForm {
    /// GTK's form for a new tool (`clear_form`).
    fn default() -> Self {
        Self {
            id: String::new(),
            number: 1,
            name: String::new(),
            tool_type: ToolType::EndMillFlat,
            material: ToolMaterial::Carbide,
            coating: None,
            shank: ShankChoice::Derived,
            diameter: "6.350".into(),
            length: "50.000".into(),
            flute_length: "20.000".into(),
            shaft_diameter: "6.350".into(),
            flutes: 2,
            corner_radius: String::new(),
            tip_angle: String::new(),
            manufacturer: String::new(),
            part_number: String::new(),
            description: String::new(),
            notes: String::new(),
        }
    }
}

/// Parses a required length.
fn required(text: &str, error: &'static str) -> Result<f32, &'static str> {
    text.trim().parse::<f32>().map_err(|_| error)
}

/// Parses an optional length; empty is `None`.
fn optional(text: &str, error: &'static str) -> Result<Option<f32>, &'static str> {
    let text = text.trim();
    if text.is_empty() {
        Ok(None)
    } else {
        text.parse::<f32>().map(Some).map_err(|_| error)
    }
}

/// `None` for an empty (trimmed) text.
fn non_empty(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

impl ToolForm {
    /// The form showing `tool` (GTK's `load_tool_for_edit`, three decimals).
    ///
    /// # Arguments
    /// * `tool` - The tool to edit.
    pub fn from_tool(tool: &Tool) -> Self {
        let mm = |v: f32| format!("{v:.3}");
        let mut form = Self {
            id: tool.id.0.clone(),
            number: tool.number,
            name: tool.name.clone(),
            tool_type: tool.tool_type,
            material: tool.material,
            coating: tool.coating,
            shank: ShankChoice::of(tool.shank),
            diameter: mm(tool.diameter),
            length: mm(tool.length),
            flute_length: mm(tool.flute_length),
            shaft_diameter: mm(tool.shaft_diameter.unwrap_or(tool.diameter)),
            flutes: tool.flutes,
            corner_radius: tool.corner_radius.map(mm).unwrap_or_default(),
            tip_angle: tool.tip_angle.map(mm).unwrap_or_default(),
            manufacturer: tool.manufacturer.clone().unwrap_or_default(),
            part_number: tool.part_number.clone().unwrap_or_default(),
            description: tool.description.clone(),
            notes: tool.notes.clone(),
        };
        form.clear_unused_fields();
        form
    }

    /// Sets the tool type and clears the fields it does not use (GTK's
    /// `update_type_dependent_fields`).
    ///
    /// # Arguments
    /// * `tool_type` - The new type.
    pub fn set_tool_type(&mut self, tool_type: ToolType) {
        self.tool_type = tool_type;
        self.clear_unused_fields();
    }

    /// Empties the corner radius and tip angle when the type does not use them.
    pub fn clear_unused_fields(&mut self) {
        if !has_corner_radius(self.tool_type) {
            self.corner_radius.clear();
        }
        if !has_tip_angle(self.tool_type) {
            self.tip_angle.clear();
        }
    }

    /// The tool the form describes (GTK's `build_tool_from_form`), marked custom.
    ///
    /// Fields the form does not edit (cutting parameters, extended geometry, limits,
    /// features, cost) are kept from `base`, the tool being edited.
    ///
    /// # Arguments
    /// * `base` - The tool being edited, or `None` for a new tool.
    ///
    /// # Errors
    /// The msgid of the first invalid field ("Tool ID is required", "Invalid diameter", ...).
    pub fn build(&self, base: Option<&Tool>) -> Result<Tool, &'static str> {
        let id = self.id.trim();
        if id.is_empty() {
            return Err(n_!("Tool ID is required"));
        }
        let name = self.name.trim();
        if name.is_empty() {
            return Err(n_!("Tool name is required"));
        }
        let diameter = required(&self.diameter, n_!("Invalid diameter"))?;
        let length = required(&self.length, n_!("Invalid length"))?;
        let flute_length = required(&self.flute_length, n_!("Invalid flute length"))?;
        let shaft_diameter = required(&self.shaft_diameter, n_!("Invalid shaft diameter"))?;
        let corner_radius = if has_corner_radius(self.tool_type) {
            optional(&self.corner_radius, n_!("Invalid corner radius"))?
        } else {
            None
        };
        let tip_angle = if has_tip_angle(self.tool_type) {
            optional(&self.tip_angle, n_!("Invalid tip angle"))?
        } else {
            None
        };

        let mut tool = match base {
            Some(base) => base.clone(),
            None => Tool::new(
                ToolId(String::new()),
                0,
                String::new(),
                self.tool_type,
                diameter,
                length,
            ),
        };
        tool.id = ToolId(id.to_string());
        tool.number = self.number;
        tool.name = name.to_string();
        tool.tool_type = self.tool_type;
        tool.diameter = diameter;
        tool.length = length;
        tool.flute_length = flute_length;
        tool.shaft_diameter = Some(shaft_diameter);
        tool.flutes = self.flutes;
        tool.corner_radius = corner_radius;
        tool.tip_angle = tip_angle;
        tool.material = self.material;
        tool.coating = self.coating;
        tool.shank = self.shank.shank(shaft_diameter);
        tool.manufacturer = non_empty(&self.manufacturer);
        tool.part_number = non_empty(&self.part_number);
        tool.description = self.description.clone();
        tool.notes = self.notes.clone();
        tool.custom = true;
        Ok(tool)
    }

    /// Whether the form differs from `original` (GTK's `is_dirty`): a form that does not
    /// build counts as changed.
    ///
    /// # Arguments
    /// * `original` - The tool being edited, or `None` for a new tool (always changed).
    pub fn is_dirty(&self, original: Option<&Tool>) -> bool {
        let Some(original) = original else {
            return true;
        };
        match self.build(Some(original)) {
            Ok(current) => !tool_contents_equal(original, &current),
            Err(_) => true,
        }
    }
}

/// Whether two tools hold the same values at the form's precision (GTK's
/// `tool_contents_equal`): lengths within 0.0001 mm, a missing shaft diameter equal to the
/// diameter, an empty manufacturer or part number equal to none.
///
/// # Arguments
/// * `a`, `b` - The tools to compare.
pub fn tool_contents_equal(a: &Tool, b: &Tool) -> bool {
    let eq_f32 = |x: f32, y: f32| (x - y).abs() < 0.0001;
    let eq_opt_f32 = |x: Option<f32>, y: Option<f32>| match (x, y) {
        (Some(vx), Some(vy)) => eq_f32(vx, vy),
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    };
    let shaft = |t: &Tool| t.shaft_diameter.unwrap_or(t.diameter);
    let eq_str = |x: &Option<String>, y: &Option<String>| {
        x.as_deref().unwrap_or("").trim() == y.as_deref().unwrap_or("").trim()
    };
    a.id.0 == b.id.0
        && a.number == b.number
        && a.name == b.name
        && a.description == b.description
        && a.tool_type == b.tool_type
        && eq_f32(a.diameter, b.diameter)
        && eq_f32(a.length, b.length)
        && eq_f32(a.flute_length, b.flute_length)
        && a.flutes == b.flutes
        && eq_opt_f32(a.corner_radius, b.corner_radius)
        && eq_opt_f32(a.tip_angle, b.tip_angle)
        && eq_f32(shaft(a), shaft(b))
        && a.material == b.material
        && a.coating == b.coating
        && a.shank == b.shank
        && eq_str(&a.manufacturer, &b.manufacturer)
        && eq_str(&a.part_number, &b.part_number)
        && a.notes == b.notes
}

/// The list's search text and filters (GTK's sidebar).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolFilter {
    /// Matches the name, id, type name or diameter (three decimals), ignoring case.
    pub search: String,
    /// Only this type, or every type.
    pub tool_type: Option<ToolType>,
    /// Only this material, or every material.
    pub material: Option<ToolMaterial>,
    /// Smallest diameter (mm) as typed; empty for none.
    pub dia_min: String,
    /// Largest diameter (mm) as typed; empty for none.
    pub dia_max: String,
}

/// A typed diameter bound: `Ok(None)` when empty, `Err` when it is not a number.
fn bound(text: &str) -> Result<Option<f32>, ()> {
    let text = text.trim();
    if text.is_empty() {
        Ok(None)
    } else {
        text.parse::<f32>().map(Some).map_err(|_| ())
    }
}

impl ToolFilter {
    /// Whether the smallest-diameter text is not a number (GTK marks the field).
    pub fn dia_min_invalid(&self) -> bool {
        bound(&self.dia_min).is_err()
    }

    /// Whether the largest-diameter text is not a number (GTK marks the field).
    pub fn dia_max_invalid(&self) -> bool {
        bound(&self.dia_max).is_err()
    }

    /// Whether `tool` is listed. An invalid diameter bound is ignored, as GTK does.
    ///
    /// # Arguments
    /// * `tool` - The tool to test.
    pub fn matches(&self, tool: &Tool) -> bool {
        let q = self.search.to_lowercase();
        if !q.is_empty() {
            let dia = format!("{:.3}", tool.diameter);
            let found = tool.name.to_lowercase().contains(&q)
                || tool.id.0.to_lowercase().contains(&q)
                || tool.tool_type.to_string().to_lowercase().contains(&q)
                || dia.contains(&q);
            if !found {
                return false;
            }
        }
        if self.tool_type.is_some_and(|t| t != tool.tool_type) {
            return false;
        }
        if self.material.is_some_and(|m| m != tool.material) {
            return false;
        }
        if let Ok(Some(min)) = bound(&self.dia_min)
            && tool.diameter < min
        {
            return false;
        }
        if let Ok(Some(max)) = bound(&self.dia_max)
            && tool.diameter > max
        {
            return false;
        }
        true
    }

    /// The tools listed, by id then name (GTK's order).
    ///
    /// # Arguments
    /// * `tools` - Every tool.
    pub fn apply<'a>(&self, tools: impl IntoIterator<Item = &'a Tool>) -> Vec<&'a Tool> {
        let mut listed: Vec<&Tool> = tools.into_iter().filter(|t| self.matches(t)).collect();
        listed.sort_by(|a, b| a.id.0.cmp(&b.id.0).then_with(|| a.name.cmp(&b.name)));
        listed
    }
}

/// The one-line summary under a tool's name in the list (GTK's row), lengths in mm.
///
/// # Arguments
/// * `tool` - The tool.
/// * `type_name` - The tool type's (translated) name.
/// * `none` - The (translated) word for no coating.
pub fn tool_summary(tool: &Tool, type_name: &str, none: &str) -> String {
    let coating = tool
        .coating
        .map(|c| c.to_string())
        .unwrap_or_else(|| none.to_string());
    let shaft = tool.shaft_diameter.unwrap_or(tool.diameter);
    format!(
        "{type_name} - id:{} - D{:.3}mm - shaft D{:.3}mm - {}F - FL{:.3}mm - {} / {coating}",
        tool.id.0, tool.diameter, shaft, tool.flutes, tool.flute_length, tool.material,
    )
}

/// The English msgids of this module, for the translation test.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        n_!("Tool ID is required"),
        n_!("Tool name is required"),
        n_!("Invalid diameter"),
        n_!("Invalid length"),
        n_!("Invalid flute length"),
        n_!("Invalid shaft diameter"),
        n_!("Invalid corner radius"),
        n_!("Invalid tip angle"),
        n_!("Carbide"),
        n_!("Coated Carbide"),
        n_!("Diamond"),
        n_!("None"),
    ];
    ids.extend(TOOL_TYPES.map(tool_type_label));
    ids.extend(ShankChoice::ALL.map(ShankChoice::label));
    ids
}
