//! The Gerber to G-Code page: isolation routing, rubout, outline and drill cuts for one layer
//! of a PCB project.
//!
//! Browse picks a project folder through the non-blocking folder dialog; the page finds its
//! layer files by their names and generates the layer chosen in a drop-down. When
//! no folder or layer is found, that is shown on the page and
//! the actions wait, as they do for a field that does not parse. Defaults: the top
//! copper of a 100 x 100 mm board, a 0.1 mm tool at 100 mm/min and 10000 RPM, a cut depth of
//! -0.1 mm below a safe Z of 5 mm. The generator checks nothing, so the page's limits are
//! its own: feed, speed, tool and board above 0, a cut depth below the surface and a safe Z
//! above it (on a device with a Z axis), and for alignment holes a diameter above 0, a margin
//! that is not negative and a depth below the surface. The Board Outline layer is cut
//! outside the board edge by the tool radius, in passes of at most the outline step-down
//! (default 0.5 mm, above 0 on a device with a Z axis) down to the cut depth. The Drill
//! Holes layer reads an Excellon drill file and drills every hole to the alignment holes'
//! drill depth (below the surface), one tool after the other with a pause to change tools.
//!
//! Dependencies: `egui`, `gcodekit5-camtools` (converter, layer detection), `gcodekit5-app`
//! (translations), `gcodekit5-core` (units), `serde_json` (presets).

mod preset;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use gcodekit5_app::i18n::Translator;
use gcodekit5_camtools::gerber::{
    GerberConverter, GerberLayerType, GerberParameters, UnsupportedConstructs, detect_layer_files,
    unsupported_constructs,
};
use gcodekit5_core::units::{self, MeasurementSystem};

use super::common::{
    self, CamAction, CamError, FieldError, any, check, check_row, combo_row, field_row, gated,
    grid, heading, not_negative, parse_len, parse_number, positive,
};

/// Field label msgids.
const LAYER_TYPE: &str = n_!("Layer Type:");
const BOARD_WIDTH: &str = n_!("Board Width:");
const BOARD_HEIGHT: &str = n_!("Board Height:");
const OFFSET_X: &str = n_!("Offset X:");
const OFFSET_Y: &str = n_!("Offset Y:");
const FEED_RATE: &str = n_!("Feed Rate:");
const SPINDLE_SPEED: &str = n_!("Spindle Speed (RPM):");
const CUT_DEPTH: &str = n_!("Cut Depth:");
const OUTLINE_STEP_DOWN: &str = n_!("Outline Step Down:");
const SAFE_Z: &str = n_!("Safe Z:");
const TOOL_DIAMETER: &str = n_!("Tool Diameter:");
const ISOLATION_WIDTH: &str = n_!("Isolation Width:");
const RUBOUT: &str = n_!("Remove Excess Copper:");
const USE_OUTLINE: &str = n_!("Use Board Outline for Rubout:");
const ALIGNMENT_HOLES: &str = n_!("Generate Alignment Holes:");
const HOLE_DIAMETER: &str = n_!("Hole Diameter:");
const HOLE_MARGIN: &str = n_!("Margin:");
const HOLE_DEPTH: &str = n_!("Drill Depth (Z):");
const DESCRIPTION: &str = n_!(
    "Convert standard Gerber files (PCB layers) into G-Code for CNC isolation routing. \
     Supports copper clearing (rubout), alignment holes, and custom tool parameters."
);

/// Folder row and page messages.
const NO_DIRECTORY_SELECTED: &str = n_!("No directory selected");
const LAYER_NOT_FOUND: &str = n_!("Layer not found in directory");
const SELECT_A_DIRECTORY: &str = n_!("Please select a Gerber directory.");
const SELECTED_LAYER_MISSING: &str = n_!("Selected layer not found in directory.");
const BELOW_SURFACE: &str = n_!("Must be below the surface (negative)");
const READ_FAILED: &str = n_!("Failed to read file");
const UNSUPPORTED: &str = n_!("Unsupported Gerber content is left out of the program");

