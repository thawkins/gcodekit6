//! The Spoilboard Grid page: a laser-engraved alignment grid.
//!
//! GTK's defaults: 400 x 300 mm with lines every 10 mm, power S1000 at 1000 mm/min in M4
//! (dynamic power). GTK silently used a default for a field it could not read; here the
//! field shows the problem and Generate waits.
//!
//! Dependencies: `egui`, `gcodekit5-camtools` (generator), `gcodekit5-app` (translations),
//! `gcodekit5-core` (units).

use gcodekit5_app::i18n::Translator;
use gcodekit5_camtools::spoilboard_grid::{SpoilboardGridGenerator, SpoilboardGridParameters};
use gcodekit5_core::units::{self, MeasurementSystem};

use super::common::{
    self, CamAction, CamError, FieldError, check, field_row, grid, heading, not_negative,
    parse_len, parse_number, positive,
};

/// Field label msgids.
const WIDTH: &str = n_!("Width:");
const HEIGHT: &str = n_!("Height:");
const GRID_SPACING: &str = n_!("Grid Spacing:");
const LASER_POWER: &str = n_!("Laser Power (S):");
const FEED_RATE: &str = n_!("Feed Rate (mm/min):");
const LASER_MODE: &str = n_!("Laser Mode:");
const DESCRIPTION: &str =
    n_!("Create a grid pattern on your spoilboard for easy workpiece alignment and fixturing.");

/// The msgids the page shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        WIDTH,
        HEIGHT,
        GRID_SPACING,
        LASER_POWER,
        FEED_RATE,
        LASER_MODE,
        DESCRIPTION,
        LaserMode::M3.msgid(),
        LaserMode::M4.msgid(),
        n_!("Spoilboard Dimensions"),
        n_!("Laser Settings"),
    ]
}

/// The laser's power mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaserMode {
    /// `M3`: constant power.
    M3,
    /// `M4`: power follows the speed.
    M4,
}

impl LaserMode {
    /// The G-code word.
    pub fn word(self) -> &'static str {
        match self {
            LaserMode::M3 => "M3",
            LaserMode::M4 => "M4",
        }
    }

    /// The msgid of the mode's name.
    pub fn msgid(self) -> &'static str {
        match self {
            LaserMode::M3 => n_!("M3 (Constant Power)"),
            LaserMode::M4 => n_!("M4 (Dynamic Power)"),
        }
    }
}

/// The page's input: lengths as typed in [`SpoilboardGridParams::units`], the rest as typed.
#[derive(Clone, Debug, PartialEq)]
pub struct SpoilboardGridParams {
    /// The units the lengths are typed in.
    pub units: MeasurementSystem,
    /// Spoilboard width.
    pub width: String,
    /// Spoilboard height.
    pub height: String,
    /// Distance between the grid lines.
    pub grid_spacing: String,
    /// Feed rate (mm/min).
    pub feed_rate: String,
    /// Laser power (S value).
    pub laser_power: String,
    /// The laser's power mode.
    pub laser_mode: LaserMode,
    /// Home the machine before engraving.
    pub home_before: bool,
}

impl SpoilboardGridParams {
    /// GTK's initial values in `units`.
    ///
    /// # Arguments
    /// * `units` - The display units.
    pub fn new(units: MeasurementSystem) -> Self {
        let len = |mm: f64| common::format_len(mm, units);
        Self {
            units,
            width: len(400.0),
            height: len(300.0),
            grid_spacing: len(10.0),
            feed_rate: "1000".into(),
            laser_power: "1000".into(),
            laser_mode: LaserMode::M4,
            home_before: false,
        }
    }

    /// Shows the lengths in `units` instead, converting the typed values.
    ///
    /// # Arguments
    /// * `units` - The new display units.
    pub fn set_units(&mut self, units: MeasurementSystem) {
        let from = self.units;
        for text in [&mut self.width, &mut self.height, &mut self.grid_spacing] {
            common::convert_len(text, from, units);
        }
        self.units = units;
    }

