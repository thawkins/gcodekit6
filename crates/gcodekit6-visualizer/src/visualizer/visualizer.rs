//! G-Code Visualizer
//! Parses G-Code toolpaths into visualization commands

use super::parse::Interpreter;
use super::viewport::Bounds;
use gcodekit6_core::constants as core_constants;
use tracing::debug;

const BOUNDS_PADDING_FACTOR: f32 = core_constants::VIEW_PADDING as f32;

/// Decide whether `new` replaces `old` (a newly loaded program) rather than
/// being an edit of it.
///
/// Used by the UI to keep the user's view while editing and to re-fit it only
/// when a different program is loaded. The text shared by both versions is
/// measured as the common prefix plus the common suffix; an edit keeps most of
/// the old text, a new file keeps little of it.
///
/// # Arguments
/// * `old` - Text currently shown.
/// * `new` - Incoming text.
///
/// # Returns
/// `true` when `old` is blank and `new` is not, or when less than half of the
/// shorter text is shared; `false` otherwise (including when `new` is blank).
pub fn is_content_replacement(old: &str, new: &str) -> bool {
    if new.trim().is_empty() {
        return false;
    }
    if old.trim().is_empty() {
        return true;
    }
    let (a, b) = (old.as_bytes(), new.as_bytes());
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let max_suffix = a.len().min(b.len()) - prefix;
    let suffix = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take(max_suffix)
        .take_while(|(x, y)| x == y)
        .count();
    let shared = prefix + suffix;
    shared * 2 < a.len().min(b.len())
}

/// Colors according to power.
///
/// Uses detected min/max S range and quantizes the gray ramp to a fixed
/// number of levels to keep rendering stable for dense raster engravings.
fn intensity_to_color(intensity: f32, max_intensity: f32) -> (f32, f32, f32) {
    if max_intensity <= 0.0 {
        return (0.5, 0.5, 0.5);
    }
    // Normalize intensity but inverted: 0 = white, 1 = black
    let engraving_color = 1.0 - (intensity / max_intensity).clamp(0.0, 0.5);
    (
        engraving_color / 2.0,
        engraving_color,
        engraving_color / 2.0,
    )
}

/// 6-Axis position including rotary axes
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RotaryPosition {
    pub a: f32, // 4th axis (rotation around X)
    pub b: f32, // 5th axis (rotation around Y)
    pub c: f32, // 6th axis (rotation around Z)
}

impl RotaryPosition {
    pub fn new(a: f32, b: f32, c: f32) -> Self {
        Self { a, b, c }
    }

    pub fn from_optional(a: Option<f32>, b: Option<f32>, c: Option<f32>) -> Self {
        Self {
            a: a.unwrap_or(0.0),
            b: b.unwrap_or(0.0),
            c: c.unwrap_or(0.0),
        }
    }

    /// Check if any rotary axis has non-zero value
    pub fn has_movement(&self) -> bool {
        self.a != 0.0 || self.b != 0.0 || self.c != 0.0
    }

    /// Get the magnitude of rotation (for visualization)
    pub fn magnitude(&self) -> f32 {
        (self.a.powi(2) + self.b.powi(2) + self.c.powi(2)).sqrt()
    }
}

/// 3D Point for visualization
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Point3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

/// Movement command
#[derive(Debug, Clone)]
pub enum GCodeCommand {
    Move {
        from: Point3D,
        to: Point3D,
        rapid: bool,
        /// Spindle/laser power (S word) for the move; `None` when the
        /// spindle/laser was switched off (M5), i.e. a non-cutting move.
        intensity: Option<f32>,
    },
    Arc {
        from: Point3D,
        to: Point3D,
        center: Point3D,
        clockwise: bool,
        /// Spindle/laser power; `None` when the spindle/laser is off (M5).
        intensity: Option<f32>,
    },
    Dwell {
        pos: Point3D,
        duration: f32,
    },
}

/// Visualizer state with 6-axis support
#[derive(Debug, Clone)]
pub struct Visualizer {
    // Linear axis bounds
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
    pub min_z: f32,
    pub max_z: f32,
    // Rotary axis bounds (4th, 5th, 6th axes)
    pub min_a: f32,
    pub max_a: f32,
    pub min_b: f32,
    pub max_b: f32,
    pub min_c: f32,
    pub max_c: f32,
    // Current position (including rotary)
    pub current_pos: Point3D,
    pub current_rotary: RotaryPosition,
    pub current_intensity: f32,
    /// Whether to use different colors for rotary moves
    pub color_rotary_moves: bool,
    /// Commands produced by the last [`Visualizer::parse_gcode`] call
    commands: Vec<GCodeCommand>,

    /// Colors according to intensity
    pub min_intensity: f32,
    pub max_intensity: f32,
    pub use_intensity_colors: bool,
    pub is_raster: bool,
}