/// The layer types and the msgids of their names, in menu order.
const LAYER_TYPES: [(GerberLayerType, &str); 8] = [
    (GerberLayerType::TopCopper, n_!("Top Copper")),
    (GerberLayerType::BottomCopper, n_!("Bottom Copper")),
    (GerberLayerType::TopSolderMask, n_!("Top Solder Mask")),
    (GerberLayerType::BottomSolderMask, n_!("Bottom Solder Mask")),
    (GerberLayerType::TopScreenPrint, n_!("Top Screen Print")),
    (
        GerberLayerType::BottomScreenPrint,
        n_!("Bottom Screen Print"),
    ),
    (GerberLayerType::DrillHoles, n_!("Drill Holes")),
    (GerberLayerType::BoardOutline, n_!("Board Outline")),
];

/// The msgids the page shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        LAYER_TYPE,
        BOARD_WIDTH,
        BOARD_HEIGHT,
        OFFSET_X,
        OFFSET_Y,
        FEED_RATE,
        SPINDLE_SPEED,
        CUT_DEPTH,
        OUTLINE_STEP_DOWN,
        SAFE_Z,
        TOOL_DIAMETER,
        ISOLATION_WIDTH,
        RUBOUT,
        USE_OUTLINE,
        ALIGNMENT_HOLES,
        HOLE_DIAMETER,
        HOLE_MARGIN,
        HOLE_DEPTH,
        DESCRIPTION,
        NO_DIRECTORY_SELECTED,
        LAYER_NOT_FOUND,
        SELECT_A_DIRECTORY,
        SELECTED_LAYER_MISSING,
        BELOW_SURFACE,
        READ_FAILED,
        UNSUPPORTED,
        n_!("Gerber Project"),
        n_!("Select Directory"),
        n_!("Browse..."),
        n_!("Parameters"),
        n_!("Alignment Holes"),
        n_!("Negative Z for the alignment holes (through the board)"),
    ];
    ids.extend(LAYER_TYPES.iter().map(|(_, msgid)| *msgid));
    ids
}

/// The page's input: lengths as typed in [`GerberParams::units`], the rest as typed, and the
/// project folder with the layer files found in it.
#[derive(Clone, Debug, PartialEq)]
pub struct GerberParams {
    /// The units the lengths are typed in.
    pub units: MeasurementSystem,
    /// The layer to convert.
    pub layer_type: GerberLayerType,
    /// Feed rate (mm/min).
    pub feed_rate: String,
    /// Spindle speed (RPM).
    pub spindle_speed: String,
    /// Board width.
    pub board_width: String,
    /// Board height.
    pub board_height: String,
    /// Work origin offset X.
    pub offset_x: String,
    /// Work origin offset Y.
    pub offset_y: String,
    /// Drill alignment holes around the board.
    pub generate_alignment_holes: bool,
    /// Alignment hole diameter.
    pub alignment_hole_diameter: String,
    /// Distance of the alignment holes from the board.
    pub alignment_hole_margin: String,
    /// Z of the alignment holes (negative is below the surface).
    pub alignment_hole_depth: String,
    /// Z of the isolation cut (negative is below the surface).
    pub cut_depth: String,
    /// Board Outline layer: the most each pass of the outline cut goes down.
    pub outline_step_down: String,
    /// Z of the moves between cuts.
    pub safe_z: String,
    /// Tool diameter.
    pub tool_diameter: String,
    /// Isolation width beyond the tool's radius.
    pub isolation_width: String,
    /// Remove the excess copper.
    pub rubout: bool,
    /// Limit the rubout to the board outline's layer.
    pub use_board_outline: bool,
    /// The project folder, once chosen.
    pub directory: Option<PathBuf>,
    /// The layer files found in [`GerberParams::directory`].
    pub layer_files: HashMap<GerberLayerType, PathBuf>,
    /// The constructs of each layer file in [`GerberParams::layer_files`] that the converter
    /// does not draw; a layer whose file has none is absent.
    pub unsupported: HashMap<GerberLayerType, UnsupportedConstructs>,
    /// The active device's axes (the pane keeps this up to date).
    pub num_axes: u8,
}

