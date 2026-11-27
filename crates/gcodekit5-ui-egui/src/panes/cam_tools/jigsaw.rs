//! The Jigsaw Puzzle Generator page: a puzzle of interlocking pieces, made as outlines for
//! the designer.
//!
//! GTK's defaults: 200 x 150 mm, 4 x 3 pieces, seed 42, tabs 20 %, jitter 4 % and a 2 mm
//! corner radius. The generator makes paths, not a program, so Generate adds them to the
//! design (grouped, the border closed) and brings the designer to the front, as GTK's did.
//! GTK silently used a default for a field it could not read; here the field shows the
//! problem and Generate waits. The generator's limits are checked at the fields: pieces of
//! at least 15 mm, 2 to 20 of them in each direction, a puzzle of 50 to 2000 mm, tabs of
//! 10 to 30 %, jitter of 0 to 13 %, a corner radius of 0 to 10 mm.
//!
//! Dependencies: `egui`, `gcodekit5-camtools` (generator), `gcodekit5-app` (translations),
//! `gcodekit5-core` (units), `serde_json` (presets).

use gcodekit5_app::i18n::Translator;
use gcodekit5_camtools::jigsaw_puzzle::{
    JigsawPuzzleMaker, MAX_CORNER_RADIUS_MM, MAX_JITTER_PERCENT, MAX_PIECES, MAX_TAB_SIZE_PERCENT,
    MIN_PIECE_MM, MIN_PIECES, MIN_SIZE_MM, MIN_TAB_SIZE_PERCENT, PuzzleParameters,
};
use gcodekit5_core::units::{self, MeasurementSystem};

use super::common::{
    self, CamAction, CamError, FieldError, at_least, between, check, field_row, gated, grid,
    heading, parse_len, parse_number, parse_whole,
};
use super::requests::{DesignerPath, DesignerPaths};

/// Field label msgids.
const WIDTH: &str = n_!("Width:");
const HEIGHT: &str = n_!("Height:");
const CORNER_RADIUS: &str = n_!("Corner Radius:");
const PIECES_ACROSS: &str = n_!("Pieces Across:");
const PIECES_DOWN: &str = n_!("Pieces Down:");
const TAB_SIZE: &str = n_!("Tab Size (%):");
const JITTER: &str = n_!("Jitter (%):");
const SEED: &str = n_!("Random Seed:");
const DESCRIPTION: &str = n_!(
    "Create custom jigsaw puzzle patterns from images or blank material. Features Draradech's \
     algorithm for unique pieces."
);

/// Problem messages.
const SIZE_TOO_SMALL: &str = n_!("Must be at least 50 mm");
const SIZE_TOO_LARGE: &str = n_!("Must be at most 2000 mm");
const PIECE_COUNT: &str = n_!("Must be a whole number from 2 to 20");
const PIECES_TOO_NARROW: &str = n_!("Pieces would be narrower than 15 mm");
const PIECES_TOO_SHORT: &str = n_!("Pieces would be shorter than 15 mm");
const TAB_RANGE: &str = n_!("Must be between 10 and 30");
const JITTER_RANGE: &str = n_!("Must be between 0 and 13");
const RADIUS_RANGE: &str = n_!("Must be between 0 and 10 mm");
const SEED_RANGE: &str = n_!("Must be a whole number from 0 to 4294967295");

/// The largest puzzle side (mm): the generator's most pieces (20) at 100 mm each. The
/// generator itself has no upper limit.
const MAX_SIZE_MM: f64 = 2000.0;

/// Rule for the puzzle's width and height: from [`MIN_SIZE_MM`] to [`MAX_SIZE_MM`].
fn size_rule(v: f64) -> Option<&'static str> {
    at_least(MIN_SIZE_MM, SIZE_TOO_SMALL)(v).or_else(|| at_most(MAX_SIZE_MM, v))
}

/// [`SIZE_TOO_LARGE`] above `max` (compared in `f32`, as the generator).
fn at_most(max: f64, v: f64) -> Option<&'static str> {
    (v as f32 > max as f32).then_some(SIZE_TOO_LARGE)
}

/// The msgids the page shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        WIDTH,
        HEIGHT,
        CORNER_RADIUS,
        PIECES_ACROSS,
        PIECES_DOWN,
        TAB_SIZE,
        JITTER,
        SEED,
        DESCRIPTION,
        SIZE_TOO_SMALL,
        SIZE_TOO_LARGE,
        PIECE_COUNT,
        PIECES_TOO_NARROW,
        PIECES_TOO_SHORT,
        TAB_RANGE,
        JITTER_RANGE,
        RADIUS_RANGE,
        SEED_RANGE,
        n_!("Puzzle Dimensions"),
        n_!("Grid Configuration"),
        n_!("Puzzle Parameters"),
        n_!("Randomize"),
    ]
}

