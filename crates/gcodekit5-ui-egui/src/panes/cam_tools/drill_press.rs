//! The Drill Press page: plunge, peck or helical drilling of one hole.
//!
//! GTK's defaults: a 10 mm hole with a 6 mm tool from Z 0 to -10 in 2 mm pecks, safe Z 5,
//! 100 / 500 mm/min and 10000 RPM, centred in the machine's travel (GRBL `$130` / `$131`,
//! 200 mm when unknown). GTK ran the program on the machine straight away; here it goes to
//! the editor like the other tools.
//!
//! Dependencies: `egui`, `gcodekit5-camtools` (generator), `gcodekit5-app` (translations,
//! device status), `gcodekit5-core` (units).

use gcodekit5_app::device_status;
use gcodekit5_app::i18n::Translator;
use gcodekit5_camtools::drill_press::{DrillPressGenerator, DrillPressParameters};
use gcodekit5_core::units::{self, MeasurementSystem};

use super::common::{
    self, CamAction, CamError, FieldError, any, check, field_row, grid, heading, not_negative,
    parse_len, parse_number, positive,
};

/// Field label msgids.
const HOLE_DIAMETER: &str = n_!("Hole Diameter:");
const TOOL_DIAMETER: &str = n_!("Tool Diameter:");
const CENTER_X: &str = n_!("Center X:");
const CENTER_Y: &str = n_!("Center Y:");
const TOP_Z: &str = n_!("Top Z (Surface):");
const BOTTOM_Z: &str = n_!("Bottom Z (Depth):");
const PECK_DEPTH: &str = n_!("Peck Depth (0 for none):");
const PLUNGE_RATE: &str = n_!("Plunge Rate (mm/min):");
const FEED_RATE: &str = n_!("Feed Rate (mm/min):");
const SPINDLE_SPEED: &str = n_!("Spindle Speed (RPM):");
const SAFE_Z: &str = n_!("Safe Z:");
const BELOW_TOP: &str = n_!("Must be below Top Z");
const ABOVE_TOP: &str = n_!("Must be above Top Z");
const LARGER_THAN_HOLE: &str = n_!("Must not be larger than the hole");
const LASER_MODE: &str =
    n_!("The device is in laser mode ($32=1). Switch it to CNC mode to use the Drill Press.");
const DESCRIPTION: &str = n_!(
    "Emulate a drill press on your CNC. Supports standard drilling, peck drilling for chip clearing, and helical interpolation for holes larger than the tool."
);

/// The msgids the page shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        HOLE_DIAMETER,
        TOOL_DIAMETER,
        CENTER_X,
        CENTER_Y,
        TOP_Z,
        BOTTOM_Z,
        PECK_DEPTH,
        PLUNGE_RATE,
        FEED_RATE,
        SPINDLE_SPEED,
        SAFE_Z,
        BELOW_TOP,
        ABOVE_TOP,
        LARGER_THAN_HOLE,
        LASER_MODE,
        DESCRIPTION,
        n_!("Hole Geometry"),
        n_!("Depth and Pecking"),
        n_!("Machine Settings"),
    ]
}

/// The page's input: lengths as typed in [`DrillPressParams::units`], feeds and speed as
/// typed.
#[derive(Clone, Debug, PartialEq)]
pub struct DrillPressParams {
    /// The units the lengths are typed in.
    pub units: MeasurementSystem,
    /// Hole diameter.
    pub hole_diameter: String,
    /// Tool diameter.
    pub tool_diameter: String,
    /// Hole centre X.
    pub x: String,
    /// Hole centre Y.
    pub y: String,
    /// Top Z, the material surface.
    pub top_z: String,
    /// Bottom Z, the hole's depth.
    pub bottom_z: String,
    /// Depth of each peck; 0 drills in one plunge.
    pub peck_depth: String,
    /// Plunge rate (mm/min).
    pub plunge_rate: String,
    /// Feed rate of the helical moves (mm/min).
    pub feed_rate: String,
    /// Spindle speed (RPM).
    pub spindle_speed: String,
    /// Safe Z for travel.
    pub safe_z: String,
    /// Home the machine before drilling.
    pub home_before: bool,
    /// The active device's axis count (the pane refreshes it from the device status).
    pub num_axes: u8,
    /// The device is in laser mode, GRBL `$32=1` (the pane refreshes it from the device
    /// status). The program starts the spindle with `M3` and plunges: a laser would fire in
    /// place, so GTK refused to drill.
    pub laser_mode: bool,
}

/// Whether the device status says the machine is in laser mode (GRBL `$32=1`).
pub fn device_in_laser_mode() -> bool {
    device_status::get_grbl_setting_numeric(32).unwrap_or(0.0) > 0.5
}

impl DrillPressParams {
    /// GTK's initial values in `units`, the hole centred in the machine's travel as far as
    /// the GRBL settings tell (`$130` / `$131`, 200 mm when unknown).
    ///
    /// # Arguments
    /// * `units` - The display units.
    pub fn new(units: MeasurementSystem) -> Self {
        let half =
            |setting: u16| device_status::get_grbl_setting_numeric(setting).unwrap_or(200.0) / 2.0;
        Self::with_center(units, (half(130), half(131)))
    }

