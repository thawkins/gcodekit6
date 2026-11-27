//! The Spoilboard Surfacing page: a raster toolpath that flattens the spoilboard.
//!
//! Defaults: 400 x 300 mm with a 25 mm cutter, 0.5 mm deep at 40 % stepover, 1000
//! mm/min and 18000 RPM, safe Z 5. A field that cannot be
//! read shows the problem and Generate waits.
//!
//! Dependencies: `egui`, `gcodekit6-camtools` (generator), `gcodekit6-app` (translations,
//! device status), `gcodekit6-core` (units).

use gcodekit6_app::i18n::Translator;
use gcodekit6_camtools::spoilboard_surfacing::{
    SpoilboardSurfacingGenerator, SpoilboardSurfacingParameters,
};
use gcodekit6_core::units::{self, MeasurementSystem};

use super::common::{
    self, CamAction, CamError, FieldError, check, field_row, grid, heading, parse_len,
    parse_number, positive,
};

/// Field label msgids.
const WIDTH: &str = n_!("Width:");
const HEIGHT: &str = n_!("Height:");
const TOOL_DIAMETER: &str = n_!("Tool Diameter:");
const CUT_DEPTH: &str = n_!("Cut Depth:");
const STEPOVER: &str = n_!("Stepover (%):");
const FEED_RATE: &str = n_!("Feed Rate (mm/min):");
const SPINDLE_SPEED: &str = n_!("Spindle Speed (RPM):");
const SAFE_Z: &str = n_!("Safe Z Height:");
const DESCRIPTION: &str =
    n_!("Generate G-code for surfacing your CNC spoilboard to ensure a flat, level work surface.");

/// The msgids the page shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        WIDTH,
        HEIGHT,
        TOOL_DIAMETER,
        CUT_DEPTH,
        STEPOVER,
        FEED_RATE,
        SPINDLE_SPEED,
        SAFE_Z,
        DESCRIPTION,
        n_!("Spoilboard Dimensions"),
        n_!("Tool Settings"),
        n_!("Machine Settings"),
    ]
}

/// Rule: a percentage above 0 and at most 100.
fn percent(v: f64) -> Option<&'static str> {
    (v <= 0.0 || v > 100.0).then_some(common::OUT_OF_RANGE)
}

/// The page's input: lengths as typed in [`SpoilboardSurfacingParams::units`], the rest as
/// typed.
#[derive(Clone, Debug, PartialEq)]
pub struct SpoilboardSurfacingParams {
    /// The units the lengths are typed in.
    pub units: MeasurementSystem,
    /// Spoilboard width.
    pub width: String,
    /// Spoilboard height.
    pub height: String,
    /// Cutter diameter.
    pub tool_diameter: String,
    /// Feed rate (mm/min).
    pub feed_rate: String,
    /// Spindle speed (RPM).
    pub spindle_speed: String,
    /// Depth of the cut.
    pub cut_depth: String,
    /// Stepover in percent of the cutter's diameter.
    pub stepover_percent: String,
    /// Safe Z for travel.
    pub safe_z: String,
    /// Home the machine before surfacing.
    pub home_before: bool,
    /// The active device's axis count (the pane refreshes it from the device profile).
    pub num_axes: u8,
}

impl SpoilboardSurfacingParams {
    /// The initial values in `units`.
    ///
    /// # Arguments
    /// * `units` - The display units.
    pub fn new(units: MeasurementSystem) -> Self {
        let len = |mm: f64| common::format_len(mm, units);
        Self {
            units,
            width: len(400.0),
            height: len(300.0),
            tool_diameter: len(25.0),
            feed_rate: "1000".into(),
            spindle_speed: "18000".into(),
            cut_depth: len(0.5),
            stepover_percent: "40".into(),
            safe_z: len(5.0),
            home_before: false,
            num_axes: 3,
        }
    }

    /// Shows the lengths in `units` instead, converting the typed values.
    ///
    /// # Arguments
    /// * `units` - The new display units.
    pub fn set_units(&mut self, units: MeasurementSystem) {
        let from = self.units;
        for text in [
            &mut self.width,
            &mut self.height,
            &mut self.tool_diameter,
            &mut self.cut_depth,
            &mut self.safe_z,
        ] {
            common::convert_len(text, from, units);
        }
        self.units = units;
    }