impl Visualizer {
    /// Create new visualizer
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new() -> Self {
        Self {
            // Linear axis bounds
            min_x: -(core_constants::WORLD_EXTENT_MM as f32),
            max_x: core_constants::WORLD_EXTENT_MM as f32,
            min_y: -(core_constants::WORLD_EXTENT_MM as f32),
            max_y: core_constants::WORLD_EXTENT_MM as f32,
            min_z: 0.0,
            max_z: 100.0,
            // Rotary axis bounds (default to 0-360 degrees)
            min_a: 0.0,
            max_a: 360.0,
            min_b: 0.0,
            max_b: 360.0,
            min_c: 0.0,
            max_c: 360.0,
            // Position tracking
            current_pos: Point3D::new(0.0, 0.0, 0.0),
            current_rotary: RotaryPosition::default(),
            current_intensity: 0.0,
            color_rotary_moves: true, // Enable rotary color coding by default
            commands: Vec::new(),
            min_intensity: 0.0,
            max_intensity: 1000.0,
            use_intensity_colors: true,
            is_raster: false,
        }
    }

    /// Parse G-Code and extract movement commands
    ///
    /// Honours modal motion (G0-G3, G80), distance mode (G90/G91), units
    /// (G20/G21, converted to mm), arc planes (G17/G18/G19), I/J/K and R arcs
    /// (including negative R and full circles), dwell (G4 P), and non-motion
    /// axis words (G10, G28.1, G30.1, G43.1, G92). Comments in parentheses and
    /// after `;` are ignored and several G words may share one line.
    pub fn parse_gcode(&mut self, gcode: &str) {
        debug!("Starting G-code parse, input size: {} bytes", gcode.len());

        let parsed = Interpreter::run(gcode);

        self.is_raster = parsed.is_raster;
        self.current_intensity = parsed.intensity;
        self.update_intensity_range(&parsed.commands);
        self.current_pos = parsed.pos;
        self.current_rotary = parsed.rotary;
        self.commands = parsed.commands;
        self.update_bounds(parsed.bounds);
    }

    /// Set `min_intensity`/`max_intensity` to the range of the positive S
    /// values in `commands`, or to the 0..1000 default when there are none.
    fn update_intensity_range(&mut self, commands: &[GCodeCommand]) {
        let mut min_s = f32::MAX;
        let mut max_s = f32::MIN;

        for cmd in commands {
            let intensity = match cmd {
                GCodeCommand::Move { intensity, .. } => intensity.unwrap_or(0.0),
                GCodeCommand::Arc { intensity, .. } => intensity.unwrap_or(0.0),
                GCodeCommand::Dwell { .. } => continue,
            };

            if intensity > 0.0 {
                min_s = min_s.min(intensity);
                max_s = max_s.max(intensity);
            }
        }

        if min_s != f32::MAX && max_s != f32::MIN {
            self.min_intensity = min_s;
            self.max_intensity = max_s;
            debug!("Intensity range: S=[{:.1}, {:.1}]", min_s, max_s);
        } else {
            // Default values if no intensity was detected
            self.min_intensity = 0.0;
            self.max_intensity = 1000.0;
        }
    }

    /// Copy the parsed toolpath extent (padded) and rotary ranges into the
    /// visualizer's bounds fields.
    fn update_bounds(&mut self, bounds: Bounds) {
        (
            self.min_x, self.max_x, self.min_y, self.max_y, self.min_z, self.max_z,
        ) = bounds.finalize_with_padding(BOUNDS_PADDING_FACTOR);

        if bounds.has_rotary_movement() {
            let ((min_a, max_a), (min_b, max_b), (min_c, max_c)) = bounds.rotary_ranges();
            self.min_a = min_a;
            self.max_a = max_a;
            self.min_b = min_b;
            self.max_b = max_b;
            self.min_c = min_c;
            self.max_c = max_c;

            debug!(
                "Rotary bounds: A=[{:.2}°, {:.2}°], B=[{:.2}°, {:.2}°], C=[{:.2}°, {:.2}°]",
                self.min_a, self.max_a, self.min_b, self.max_b, self.min_c, self.max_c
            );
        }

        debug!(
            "Bounds: x=[{:.2}, {:.2}], y=[{:.2}, {:.2}], z=[{:.2}, {:.2}]",
            self.min_x, self.max_x, self.min_y, self.max_y, self.min_z, self.max_z
        );
    }

    /// Commands produced by the last [`Visualizer::parse_gcode`] call.
    pub fn commands(&self) -> &[GCodeCommand] {
        &self.commands
    }

    /// Obtain the color for a given intensity.
    pub fn get_color_for_intensity(&self, intensity: f32) -> (f32, f32, f32) {
        if !self.use_intensity_colors {
            return (1.0, 1.0, 0.0);
        }

        if self.max_intensity <= self.min_intensity {
            return (1.0, 1.0, 0.0);
        }

        intensity_to_color(intensity, self.max_intensity)
    }
}

impl Default for Visualizer {
    fn default() -> Self {
        Self::new()
    }
}