impl GerberParams {
    /// The initial values in `units`, with no project folder.
    ///
    /// # Arguments
    /// * `units` - The display units.
    pub fn new(units: MeasurementSystem) -> Self {
        let len = |mm: f64| common::format_len(mm, units);
        Self {
            units,
            layer_type: GerberLayerType::TopCopper,
            feed_rate: "100.0".into(),
            spindle_speed: "10000.0".into(),
            board_width: len(100.0),
            board_height: len(100.0),
            offset_x: len(0.0),
            offset_y: len(0.0),
            generate_alignment_holes: false,
            alignment_hole_diameter: len(3.175),
            alignment_hole_margin: len(5.0),
            alignment_hole_depth: len(-1.8),
            cut_depth: len(-0.1),
            outline_step_down: len(0.5),
            safe_z: len(5.0),
            tool_diameter: len(0.1),
            isolation_width: len(0.0),
            rubout: false,
            use_board_outline: false,
            directory: None,
            layer_files: HashMap::new(),
            unsupported: HashMap::new(),
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
            &mut self.board_width,
            &mut self.board_height,
            &mut self.offset_x,
            &mut self.offset_y,
            &mut self.alignment_hole_diameter,
            &mut self.alignment_hole_margin,
            &mut self.alignment_hole_depth,
            &mut self.cut_depth,
            &mut self.outline_step_down,
            &mut self.safe_z,
            &mut self.tool_diameter,
            &mut self.isolation_width,
        ] {
            common::convert_len(text, from, units);
        }
        self.units = units;
    }

    /// Takes `directory` as the project folder, finds its layer files and counts the
    /// constructs of each that the converter does not draw.
    ///
    /// # Arguments
    /// * `directory` - The folder.
    pub fn set_directory(&mut self, directory: &Path) {
        self.layer_files = detect_layer_files(directory);
        self.unsupported = self
            .layer_files
            .iter()
            .filter(|(layer, _)| **layer != GerberLayerType::DrillHoles)
            .filter_map(|(layer, path)| {
                let counts = unsupported_constructs(&std::fs::read_to_string(path).ok()?);
                (!counts.is_empty()).then_some((*layer, counts))
            })
            .collect();
        self.directory = Some(directory.to_path_buf());
    }

    /// The file of the chosen layer, if the folder has one.
    pub fn layer_file(&self) -> Option<&PathBuf> {
        self.layer_files.get(&self.layer_type)
    }

    /// The converter's parameters, or every problem with the input.
    ///
    /// # Errors
    /// No folder with layers, no file for the chosen layer, and the fields that do not
    /// parse or are unsafe.
    pub fn parameters(&self) -> Result<GerberParameters, Vec<FieldError>> {
        self.build(true)
    }

