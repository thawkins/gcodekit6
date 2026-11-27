//! Tool library management for CAM operations.
//!
//! Provides definitions and management for cutting tools with their geometry,
//! cutting parameters, and material-specific settings.

use std::collections::HashMap;

/// Tool types for different operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolType {
    EndMill,
    BallNose,
    VBit,
    Drill,
    Slot,
}

impl ToolType {
    /// Returns the name of the tool type.
    pub fn name(&self) -> &'static str {
        match self {
            ToolType::EndMill => "End Mill",
            ToolType::BallNose => "Ball Nose",
            ToolType::VBit => "V-Bit",
            ToolType::Drill => "Drill",
            ToolType::Slot => "Slot Cutter",
        }
    }
}

/// Represents a cutting tool with its geometry and parameters.
#[derive(Debug, Clone)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub tool_type: ToolType,
    pub diameter: f64,
    pub flutes: u32,
    pub material: String,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub spindle_speed: u32,
    pub max_depth_per_pass: f64,
    pub stepover: f64,
    pub coolant: CoolantType,
}

/// Types of coolant used during cutting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoolantType {
    None,
    Flood,
    Mist,
    ThroughSpindle,
}

impl CoolantType {
    /// Returns the name of the coolant type.
    pub fn name(&self) -> &'static str {
        match self {
            CoolantType::None => "None",
            CoolantType::Flood => "Flood",
            CoolantType::Mist => "Mist",
            CoolantType::ThroughSpindle => "Through Spindle",
        }
    }
}

impl Tool {
    /// Creates a new tool with the given parameters.
    pub fn new(
        id: String,
        name: String,
        tool_type: ToolType,
        diameter: f64,
        flutes: u32,
        material: String,
    ) -> Self {
        Self {
            id,
            name,
            tool_type,
            diameter,
            flutes,
            material,
            feed_rate: 100.0,
            plunge_rate: 50.0,
            spindle_speed: 10000,
            max_depth_per_pass: 5.0,
            stepover: 2.0,
            coolant: CoolantType::None,
        }
    }
}

/// Material definitions with recommended tool parameters.
#[derive(Debug, Clone)]
pub struct MaterialProfile {
    pub name: String,
    pub density: f64,
    pub hardness: String,
    pub recommended_tools: Vec<String>,
    pub cutting_speed: HashMap<String, f64>,
}

impl MaterialProfile {
    /// Creates a new material profile.
    pub fn new(name: String, density: f64, hardness: String) -> Self {
        Self {
            name,
            density,
            hardness,
            recommended_tools: Vec::new(),
            cutting_speed: HashMap::new(),
        }
    }
}

/// Manages a library of tools and materials.
#[derive(Debug, Clone)]
pub struct ToolLibrary {
    tools: HashMap<String, Tool>,
    materials: HashMap<String, MaterialProfile>,
    default_tool: Option<String>,
}

impl ToolLibrary {
    /// Creates a new empty tool library.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            materials: HashMap::new(),
            default_tool: None,
        }
    }

    /// Adds a tool to the library.
    pub fn add_tool(&mut self, tool: Tool) {
        let tool_id = tool.id.clone();
        self.tools.insert(tool_id.clone(), tool);
        if self.default_tool.is_none() {
            self.default_tool = Some(tool_id);
        }
    }

    /// Removes a tool from the library.
    pub fn remove_tool(&mut self, tool_id: &str) -> Option<Tool> {
        self.tools.remove(tool_id)
    }

    /// Gets a tool by ID.
    pub fn get_tool(&self, tool_id: &str) -> Option<&Tool> {
        self.tools.get(tool_id)
    }

    /// Adds a material profile to the library.
    pub fn add_material(&mut self, material: MaterialProfile) {
        self.materials.insert(material.name.clone(), material);
    }

    /// Gets a material profile.
    pub fn get_material(&self, name: &str) -> Option<&MaterialProfile> {
        self.materials.get(name)
    }
}

impl Default for ToolLibrary {
    fn default() -> Self {
        Self::new()
    }
}