/// The page's input: lengths as typed in [`JigsawParams::units`], the rest as typed.
#[derive(Clone, Debug, PartialEq)]
pub struct JigsawParams {
    /// The units the lengths are typed in.
    pub units: MeasurementSystem,
    /// Puzzle width.
    pub width: String,
    /// Puzzle height.
    pub height: String,
    /// Pieces across.
    pub pieces_across: String,
    /// Pieces down.
    pub pieces_down: String,
    /// The random seed.
    pub seed: String,
    /// Tab size (percent of the piece).
    pub tab_size: String,
    /// Jitter (percent).
    pub jitter: String,
    /// Corner radius.
    pub corner_radius: String,
}

impl JigsawParams {
    /// GTK's initial values in `units`.
    ///
    /// # Arguments
    /// * `units` - The display units.
    pub fn new(units: MeasurementSystem) -> Self {
        let len = |mm: f64| common::format_len(mm, units);
        Self {
            units,
            width: len(200.0),
            height: len(150.0),
            pieces_across: "4".into(),
            pieces_down: "3".into(),
            seed: "42".into(),
            tab_size: "20".into(),
            jitter: "4".into(),
            corner_radius: len(2.0),
        }
    }

    /// Shows the lengths in `units` instead, converting the typed values.
    ///
    /// # Arguments
    /// * `units` - The new display units.
    pub fn set_units(&mut self, units: MeasurementSystem) {
        let from = self.units;
        for text in [&mut self.width, &mut self.height, &mut self.corner_radius] {
            common::convert_len(text, from, units);
        }
        self.units = units;
    }