    /// The generator's parameters, or every problem with the input.
    ///
    /// # Errors
    /// The fields that do not parse or are unsafe (a size, cutter, depth, feed, speed or
    /// safe Z that is not above 0, a stepover outside 0-100 %), and a device with fewer than
    /// 3 axes.
    pub fn parameters(&self) -> Result<SpoilboardSurfacingParameters, Vec<FieldError>> {
        let mut errors = Vec::new();
        let u = self.units;
        let mut len = |label, text: &str, rule: fn(f64) -> Option<&'static str>| {
            check(&mut errors, label, parse_len(text, u), rule)
        };
        let width = len(WIDTH, &self.width, positive);
        let height = len(HEIGHT, &self.height, positive);
        let tool_diameter = len(TOOL_DIAMETER, &self.tool_diameter, positive);
        let cut_depth = len(CUT_DEPTH, &self.cut_depth, positive);
        let safe_z = len(SAFE_Z, &self.safe_z, positive);
        let mut num = |label, text: &str, rule: fn(f64) -> Option<&'static str>| {
            check(&mut errors, label, parse_number(text), rule)
        };
        let stepover_percent = num(STEPOVER, &self.stepover_percent, percent);
        let feed_rate = num(FEED_RATE, &self.feed_rate, positive);
        let spindle_speed = num(SPINDLE_SPEED, &self.spindle_speed, positive);
        if self.num_axes < 3 {
            errors.push(FieldError::page(common::NEEDS_THREE_AXES));
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(SpoilboardSurfacingParameters {
            width,
            height,
            tool_diameter,
            feed_rate,
            spindle_speed,
            cut_depth,
            stepover_percent,
            safe_z,
            home_before_job: self.home_before,
        })
    }

    /// The page's input as a preset file: lengths in millimetres, feed, speed and
    /// stepover as typed (Home is not kept).
    ///
    /// # Arguments
    /// * `tr` - Translator for the current UI language.
    ///
    /// # Errors
    /// The first length that does not parse (saving is refused then).
    pub fn preset_json(&self, tr: &Translator) -> Result<String, String> {
        let u = self.units;
        let len = |label, text: &str| common::preset_len(label, text, u);
        let preset = (|| {
            Ok(serde_json::json!({
                "width": len(WIDTH, &self.width)?,
                "height": len(HEIGHT, &self.height)?,
                "tool_diameter": len(TOOL_DIAMETER, &self.tool_diameter)?,
                "feed_rate": self.feed_rate,
                "spindle_speed": self.spindle_speed,
                "cut_depth": len(CUT_DEPTH, &self.cut_depth)?,
                "stepover_percent": self.stepover_percent,
                "safe_z": len(SAFE_Z, &self.safe_z)?,
            }))
        })();
        common::preset_text(preset, tr)
    }

    /// Sets the page from a preset file ([`Self::preset_json`]); keys it
    /// does not hold leave their fields as they are.
    ///
    /// # Arguments
    /// * `json` - The file's text.
    ///
    /// # Errors
    /// The text is not a JSON object; the page is left as it was.
    pub fn apply_preset(&mut self, json: &str) -> Result<(), String> {
        let map = common::preset_object(json)?;
        let u = self.units;
        common::apply_len(&mut self.width, &map, "width", u);
        common::apply_len(&mut self.height, &map, "height", u);
        common::apply_len(&mut self.tool_diameter, &map, "tool_diameter", u);
        common::apply_text(&mut self.feed_rate, &map, "feed_rate");
        common::apply_text(&mut self.spindle_speed, &map, "spindle_speed");
        common::apply_len(&mut self.cut_depth, &map, "cut_depth", u);
        common::apply_text(&mut self.stepover_percent, &map, "stepover_percent");
        common::apply_len(&mut self.safe_z, &map, "safe_z", u);
        Ok(())
    }

    /// Generates the program.
    ///
    /// # Errors
    /// [`CamError::Invalid`] while the input has problems, else the generator's refusal.
    pub fn generate(&self) -> Result<String, CamError> {
        let params = self.parameters().map_err(|_| CamError::Invalid)?;
        SpoilboardSurfacingGenerator::new(params)
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
pub fn show(
    ui: &mut egui::Ui,
    params: &mut SpoilboardSurfacingParams,
    tr: &Translator,
) -> CamAction {
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
            heading(ui, tr, n_!("Spoilboard Dimensions"));
            grid(ui, "cam-surfacing-dimensions", |ui| {
                field_row(ui, tr, WIDTH, &mut params.width, unit, &errors);
                field_row(ui, tr, HEIGHT, &mut params.height, unit, &errors);
            });
            heading(ui, tr, n_!("Tool Settings"));
            grid(ui, "cam-surfacing-tool", |ui| {
                field_row(
                    ui,
                    tr,
                    TOOL_DIAMETER,
                    &mut params.tool_diameter,
                    unit,
                    &errors,
                );
                field_row(ui, tr, CUT_DEPTH, &mut params.cut_depth, unit, &errors);
                field_row(ui, tr, STEPOVER, &mut params.stepover_percent, "", &errors);
            });
            heading(ui, tr, n_!("Machine Settings"));
            grid(ui, "cam-surfacing-machine", |ui| {
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
