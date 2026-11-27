//! Configuration and settings management for GCodeKit6
//!
//! Provides configuration file handling, settings management, and validation.
//! Supports JSON and TOML file formats stored in platform-specific directories.
//!
//! Configuration is organized into logical sections:
//! - Connection settings (ports, timeouts, protocols)
//! - UI preferences (theme, layout, fonts)
//! - File processing defaults (preprocessors, arc settings)
//! - Machine preferences (limits, jog settings)
//! - Firmware-specific settings

pub use gcodekit6_core::units::{FeedRateUnits, MeasurementSystem};
use gcodekit6_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Connection protocol type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionType {
    /// Serial/USB connection
    Serial,
    /// TCP/IP connection
    Tcp,
    /// WebSocket connection
    WebSocket,
}

impl std::fmt::Display for ConnectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serial => write!(f, "serial"),
            Self::Tcp => write!(f, "tcp"),
            Self::WebSocket => write!(f, "websocket"),
        }
    }
}

/// Connection settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectionSettings {
    /// Last used connection type
    pub connection_type: ConnectionType,
    /// Last used port (serial) or hostname (TCP/WebSocket)
    pub port: String,
    /// Baud rate for serial connections
    pub baud_rate: u32,
    /// TCP port for network connections
    pub tcp_port: u16,
    /// Connection timeout in milliseconds
    pub timeout_ms: u64,
    /// Auto-reconnect on connection loss
    pub auto_reconnect: bool,
}

impl Default for ConnectionSettings {
    fn default() -> Self {
        Self {
            connection_type: ConnectionType::Serial,
            port: "Auto".to_string(),
            baud_rate: 115200,
            tcp_port: 8888,
            timeout_ms: 5000,
            auto_reconnect: true,
        }
    }
}

/// Theme selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Follow system preference
    #[default]
    System,
    /// Force light theme
    Light,
    /// Force dark theme
    Dark,
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::System => write!(f, "System"),
            Self::Light => write!(f, "Light"),
            Self::Dark => write!(f, "Dark"),
        }
    }
}

// Measurement system selection
// Removed local definition, imported from gcodekit6_core::units

// Feed rate units selection
// Removed local definition, imported from gcodekit6_core::units

/// UI preference settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    /// Whether panels are visible (by name)
    pub panel_visibility: HashMap<String, bool>,
    /// Selected theme (light/dark/system)
    #[serde(default)]
    pub theme: Theme,
    /// UI language code (e.g., "en", "es", "fr")
    pub language: String,
    /// Measurement system (Metric or Imperial)
    #[serde(default)]
    pub measurement_system: MeasurementSystem,
    /// Feed rate units preference
    #[serde(default)]
    pub feed_rate_units: FeedRateUnits,

    /// Show the About dialog on startup (auto-closes after a short delay)
    #[serde(default)]
    pub show_about_on_startup: bool,

    /// Enable 3D stock removal visualization (experimental)
    #[serde(default)]
    pub enable_stock_removal_3d: bool,

    /// Enable STL file import with shadow projection (experimental)
    #[serde(default)]
    pub enable_stl_import: bool,
    /// Tools tab search text
    #[serde(default)]
    pub tools_manager_search: String,

    /// Tools tab type filter active id
    #[serde(default)]
    pub tools_manager_type_filter: String,

    /// Tools tab material filter active id
    #[serde(default)]
    pub tools_manager_material_filter: String,

    /// Tools tab diameter min filter (mm)
    #[serde(default)]
    pub tools_manager_dia_min: Option<f32>,

    /// Tools tab diameter max filter (mm)
    #[serde(default)]
    pub tools_manager_dia_max: Option<f32>,

    /// Tools tab last selected tool id
    #[serde(default)]
    pub tools_manager_selected_tool: Option<String>,

    /// Grid major line width in pixels (coarse grid lines)
    #[serde(default = "default_grid_major_line_width")]
    pub grid_major_line_width: f64,

    /// Grid minor line width in pixels (fine grid lines)
    #[serde(default = "default_grid_minor_line_width")]
    pub grid_minor_line_width: f64,

    /// "Use system title bar": the desktop draws the window frame instead of the
    /// app's own title bar (egui front end; read before the window opens)
    #[serde(default)]
    pub use_system_title_bar: bool,

    /// The user's changes to the default keyboard shortcuts (egui front end's
    /// Preferences > Shortcuts): each entry replaces one command's default keys.
    /// Saved as an array, which the store replaces whole, so an entry removed
    /// (a command reset to its defaults) stays removed.
    #[serde(default)]
    pub keyboard_shortcuts: Vec<ShortcutOverride>,
}

/// One command's keyboard shortcuts in place of its defaults, in craft-keymap's
/// stored form (`{ command, keys }`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShortcutOverride {
    /// The command's stable id (`file.open`).
    pub command: String,
    /// The keys in canonical text form (`Cmd+Shift+O`); empty for no shortcut.
    pub keys: Vec<String>,
}

impl Default for UiSettings {
    fn default() -> Self {
        let mut visibility = HashMap::new();
        visibility.insert("connection".to_string(), true);
        visibility.insert("dro".to_string(), true);
        visibility.insert("jog".to_string(), true);
        visibility.insert("console".to_string(), true);
        visibility.insert("visualizer_sidebar".to_string(), true);
        visibility.insert("device_config_sidebar".to_string(), true);

        Self {
            panel_visibility: visibility,
            theme: Theme::default(),
            language: "en".to_string(),
            measurement_system: MeasurementSystem::default(),
            feed_rate_units: FeedRateUnits::default(),
            show_about_on_startup: false,
            enable_stock_removal_3d: false,
            enable_stl_import: false,
            tools_manager_search: String::new(),
            tools_manager_type_filter: String::new(),
            tools_manager_material_filter: String::new(),
            tools_manager_dia_min: None,
            tools_manager_dia_max: None,
            tools_manager_selected_tool: None,
            grid_major_line_width: 2.0,
            grid_minor_line_width: 1.0,
            use_system_title_bar: false,
            keyboard_shortcuts: Vec::new(),
        }
    }
}