    /// Gives the seed GTK's shuffle button gave: the clock's nanoseconds below 100000.
    pub fn randomize_seed(&mut self) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        self.seed = (nanos % 100_000).to_string();
    }

    /// The generator's parameters, or every problem with the input.
    ///
    /// # Errors
    /// The fields that do not parse or that the generator would refuse.
    pub fn parameters(&self) -> Result<PuzzleParameters, Vec<FieldError>> {
        self.build(true)
    }

    /// The parameters; with `strict` false only the fields' syntax is checked (a preset).
    fn build(&self, strict: bool) -> Result<PuzzleParameters, Vec<FieldError>> {
        let mut errors = Vec::new();
        let u = self.units;
        let mut len = |label, text: &str, rule: &dyn Fn(f64) -> Option<&'static str>| {
            check(&mut errors, label, parse_len(text, u), gated(strict, rule))
        };
        let width = len(WIDTH, &self.width, &size_rule);
        let height = len(HEIGHT, &self.height, &size_rule);
        let corner_radius = len(
            CORNER_RADIUS,
            &self.corner_radius,
            &between(0.0, MAX_CORNER_RADIUS_MM, RADIUS_RANGE),
        );
        let count = between(MIN_PIECES, MAX_PIECES, PIECE_COUNT);
        let mut whole =
            |label, text: &str| check(&mut errors, label, parse_whole(text), gated(strict, &count));
        let pieces_across = whole(PIECES_ACROSS, &self.pieces_across) as i32;
        let pieces_down = whole(PIECES_DOWN, &self.pieces_down) as i32;
        let mut num = |label, text: &str, rule: &dyn Fn(f64) -> Option<&'static str>| {
            check(&mut errors, label, parse_number(text), gated(strict, rule))
        };
        let tab_size_percent = num(
            TAB_SIZE,
            &self.tab_size,
            &between(MIN_TAB_SIZE_PERCENT, MAX_TAB_SIZE_PERCENT, TAB_RANGE),
        );
        let jitter_percent = num(
            JITTER,
            &self.jitter,
            &between(0.0, MAX_JITTER_PERCENT, JITTER_RANGE),
        );
        let seed = match self.seed.trim().parse::<u32>() {
            Ok(seed) => seed,
            Err(_) => {
                errors.push(FieldError::field(SEED, SEED_RANGE));
                0
            }
        };
        if strict && errors.is_empty() {
            let (w, h) = (width as f32, height as f32);
            if w / (pieces_across as f32) < MIN_PIECE_MM as f32 {
                errors.push(FieldError::field(PIECES_ACROSS, PIECES_TOO_NARROW));
            }
            if h / (pieces_down as f32) < MIN_PIECE_MM as f32 {
                errors.push(FieldError::field(PIECES_DOWN, PIECES_TOO_SHORT));
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(PuzzleParameters {
            width: width as f32,
            height: height as f32,
            pieces_across,
            pieces_down,
            seed,
            tab_size_percent: tab_size_percent as f32,
            jitter_percent: jitter_percent as f32,
            corner_radius: corner_radius as f32,
        })
    }

    /// The page's input as GTK's preset file: the generator's parameters as pretty JSON.
    ///
    /// # Arguments
    /// * `tr` - Translator for the current UI language.
    ///
    /// # Errors
    /// The first field that does not parse (GTK refused to save then too).
    pub fn preset_json(&self, tr: &Translator) -> Result<String, String> {
        common::preset_text(self.build(false).map_err(common::first_error), tr)
    }

    /// Sets the page from a preset file in GTK's format ([`Self::preset_json`]).
    ///
    /// # Arguments
    /// * `json` - The file's text.
    ///
    /// # Errors
    /// The text is not a jigsaw preset; the page is left as it was.
    pub fn apply_preset(&mut self, json: &str) -> Result<(), String> {
        let p: PuzzleParameters = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let len = |mm: f32| common::format_len(f64::from(mm), self.units);
        self.width = len(p.width);
        self.height = len(p.height);
        self.pieces_across = p.pieces_across.to_string();
        self.pieces_down = p.pieces_down.to_string();
        self.seed = p.seed.to_string();
        self.tab_size = p.tab_size_percent.to_string();
        self.jitter = p.jitter_percent.to_string();
        self.corner_radius = len(p.corner_radius);
        Ok(())
    }

    /// Makes the puzzle's outlines: the pieces' edges, then the border, which is closed.
    ///
    /// # Errors
    /// [`CamError::Invalid`] while the input has problems, else the generator's refusal.
    pub fn generate(&self) -> Result<DesignerPaths, CamError> {
        let params = self.parameters().map_err(|_| CamError::Invalid)?;
        let mut maker = JigsawPuzzleMaker::new(params).map_err(CamError::Generator)?;
        maker.generate().map_err(CamError::Generator)?;
        let paths = maker.paths_as_f64();
        let total = paths.len();
        Ok(DesignerPaths {
            paths: paths
                .into_iter()
                .enumerate()
                .map(|(i, points)| DesignerPath {
                    points,
                    closed: i + 1 == total,
                })
                .collect(),
            group: true,
        })
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
/// The action button pressed, if any; never Generate while the input has problems.
pub fn show(ui: &mut egui::Ui, params: &mut JigsawParams, tr: &Translator) -> CamAction {
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
            heading(ui, tr, n_!("Puzzle Dimensions"));
            grid(ui, "cam-jigsaw-dimensions", |ui| {
                field_row(ui, tr, WIDTH, &mut params.width, unit, &errors);
                field_row(ui, tr, HEIGHT, &mut params.height, unit, &errors);
                field_row(
                    ui,
                    tr,
                    CORNER_RADIUS,
                    &mut params.corner_radius,
                    unit,
                    &errors,
                );
            });
            heading(ui, tr, n_!("Grid Configuration"));
            grid(ui, "cam-jigsaw-grid", |ui| {
                field_row(
                    ui,
                    tr,
                    PIECES_ACROSS,
                    &mut params.pieces_across,
                    "",
                    &errors,
                );
                field_row(ui, tr, PIECES_DOWN, &mut params.pieces_down, "", &errors);
            });
            heading(ui, tr, n_!("Puzzle Parameters"));
            grid(ui, "cam-jigsaw-parameters", |ui| {
                field_row(ui, tr, TAB_SIZE, &mut params.tab_size, "", &errors);
                field_row(ui, tr, JITTER, &mut params.jitter, "", &errors);
                field_row(ui, tr, SEED, &mut params.seed, "", &errors);
                ui.label("");
                if crate::icons::icon_button(
                    ui,
                    crate::icons::Icon::Shuffle,
                    &tr.tr(n_!("Randomize")),
                )
                .clicked()
                {
                    params.randomize_seed();
                }
                ui.end_row();
            });
            common::page_errors(ui, tr, &errors);
            CamAction::None
        },
        |ui, _| {
            common::action_row(
                ui,
                tr,
                errors.is_empty(),
                &[(n_!("Generate"), CamAction::SendToDesigner)],
            )
        },
    )
}