    /// GTK's initial values in `units` with the hole at `center` (mm).
    ///
    /// # Arguments
    /// * `units` - The display units.
    /// * `center` - The hole centre (mm).
    pub fn with_center(units: MeasurementSystem, center: (f64, f64)) -> Self {
        let len = |mm: f64| common::format_len(mm, units);
        Self {
            units,
            hole_diameter: len(10.0),
            tool_diameter: len(6.0),
            x: len(center.0),
            y: len(center.1),
            top_z: len(0.0),
            bottom_z: len(-10.0),
            peck_depth: len(2.0),
            plunge_rate: "100".into(),
            feed_rate: "500".into(),
            spindle_speed: "10000".into(),
            safe_z: len(5.0),
            home_before: false,
            num_axes: device_status::get_active_num_axes(),
            laser_mode: device_in_laser_mode(),
        }
    }

    /// Shows the lengths in `units` instead, converting the typed values.
    ///
    /// # Arguments
    /// * `units` - The new display units.
    pub fn set_units(&mut self, units: MeasurementSystem) {
        let from = self.units;
        for text in [
            &mut self.hole_diameter,
            &mut self.tool_diameter,
            &mut self.x,
            &mut self.y,
            &mut self.top_z,
            &mut self.bottom_z,
            &mut self.peck_depth,
            &mut self.safe_z,
        ] {
            common::convert_len(text, from, units);
        }
        self.units = units;
    }

    /// The generator's parameters, or every problem with the input.
    ///
    /// # Errors
    /// The fields that do not parse or break the generator's rules (positive diameters and
    /// rates, a bottom below the top, a safe Z above it, a peck depth that is not
    /// negative), and a device with fewer than 3 axes.
    pub fn parameters(&self) -> Result<DrillPressParameters, Vec<FieldError>> {
        let mut errors = Vec::new();
        let u = self.units;
        let mut len = |label, text: &str, rule: fn(f64) -> Option<&'static str>| {
            check(&mut errors, label, parse_len(text, u), rule)
        };
        let hole_diameter = len(HOLE_DIAMETER, &self.hole_diameter, positive);
        let tool_diameter = len(TOOL_DIAMETER, &self.tool_diameter, positive);
        let top_z = len(TOP_Z, &self.top_z, any);
        let bottom_z = len(BOTTOM_Z, &self.bottom_z, any);
        let peck_depth = len(PECK_DEPTH, &self.peck_depth, not_negative);
        let safe_z = len(SAFE_Z, &self.safe_z, any);
        let x = len(CENTER_X, &self.x, any);
        let y = len(CENTER_Y, &self.y, any);
        let mut num = |label, text: &str, rule: fn(f64) -> Option<&'static str>| {
            check(&mut errors, label, parse_number(text), rule)
        };
        let plunge_rate = num(PLUNGE_RATE, &self.plunge_rate, positive);
        let feed_rate = num(FEED_RATE, &self.feed_rate, positive);
        let spindle_speed = num(SPINDLE_SPEED, &self.spindle_speed, positive);
        let parsed = |label| !errors.iter().any(|e: &FieldError| e.field == Some(label));
        let (top_ok, bottom_ok, safe_ok) = (parsed(TOP_Z), parsed(BOTTOM_Z), parsed(SAFE_Z));
        let (hole_ok, tool_ok) = (parsed(HOLE_DIAMETER), parsed(TOOL_DIAMETER));
        if hole_ok && tool_ok && tool_diameter > hole_diameter {
            errors.push(FieldError::field(TOOL_DIAMETER, LARGER_THAN_HOLE));
        }
        if top_ok && bottom_ok && bottom_z >= top_z {
            errors.push(FieldError::field(BOTTOM_Z, BELOW_TOP));
        }
        if top_ok && safe_ok && safe_z <= top_z {
            errors.push(FieldError::field(SAFE_Z, ABOVE_TOP));
        }
        if self.laser_mode {
            errors.push(FieldError::page(LASER_MODE));
        }
        if self.num_axes < 3 {
            errors.push(FieldError::page(common::NEEDS_THREE_AXES));
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(DrillPressParameters {
            hole_diameter,
            tool_diameter,
            top_z,
            bottom_z,
            peck_depth,
            plunge_rate,
            feed_rate,
            spindle_speed,
            safe_z,
            x,
            y,
            home_before_job: self.home_before,
        })
    }

    /// The page's input as GTK's preset file: [`DrillPressParameters`] as serde writes it,
    /// lengths in millimetres.
    ///
    /// # Arguments
    /// * `tr` - Translator for the current UI language.
    ///
    /// # Errors
    /// The first field that does not parse (GTK refused to save then too).
    pub fn preset_json(&self, tr: &Translator) -> Result<String, String> {
        let u = self.units;
        let len = |label, text: &str| common::preset_len(label, text, u).map(f64::from);
        let num = common::preset_number;
        let preset = (|| {
            Ok(DrillPressParameters {
                hole_diameter: len(HOLE_DIAMETER, &self.hole_diameter)?,
                tool_diameter: len(TOOL_DIAMETER, &self.tool_diameter)?,
                top_z: len(TOP_Z, &self.top_z)?,
                bottom_z: len(BOTTOM_Z, &self.bottom_z)?,
                peck_depth: len(PECK_DEPTH, &self.peck_depth)?,
                plunge_rate: num(PLUNGE_RATE, &self.plunge_rate)?,
                feed_rate: num(FEED_RATE, &self.feed_rate)?,
                spindle_speed: num(SPINDLE_SPEED, &self.spindle_speed)?,
                safe_z: len(SAFE_Z, &self.safe_z)?,
                x: len(CENTER_X, &self.x)?,
                y: len(CENTER_Y, &self.y)?,
                home_before_job: self.home_before,
            })
        })();
        common::preset_text(preset, tr)
    }

    /// Sets the page from a preset file in GTK's format ([`DrillPressParams::preset_json`]).
    ///
    /// # Arguments
    /// * `json` - The file's text.
    ///
    /// # Errors
    /// The text is not a Drill Press preset; the page is left as it was.
    pub fn apply_preset(&mut self, json: &str) -> Result<(), String> {
        let p: DrillPressParameters = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let len = |mm: f64| common::format_len(mm, self.units);
        self.hole_diameter = len(p.hole_diameter);
        self.tool_diameter = len(p.tool_diameter);
        self.top_z = len(p.top_z);
        self.bottom_z = len(p.bottom_z);
        self.peck_depth = len(p.peck_depth);
        self.plunge_rate = p.plunge_rate.to_string();
        self.feed_rate = p.feed_rate.to_string();
        self.spindle_speed = p.spindle_speed.to_string();
        self.safe_z = len(p.safe_z);
        self.x = len(p.x);
        self.y = len(p.y);
        self.home_before = p.home_before_job;
        Ok(())
    }

    /// Generates the program.
    ///
    /// # Errors
    /// [`CamError::Invalid`] while the input has problems, else the generator's refusal.
    pub fn generate(&self) -> Result<String, CamError> {
        let params = self.parameters().map_err(|_| CamError::Invalid)?;
        DrillPressGenerator::new(params)
            .generate()
            .map_err(CamError::generator)
    }
}