    /// The generator's parameters, or every problem with the input.
    ///
    /// # Errors
    /// The fields that do not parse or are unsafe (a size, spacing or feed that is not above
    /// 0, a negative power).
    pub fn parameters(&self) -> Result<SpoilboardGridParameters, Vec<FieldError>> {
        let mut errors = Vec::new();
        let u = self.units;
        let mut len = |label, text: &str| check(&mut errors, label, parse_len(text, u), positive);
        let width = len(WIDTH, &self.width);
        let height = len(HEIGHT, &self.height);
        let grid_spacing = len(GRID_SPACING, &self.grid_spacing);
        let mut num = |label, text: &str, rule: fn(f64) -> Option<&'static str>| {
            check(&mut errors, label, parse_number(text), rule)
        };
        let laser_power = num(LASER_POWER, &self.laser_power, not_negative);
        let feed_rate = num(FEED_RATE, &self.feed_rate, positive);
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(SpoilboardGridParameters {
            width,
            height,
            grid_spacing,
            feed_rate,
            laser_power,
            laser_mode: self.laser_mode.word().to_string(),
            home_before_job: self.home_before,
        })
    }

    /// The page's input as GTK's preset file: lengths in millimetres, feed and power as
    /// typed, the laser mode's word (GTK's keys; Home is not kept).
    ///
    /// # Arguments
    /// * `tr` - Translator for the current UI language.
    ///
    /// # Errors
    /// The first length that does not parse (GTK refused to save then too).
    pub fn preset_json(&self, tr: &Translator) -> Result<String, String> {
        let u = self.units;
        let len = |label, text: &str| common::preset_len(label, text, u);
        let preset = (|| {
            Ok(serde_json::json!({
                "width": len(WIDTH, &self.width)?,
                "height": len(HEIGHT, &self.height)?,
                "grid_spacing": len(GRID_SPACING, &self.grid_spacing)?,
                "feed_rate": self.feed_rate,
                "laser_power": self.laser_power,
                "laser_mode": self.laser_mode.word(),
            }))
        })();
        common::preset_text(preset, tr)
    }

    /// Sets the page from a preset file in GTK's format ([`Self::preset_json`]); keys it
    /// does not hold, and a laser mode other than `M3` / `M4`, leave their fields as they
    /// are.
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
        common::apply_len(&mut self.grid_spacing, &map, "grid_spacing", u);
        common::apply_text(&mut self.feed_rate, &map, "feed_rate");
        common::apply_text(&mut self.laser_power, &map, "laser_power");
        match map.get("laser_mode").and_then(serde_json::Value::as_str) {
            Some("M3") => self.laser_mode = LaserMode::M3,
            Some("M4") => self.laser_mode = LaserMode::M4,
            Some(_) | None => {}
        }
        Ok(())
    }

    /// Generates the program.
    ///
    /// # Errors
    /// [`CamError::Invalid`] while the input has problems, else the generator's refusal.
    pub fn generate(&self) -> Result<String, CamError> {
        let params = self.parameters().map_err(|_| CamError::Invalid)?;
        SpoilboardGridGenerator::new(params)
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
pub fn show(ui: &mut egui::Ui, params: &mut SpoilboardGridParams, tr: &Translator) -> CamAction {
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
            grid(ui, "cam-grid-dimensions", |ui| {
                field_row(ui, tr, WIDTH, &mut params.width, unit, &errors);
                field_row(ui, tr, HEIGHT, &mut params.height, unit, &errors);
                field_row(
                    ui,
                    tr,
                    GRID_SPACING,
                    &mut params.grid_spacing,
                    unit,
                    &errors,
                );
            });
            heading(ui, tr, n_!("Laser Settings"));
            grid(ui, "cam-grid-laser", |ui| {
                field_row(ui, tr, LASER_POWER, &mut params.laser_power, "", &errors);
                field_row(ui, tr, FEED_RATE, &mut params.feed_rate, "", &errors);
                let name = ui.label(tr.tr(LASER_MODE));
                egui::ComboBox::from_id_salt("cam-grid-laser-mode")
                    .selected_text(tr.tr(params.laser_mode.msgid()))
                    .show_ui(ui, |ui| {
                        for mode in [LaserMode::M3, LaserMode::M4] {
                            ui.selectable_value(&mut params.laser_mode, mode, tr.tr(mode.msgid()));
                        }
                    })
                    .response
                    .labelled_by(name.id);
                ui.end_row();
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