    /// The parameters; with `strict` false only the fields' syntax is checked (a preset).
    fn build(&self, strict: bool) -> Result<GerberParameters, Vec<FieldError>> {
        let mut errors = Vec::new();
        let u = self.units;
        let has_z = self.num_axes >= 3;
        let holes = self.generate_alignment_holes;
        let mut len = |label, text: &str, on: bool, rule: &dyn Fn(f64) -> Option<&'static str>| {
            check(
                &mut errors,
                label,
                parse_len(text, u),
                gated(strict && on, rule),
            )
        };
        let below = |v: f64| ((v as f32) >= 0.0).then_some(BELOW_SURFACE);
        let board_width = len(BOARD_WIDTH, &self.board_width, true, &positive);
        let board_height = len(BOARD_HEIGHT, &self.board_height, true, &positive);
        let offset_x = len(OFFSET_X, &self.offset_x, true, &any);
        let offset_y = len(OFFSET_Y, &self.offset_y, true, &any);
        let cut_depth = len(CUT_DEPTH, &self.cut_depth, has_z, &below);
        let outline = self.layer_type == GerberLayerType::BoardOutline;
        let outline_step_down = len(
            OUTLINE_STEP_DOWN,
            &self.outline_step_down,
            outline && has_z,
            &positive,
        );
        let safe_z = len(SAFE_Z, &self.safe_z, has_z, &positive);
        let tool_diameter = len(TOOL_DIAMETER, &self.tool_diameter, true, &positive);
        let isolation_width = len(ISOLATION_WIDTH, &self.isolation_width, true, &not_negative);
        let hole_diameter = len(
            HOLE_DIAMETER,
            &self.alignment_hole_diameter,
            holes,
            &positive,
        );
        let hole_margin = len(
            HOLE_MARGIN,
            &self.alignment_hole_margin,
            holes,
            &not_negative,
        );
        // The Drill Holes layer drills to the same depth as the alignment holes.
        let drill = self.layer_type == GerberLayerType::DrillHoles;
        let hole_depth = len(
            HOLE_DEPTH,
            &self.alignment_hole_depth,
            (holes || drill) && has_z,
            &below,
        );
        let mut num = |label, text: &str| {
            check(
                &mut errors,
                label,
                parse_number(text),
                gated(strict, positive),
            )
        };
        let feed_rate = num(FEED_RATE, &self.feed_rate);
        let spindle_speed = num(SPINDLE_SPEED, &self.spindle_speed);
        if strict {
            if self.layer_files.is_empty() {
                errors.push(FieldError::page(SELECT_A_DIRECTORY));
            } else if self.layer_file().is_none() {
                errors.push(FieldError::page(SELECTED_LAYER_MISSING));
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(GerberParameters {
            layer_type: self.layer_type,
            feed_rate: feed_rate as f32,
            spindle_speed: spindle_speed as f32,
            board_width: board_width as f32,
            board_height: board_height as f32,
            offset_x: offset_x as f32,
            offset_y: offset_y as f32,
            generate_alignment_holes: holes,
            alignment_hole_diameter: hole_diameter as f32,
            alignment_hole_margin: hole_margin as f32,
            alignment_hole_depth: hole_depth as f32,
            cut_depth: cut_depth as f32,
            outline_step_down: outline_step_down as f32,
            safe_z: safe_z as f32,
            tool_diameter: tool_diameter as f32,
            isolation_width: isolation_width as f32,
            rubout: self.rubout,
            use_board_outline: self.use_board_outline,
            directory: self
                .directory
                .as_ref()
                .map(|d| d.to_string_lossy().to_string()),
            num_axes: self.num_axes,
        })
    }

    /// Makes the program for the chosen layer.
    ///
    /// # Errors
    /// [`CamError::Invalid`] while the input has problems; the layer file's read error; else
    /// the converter's refusal.
    pub fn generate(&self, tr: &Translator) -> Result<String, CamError> {
        let params = self.parameters().map_err(|_| CamError::Invalid)?;
        let Some(path) = self.layer_file() else {
            return Err(CamError::Invalid);
        };
        let content = std::fs::read_to_string(path)
            .map_err(|e| CamError::Generator(format!("{}: {e}", tr.tr(READ_FAILED))))?;
        GerberConverter::generate(&params, &content).map_err(CamError::generator)
    }
}

/// The last component of `path` as text.
fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().to_string(),
    )
}