/// Draws the page.
///
/// # Arguments
/// * `ui` - The page's `Ui`.
/// * `params` - The page's input.
/// * `tr` - Translator for the current UI language.
///
/// # Returns
/// The action button pressed, if any; never one while the input has problems.
pub fn show(ui: &mut egui::Ui, params: &mut DrillPressParams, tr: &Translator) -> CamAction {
    let errors = params.parameters().err().unwrap_or_default();
    common::cam_page(
        ui,
        common::SIDEBAR_SHARE,
        params,
        |ui, _| {
            ui.label(tr.tr(DESCRIPTION));
        },
        |ui, params| {
            let unit = units::get_unit_label(params.units);
            heading(ui, tr, n_!("Hole Geometry"));
            grid(ui, "cam-drill-hole", |ui| {
                field_row(
                    ui,
                    tr,
                    HOLE_DIAMETER,
                    &mut params.hole_diameter,
                    unit,
                    &errors,
                );
                field_row(
                    ui,
                    tr,
                    TOOL_DIAMETER,
                    &mut params.tool_diameter,
                    unit,
                    &errors,
                );
                field_row(ui, tr, CENTER_X, &mut params.x, unit, &errors);
                field_row(ui, tr, CENTER_Y, &mut params.y, unit, &errors);
            });
            heading(ui, tr, n_!("Depth and Pecking"));
            grid(ui, "cam-drill-depth", |ui| {
                field_row(ui, tr, TOP_Z, &mut params.top_z, unit, &errors);
                field_row(ui, tr, BOTTOM_Z, &mut params.bottom_z, unit, &errors);
                field_row(ui, tr, PECK_DEPTH, &mut params.peck_depth, unit, &errors);
            });
            heading(ui, tr, n_!("Machine Settings"));
            grid(ui, "cam-drill-machine", |ui| {
                field_row(ui, tr, PLUNGE_RATE, &mut params.plunge_rate, "", &errors);
                field_row(ui, tr, FEED_RATE, &mut params.feed_rate, "", &errors);
                field_row(
                    ui,
                    tr,
                    SPINDLE_SPEED,
                    &mut params.spindle_speed,
                    "",
                    &errors,
                );
                field_row(ui, tr, SAFE_Z, &mut params.safe_z, unit, &errors);
                common::check_row(
                    ui,
                    tr,
                    n_!("Home Device Before Start"),
                    &mut params.home_before,
                );
            });
            common::page_errors(ui, tr, &errors);
            CamAction::None
        },
        |ui, _| common::gcode_actions(ui, tr, errors.is_empty()),
    )
}