/// Default value for grid major line width
fn default_grid_major_line_width() -> f64 {
    2.0
}

/// Default value for grid minor line width
fn default_grid_minor_line_width() -> f64 {
    1.0
}

/// File processing settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FileProcessingSettings {
    /// The longest chord (mm) the visualizer draws an arc with; arcs also take steps of at
    /// most 5 degrees, so the default of 100 mm binds only on very large arcs.
    pub arc_segment_length: f64,
    /// Maximum line length in characters
    pub max_line_length: u32,
    /// Whether to preserve comments
    pub preserve_comments: bool,
    /// Default output directory
    pub output_directory: PathBuf,
    /// Whether to add `N[nnn]` line numbers in generated G-code
    pub line_numbers_enabled: bool,
}

impl Default for FileProcessingSettings {
    fn default() -> Self {
        Self {
            arc_segment_length: 100.0,
            max_line_length: 256,
            preserve_comments: false,
            output_directory: dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")),
            line_numbers_enabled: false,
        }
    }
}

/// Machine preference settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MachineSettings {
    /// Default jog increment in mm (linear axes)
    pub jog_increment: f64,
    /// Default jog increment in degrees (rotary axes)
    pub jog_increment_rotary: f64,
    /// Default jog feed rate in units/min
    pub jog_feed_rate: f64,
    /// Machine X limit (max)
    pub x_limit: f64,
    /// Machine Y limit (max)
    pub y_limit: f64,
    /// Machine Z limit (max)
    pub z_limit: f64,
    /// Machine A limit (max degrees)
    pub a_limit: f64,
    /// Machine B limit (max degrees)
    pub b_limit: f64,
    /// Machine C limit (max degrees)
    pub c_limit: f64,
    /// Direction inversion per axis (true = inverted)
    pub axis_direction_invert: HashMap<String, bool>,
    /// Rotary axis calibration offsets (degrees)
    pub rotary_calibration: HashMap<String, f64>,
}

impl Default for MachineSettings {
    fn default() -> Self {
        let mut direction_invert = HashMap::new();
        direction_invert.insert("X".to_string(), false);
        direction_invert.insert("Y".to_string(), false);
        direction_invert.insert("Z".to_string(), false);
        direction_invert.insert("A".to_string(), false);
        direction_invert.insert("B".to_string(), false);
        direction_invert.insert("C".to_string(), false);

        let mut rotary_cal = HashMap::new();
        rotary_cal.insert("A".to_string(), 0.0);
        rotary_cal.insert("B".to_string(), 0.0);
        rotary_cal.insert("C".to_string(), 0.0);

        Self {
            jog_increment: 1.0,
            jog_increment_rotary: 1.0,
            jog_feed_rate: 1000.0,
            x_limit: 200.0,
            y_limit: 200.0,
            z_limit: 100.0,
            a_limit: 360.0,
            b_limit: 360.0,
            c_limit: 360.0,
            axis_direction_invert: direction_invert,
            rotary_calibration: rotary_cal,
        }
    }
}

/// Complete application configuration
///
/// Aggregates all settings sections and provides file I/O operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
// Missing fields (e.g. from older config files) fall back to Default
// instead of failing the whole load and discarding the user's settings.
#[serde(default)]
pub struct Config {
    /// Connection settings
    pub connection: ConnectionSettings,
    /// UI preferences
    pub ui: UiSettings,
    /// File processing settings
    pub file_processing: FileProcessingSettings,
    /// Machine preferences
    pub machine: MachineSettings,
}

impl Config {
    /// Create new config with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Load config from file (JSON or TOML)
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::other(format!("Failed to read config file: {}", e)))?;

        let config: Self = if path.extension().is_some_and(|ext| ext == "json") {
            serde_json::from_str(&content)
                .map_err(|e| Error::other(format!("Invalid JSON config: {}", e)))?
        } else if path.extension().is_some_and(|ext| ext == "toml") {
            toml::from_str(&content)
                .map_err(|e| Error::other(format!("Invalid TOML config: {}", e)))?
        } else {
            return Err(Error::other(
                "Config file must be .json or .toml".to_string(),
            ));
        };

        config.validate()?;
        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate connection settings
        if self.connection.timeout_ms == 0 {
            return Err(Error::other("Connection timeout must be > 0".to_string()));
        }

        if self.connection.baud_rate == 0 {
            return Err(Error::other("Baud rate must be > 0".to_string()));
        }

        // Validate file processing
        if self.file_processing.arc_segment_length <= 0.0 {
            return Err(Error::other("Arc segment length must be > 0".to_string()));
        }

        if self.file_processing.max_line_length == 0 {
            return Err(Error::other("Max line length must be > 0".to_string()));
        }

        // Validate machine settings
        if self.machine.jog_feed_rate <= 0.0 {
            return Err(Error::other("Jog feed rate must be > 0".to_string()));
        }

        if self.machine.x_limit <= 0.0 || self.machine.y_limit <= 0.0 || self.machine.z_limit <= 0.0
        {
            return Err(Error::other("Machine limits must be > 0".to_string()));
        }

        // Validate rotary axis limits (can be 0 if axis is not used)
        if self.machine.a_limit < 0.0 || self.machine.b_limit < 0.0 || self.machine.c_limit < 0.0 {
            return Err(Error::other("Rotary axis limits must be >= 0".to_string()));
        }

        Ok(())
    }
}