/// Draws the page.
///
/// # Arguments
/// * `ui` - The page's `Ui`.
/// * `params` - The page's input.
/// * `tr` - Translator for the current UI language.
///
/// # Returns
/// The action button pressed, if any; never a generating one while the input has problems.
pub fn show(ui: &mut egui::Ui, params: &mut GerberParams, tr: &Translator) -> CamAction {
    let errors = params.parameters().err().unwrap_or_default();
    common::cam_page(
        ui,
        common::SIDEBAR_SHARE,
        params,
        |ui, _| {
            ui.label(tr.tr(DESCRIPTION));
        },
        |ui, params| {
            let mut action = CamAction::None;
            heading(ui, tr, n_!("Gerber Project"));
            ui.horizontal_wrapped(|ui| {
                ui.label(tr.tr(n_!("Select Directory")));
                if crate::icons::icon_button(ui, crate::icons::Icon::Open, &tr.tr(n_!("Browse...")))
                    .clicked()
                {
                    action = CamAction::ChooseFolder;
                }
                match &params.directory {
                    Some(dir) => ui.label(file_name(dir)),
                    None => ui.weak(tr.tr(NO_DIRECTORY_SELECTED)),
                };
            });
            if params.directory.is_some() && !params.layer_files.is_empty() {
                match params.layer_file() {
                    Some(file) => ui.weak(file_name(file)),
                    None => ui.weak(tr.tr(LAYER_NOT_FOUND)),
                };
                if let Some(counts) = params.unsupported.get(&params.layer_type) {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        format!("{}: {counts}", tr.tr(UNSUPPORTED)),
                    );
                }
            }
            show_fields(ui, params, tr, &errors);
            common::page_errors(ui, tr, &errors);
            action
        },
        |ui, _| {
            common::action_row(
                ui,
                tr,
                errors.is_empty(),
                &[
                    (n_!("Generate"), CamAction::Generate),
                    (n_!("Send to Editor"), CamAction::SendToEditor),
                    (n_!("Preview"), CamAction::Preview),
                ],
            )
        },
    )
}

/// The page's groups of fields.
fn show_fields(ui: &mut egui::Ui, p: &mut GerberParams, tr: &Translator, errors: &[FieldError]) {
    let unit = units::get_unit_label(p.units);
    heading(ui, tr, n_!("Parameters"));
    grid(ui, "cam-gerber-parameters", |ui| {
        combo_row(
            ui,
            tr,
            "cam-gerber-layer",
            LAYER_TYPE,
            &mut p.layer_type,
            &LAYER_TYPES,
        );
        field_row(ui, tr, BOARD_WIDTH, &mut p.board_width, unit, errors);
        field_row(ui, tr, BOARD_HEIGHT, &mut p.board_height, unit, errors);
        field_row(ui, tr, OFFSET_X, &mut p.offset_x, unit, errors);
        field_row(ui, tr, OFFSET_Y, &mut p.offset_y, unit, errors);
        field_row(ui, tr, FEED_RATE, &mut p.feed_rate, "", errors);
        field_row(ui, tr, SPINDLE_SPEED, &mut p.spindle_speed, "", errors);
        field_row(ui, tr, CUT_DEPTH, &mut p.cut_depth, unit, errors);
        field_row(
            ui,
            tr,
            OUTLINE_STEP_DOWN,
            &mut p.outline_step_down,
            unit,
            errors,
        );
        field_row(ui, tr, SAFE_Z, &mut p.safe_z, unit, errors);
        field_row(ui, tr, TOOL_DIAMETER, &mut p.tool_diameter, unit, errors);
        field_row(
            ui,
            tr,
            ISOLATION_WIDTH,
            &mut p.isolation_width,
            unit,
            errors,
        );
        check_row(ui, tr, RUBOUT, &mut p.rubout);
        check_row(ui, tr, USE_OUTLINE, &mut p.use_board_outline);
    });
    heading(ui, tr, n_!("Alignment Holes"));
    grid(ui, "cam-gerber-holes", |ui| {
        check_row(ui, tr, ALIGNMENT_HOLES, &mut p.generate_alignment_holes);
        field_row(
            ui,
            tr,
            HOLE_DIAMETER,
            &mut p.alignment_hole_diameter,
            unit,
            errors,
        );
        field_row(
            ui,
            tr,
            HOLE_MARGIN,
            &mut p.alignment_hole_margin,
            unit,
            errors,
        );
        field_row(
            ui,
            tr,
            HOLE_DEPTH,
            &mut p.alignment_hole_depth,
            unit,
            errors,
        );
    });
    ui.weak(tr.tr(n_!(
        "Negative Z for the alignment holes (through the board)"
    )));
}
