//! The Tabbed Box Maker page: a box of finger-jointed panels for a laser or a router.
//!
//! GTK's defaults: a 100 x 100 x 100 mm full box of 3 mm material, fingers and spaces of 2
//! thicknesses, 3 laser passes at S1000 and 500 mm/min, work origin offsets of 10 mm. The
//! program goes to the editor (Send to Editor, Preview) and the panels' outlines to the
//! designer (Generate Shapes). GTK silently used a default for a field it could not read;
//! here the field shows the problem and the actions wait. The generator's limits are checked
//! at the fields: a box of at least 20 mm each way in 1 to 20 mm material, at least one
//! pass, a finger and a space that add up to something, a Z step down that is not negative;
//! and, beyond the generator, a feed above 0, a power that is not negative and, for an
//! outside size, room for the two walls.
//!
//! Dependencies: `egui`, `gcodekit5-camtools` (generator), `gcodekit5-app` (translations),
//! `gcodekit5-core` (units), `serde_json` (presets).

mod preset;

use gcodekit5_app::i18n::Translator;
use gcodekit5_camtools::tabbed_box::{
    BoxParameters, BoxType, FingerJointSettings, KeyDividerType, TabbedBoxMaker,
};
use gcodekit5_core::units::{self, MeasurementSystem};

use super::common::{
    self, CamAction, CamError, FieldError, any, at_least, between, check, check_row, combo_row,
    field_row, gated, grid, heading, not_negative, parse_len, parse_number, parse_whole, positive,
};
use super::requests::{DesignerPath, DesignerPaths};

/// Field label msgids.
const WIDTH: &str = n_!("X (Width):");
const DEPTH: &str = n_!("Y (Depth):");
const HEIGHT: &str = n_!("H (Height):");
const OUTSIDE: &str = n_!("Outside Dims:");
const BOX_TYPE: &str = n_!("Box Type:");
const DIVIDERS_X: &str = n_!("Dividers X:");
const DIVIDERS_Y: &str = n_!("Dividers Y:");
const DIVIDER_KEYING: &str = n_!("Divider Keying:");
const OPTIMIZE: &str = n_!("Optimize Layout:");
const THICKNESS: &str = n_!("Thickness:");
const BURN: &str = n_!("Burn / Tool Dia:");
const FINGER_WIDTH: &str = n_!("Finger Width:");
const SPACE_WIDTH: &str = n_!("Space Width:");
const SURROUNDING_SPACES: &str = n_!("Surrounding Spaces:");
const PLAY: &str = n_!("Play (fit tolerance):");
const EXTRA_LENGTH: &str = n_!("Extra Length:");
const PASSES: &str = n_!("Passes:");
const POWER: &str = n_!("Power (S):");
const FEED_RATE: &str = n_!("Feed Rate:");
const Z_STEP_DOWN: &str = n_!("Z Step Down:");
const OFFSET_X: &str = n_!("Offset X:");
const OFFSET_Y: &str = n_!("Offset Y:");
const DESCRIPTION_START: &str = n_!(
    "Generate G-code for laser/CNC cut boxes with finger joints based on the boxes.py \
     algorithm. For more cases involving boxes and other objects, it is recommended to use \
     the "
);
const DESCRIPTION_END: &str = n_!(
    ". Install the Extension in Inkscape. There, generate the models and save them as SVG \
     files. Then import the SVG file into the Designer, rearrange the objects as needed, and \
     generate the G-code to send to the machine."
);
const HOME_HELP: &str = n_!(
    "Runs $H, then cuts from the G54 work zero you set. Travel/retract moves then use machine \
     Z-1 (G53), never above the homed position. Off: uses the work zero you set."
);

/// Problem messages.
const SIZE_TOO_SMALL: &str = n_!("Must be at least 20 mm");
const THICKNESS_RANGE: &str = n_!("Must be between 1 and 20 mm");
const NO_ROOM_FOR_WALLS: &str = n_!("Must be more than twice the thickness");
const JOINT_TOO_SMALL: &str = n_!("Finger plus space must be at least 0.1");
const AT_LEAST_ONE: &str = n_!("Must be at least 1");
const TOO_MANY_DIVIDERS: &str = n_!("Too many dividers: they would not fit the side");
const TOO_MANY_PASSES: &str = n_!("Must be at most 100");

/// The most laser passes the page accepts (each pass repeats every path).
const MAX_PASSES: f64 = 100.0;

/// The smallest box side the generator makes (mm).
const MIN_SIDE_MM: f64 = 20.0;

/// The box types and the msgids of their names, in GTK's menu order.
const BOX_TYPES: [(BoxType, &str); 6] = [
    (BoxType::FullBox, n_!("Full Box")),
    (BoxType::NoTop, n_!("No Top")),
    (BoxType::NoBottom, n_!("No Bottom")),
    (BoxType::NoSides, n_!("No Sides")),
    (BoxType::NoFrontBack, n_!("No Front/Back")),
    (BoxType::NoLeftRight, n_!("No Left/Right")),
];

/// The divider keying choices and the msgids of their names, in GTK's menu order.
const KEYINGS: [(KeyDividerType, &str); 4] = [
    (KeyDividerType::WallsAndFloor, n_!("Walls and Floor")),
    (KeyDividerType::WallsOnly, n_!("Walls Only")),
    (KeyDividerType::FloorOnly, n_!("Floor Only")),
    (KeyDividerType::None, n_!("None")),
];

/// The msgids the page shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        WIDTH,
        DEPTH,
        HEIGHT,
        OUTSIDE,
        BOX_TYPE,
        DIVIDERS_X,
        DIVIDERS_Y,
        DIVIDER_KEYING,
        OPTIMIZE,
        THICKNESS,
        BURN,
        FINGER_WIDTH,
        SPACE_WIDTH,
        SURROUNDING_SPACES,
        PLAY,
        EXTRA_LENGTH,
        PASSES,
        POWER,
        FEED_RATE,
        Z_STEP_DOWN,
        OFFSET_X,
        OFFSET_Y,
        HOME_HELP,
        SIZE_TOO_SMALL,
        THICKNESS_RANGE,
        NO_ROOM_FOR_WALLS,
        JOINT_TOO_SMALL,
        AT_LEAST_ONE,
        TOO_MANY_DIVIDERS,
        TOO_MANY_PASSES,
        n_!("Box Dimensions"),
        n_!("Box Configuration"),
        n_!("Material Settings"),
        n_!("Finger Joint Settings (multiples of thickness)"),
        n_!("Laser Settings"),
        n_!("Work Origin Offsets"),
        n_!("Generate Shapes"),
        DESCRIPTION_START,
        n_!("Boxes"),
        DESCRIPTION_END,
    ];
    ids.extend(BOX_TYPES.iter().map(|(_, msgid)| *msgid));
    ids.extend(KEYINGS.iter().map(|(_, msgid)| *msgid));
    ids
}

/// The page's input: lengths as typed in [`TabbedBoxParams::units`], the rest as typed.
#[derive(Clone, Debug, PartialEq)]
pub struct TabbedBoxParams {
    /// The units the lengths are typed in.
    pub units: MeasurementSystem,
    /// X (width).
    pub width: String,
    /// Y (depth).
    pub depth: String,
    /// H (height).
    pub height: String,
    /// The sizes are outside dimensions.
    pub outside: bool,
    /// Material thickness.
    pub thickness: String,
    /// Burn / tool diameter.
    pub burn: String,
    /// Finger width (multiples of the thickness).
    pub finger_width: String,
    /// Space width (multiples of the thickness).
    pub space_width: String,
    /// Surrounding spaces.
    pub surrounding_spaces: String,
    /// Play (fit tolerance).
    pub play: String,
    /// Extra length.
    pub extra_length: String,
    /// Which panels the box has.
    pub box_type: BoxType,
    /// Dividers along X.
    pub dividers_x: String,
    /// Dividers along Y.
    pub dividers_y: String,
    /// Where the dividers key into the box.
    pub divider_keying: KeyDividerType,
    /// Optimize the panel layout.
    pub optimize_layout: bool,
    /// Laser passes.
    pub passes: String,
    /// Laser power (S value).
    pub power: String,
    /// Feed rate (mm/min).
    pub feed_rate: String,
    /// Z step down.
    pub z_step_down: String,
    /// Work origin offset X.
    pub offset_x: String,
    /// Work origin offset Y.
    pub offset_y: String,
    /// Home the machine before cutting.
    pub home_before: bool,
    /// The active device's axes (the pane keeps this up to date).
    pub num_axes: u8,
}

impl TabbedBoxParams {
    /// GTK's initial values in `units`.
    ///
    /// # Arguments
    /// * `units` - The display units.
    pub fn new(units: MeasurementSystem) -> Self {
        let len = |mm: f64| common::format_len(mm, units);
        Self {
            units,
            width: len(100.0),
            depth: len(100.0),
            height: len(100.0),
            outside: false,
            thickness: len(3.0),
            burn: len(0.1),
            finger_width: "2".into(),
            space_width: "2".into(),
            surrounding_spaces: "2".into(),
            play: len(0.0),
            extra_length: len(0.0),
            box_type: BoxType::FullBox,
            dividers_x: "0".into(),
            dividers_y: "0".into(),
            divider_keying: KeyDividerType::WallsAndFloor,
            optimize_layout: false,
            passes: "3".into(),
            power: "1000".into(),
            feed_rate: "500".into(),
            z_step_down: len(0.1),
            offset_x: len(10.0),
            offset_y: len(10.0),
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
            &mut self.depth,
            &mut self.height,
            &mut self.thickness,
            &mut self.burn,
            &mut self.play,
            &mut self.extra_length,
            &mut self.z_step_down,
            &mut self.offset_x,
            &mut self.offset_y,
        ] {
            common::convert_len(text, from, units);
        }
        self.units = units;
    }

    /// The generator's parameters, or every problem with the input.
    ///
    /// # Errors
    /// The fields that do not parse or that the generator would refuse.
    pub fn parameters(&self) -> Result<BoxParameters, Vec<FieldError>> {
        self.build(true)
    }

    /// The parameters; with `strict` false only the fields' syntax is checked (a preset).
    fn build(&self, strict: bool) -> Result<BoxParameters, Vec<FieldError>> {
        let mut errors = Vec::new();
        let u = self.units;
        let mut len = |label, text: &str, rule: &dyn Fn(f64) -> Option<&'static str>| {
            check(&mut errors, label, parse_len(text, u), gated(strict, rule))
        };
        let side = at_least(MIN_SIDE_MM, SIZE_TOO_SMALL);
        let x = len(WIDTH, &self.width, &side);
        let y = len(DEPTH, &self.depth, &side);
        let h = len(HEIGHT, &self.height, &side);
        let thickness = len(
            THICKNESS,
            &self.thickness,
            &between(1.0, 20.0, THICKNESS_RANGE),
        );
        let burn = len(BURN, &self.burn, &not_negative);
        let play = len(PLAY, &self.play, &any);
        let extra_length = len(EXTRA_LENGTH, &self.extra_length, &any);
        let z_step_down = len(Z_STEP_DOWN, &self.z_step_down, &not_negative);
        let offset_x = len(OFFSET_X, &self.offset_x, &any);
        let offset_y = len(OFFSET_Y, &self.offset_y, &any);
        let mut num = |label, text: &str, rule: &dyn Fn(f64) -> Option<&'static str>| {
            check(&mut errors, label, parse_number(text), gated(strict, rule))
        };
        let finger = num(FINGER_WIDTH, &self.finger_width, &positive);
        let space = num(SPACE_WIDTH, &self.space_width, &positive);
        let surrounding_spaces = num(SURROUNDING_SPACES, &self.surrounding_spaces, &not_negative);
        let feed_rate = num(FEED_RATE, &self.feed_rate, &positive);
        let mut whole = |label, text: &str, rule: &dyn Fn(f64) -> Option<&'static str>| {
            check(&mut errors, label, parse_whole(text), gated(strict, rule))
        };
        let dividers_x = whole(DIVIDERS_X, &self.dividers_x, &not_negative);
        let dividers_y = whole(DIVIDERS_Y, &self.dividers_y, &not_negative);
        let at_least_one = at_least(1.0, AT_LEAST_ONE);
        let passes_rule =
            |v: f64| at_least_one(v).or_else(|| (v > MAX_PASSES).then_some(TOO_MANY_PASSES));
        let passes = whole(PASSES, &self.passes, &passes_rule);
        let power = whole(POWER, &self.power, &not_negative);
        // The dividers need their sides and the thickness, whatever else is wrong.
        let sizes_read = !errors
            .iter()
            .any(|e| matches!(e.field, Some(f) if [WIDTH, DEPTH, THICKNESS].contains(&f)));
        if strict && sizes_read {
            self.check_dividers(&mut errors, [x, y], thickness, [dividers_x, dividers_y]);
        }
        if strict && errors.is_empty() {
            self.check_joint_and_walls(&mut errors, [x, y, h], thickness, finger, space);
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(BoxParameters {
            x: x as f32,
            y: y as f32,
            h: h as f32,
            outside: self.outside,
            thickness: thickness as f32,
            burn: burn as f32,
            finger_joint: FingerJointSettings {
                finger: finger as f32,
                space: space as f32,
                surrounding_spaces: surrounding_spaces as f32,
                play: play as f32,
                extra_length: extra_length as f32,
                ..FingerJointSettings::default()
            },
            box_type: self.box_type,
            dividers_x: dividers_x as u32,
            dividers_y: dividers_y as u32,
            key_divider_type: self.divider_keying,
            optimize_layout: self.optimize_layout,
            laser_passes: passes as i32,
            laser_power: power as i32,
            feed_rate: feed_rate as f32,
            z_step_down: z_step_down as f32,
            offset_x: offset_x as f32,
            offset_y: offset_y as f32,
            num_axes: self.num_axes,
            home_before_job: self.home_before,
        })
    }

    /// The problems that need two fields: a finger and a space that add up to something, and
    /// an outside size with room for the two walls.
    fn check_joint_and_walls(
        &self,
        errors: &mut Vec<FieldError>,
        sides: [f64; 3],
        thickness: f64,
        finger: f64,
        space: f64,
    ) {
        if ((space + finger) as f32).abs() < 0.1 {
            errors.push(FieldError::field(SPACE_WIDTH, JOINT_TOO_SMALL));
        }
        if self.outside {
            for (label, side) in [WIDTH, DEPTH, HEIGHT].into_iter().zip(sides) {
                if (side as f32) - 2.0 * (thickness as f32) <= 0.0 {
                    errors.push(FieldError::field(label, NO_ROOM_FOR_WALLS));
                }
            }
        }
    }

    /// The dividers that do not fit their side: the generator spaces `n` dividers evenly
    /// over the inside length, so each gap must be wider than one thickness, or the slots
    /// (one thickness wide, less the burn) and the divider panels overlap. Inside lengths
    /// are the sizes less the two walls when sizes are outside ones, as the generator takes
    /// them.
    fn check_dividers(
        &self,
        errors: &mut Vec<FieldError>,
        sides: [f64; 2],
        thickness: f64,
        dividers: [f64; 2],
    ) {
        let t = thickness as f32;
        for ((label, side), count) in [DIVIDERS_X, DIVIDERS_Y]
            .into_iter()
            .zip(sides)
            .zip(dividers)
        {
            let inside = if self.outside {
                side as f32 - 2.0 * t
            } else {
                side as f32
            };
            if count > 0.0 && inside / (count as f32 + 1.0) <= t {
                errors.push(FieldError::field(label, TOO_MANY_DIVIDERS));
            }
        }
    }

    /// Makes the box's program.
    ///
    /// # Errors
    /// [`CamError::Invalid`] while the input has problems, else the generator's refusal.
    pub fn generate(&self) -> Result<String, CamError> {
        let params = self.parameters().map_err(|_| CamError::Invalid)?;
        let mut maker = TabbedBoxMaker::new(params).map_err(CamError::Generator)?;
        maker.generate().map_err(CamError::Generator)?;
        Ok(maker.to_gcode())
    }

    /// Makes the panels' outlines for the designer, each closed (GTK's Generate Shapes).
    ///
    /// # Errors
    /// [`CamError::Invalid`] while the input has problems, else the generator's refusal.
    pub fn generate_shapes(&self) -> Result<DesignerPaths, CamError> {
        let params = self.parameters().map_err(|_| CamError::Invalid)?;
        let mut maker = TabbedBoxMaker::new(params).map_err(CamError::Generator)?;
        maker.generate().map_err(CamError::Generator)?;
        Ok(DesignerPaths {
            paths: maker
                .paths()
                .iter()
                .map(|path| DesignerPath {
                    points: path
                        .iter()
                        .map(|p| (f64::from(p.x), f64::from(p.y)))
                        .collect(),
                    closed: true,
                })
                .collect(),
            group: false,
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
/// The action button pressed, if any; never a generating one while the input has problems.
pub fn show(ui: &mut egui::Ui, params: &mut TabbedBoxParams, tr: &Translator) -> CamAction {
    let errors = params.parameters().err().unwrap_or_default();
    common::cam_page(
        ui,
        common::SIDEBAR_SHARE,
        params,
        |ui, _| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label(tr.tr(DESCRIPTION_START));
                ui.hyperlink_to(tr.tr("Boxes"), "https://github.com/florianfesti/boxes");
                ui.label(tr.tr(DESCRIPTION_END));
            });
        },
        |ui, params| {
            show_fields(ui, params, tr, &errors);
            common::page_errors(ui, tr, &errors);
            CamAction::None
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
                    (n_!("Generate Shapes"), CamAction::SendToDesigner),
                ],
            )
        },
    )
}

/// The page's groups of fields.
fn show_fields(ui: &mut egui::Ui, p: &mut TabbedBoxParams, tr: &Translator, errors: &[FieldError]) {
    let unit = units::get_unit_label(p.units);
    heading(ui, tr, n_!("Box Dimensions"));
    grid(ui, "cam-box-dimensions", |ui| {
        field_row(ui, tr, WIDTH, &mut p.width, unit, errors);
        field_row(ui, tr, DEPTH, &mut p.depth, unit, errors);
        field_row(ui, tr, HEIGHT, &mut p.height, unit, errors);
        check_row(ui, tr, OUTSIDE, &mut p.outside);
    });
    heading(ui, tr, n_!("Box Configuration"));
    grid(ui, "cam-box-configuration", |ui| {
        combo_row(
            ui,
            tr,
            "cam-box-type",
            BOX_TYPE,
            &mut p.box_type,
            &BOX_TYPES,
        );
        field_row(ui, tr, DIVIDERS_X, &mut p.dividers_x, "", errors);
        field_row(ui, tr, DIVIDERS_Y, &mut p.dividers_y, "", errors);
        combo_row(
            ui,
            tr,
            "cam-box-keying",
            DIVIDER_KEYING,
            &mut p.divider_keying,
            &KEYINGS,
        );
        check_row(ui, tr, OPTIMIZE, &mut p.optimize_layout);
    });
    heading(ui, tr, n_!("Material Settings"));
    grid(ui, "cam-box-material", |ui| {
        field_row(ui, tr, THICKNESS, &mut p.thickness, unit, errors);
        field_row(ui, tr, BURN, &mut p.burn, unit, errors);
    });
    heading(
        ui,
        tr,
        n_!("Finger Joint Settings (multiples of thickness)"),
    );
    grid(ui, "cam-box-fingers", |ui| {
        field_row(ui, tr, FINGER_WIDTH, &mut p.finger_width, "", errors);
        field_row(ui, tr, SPACE_WIDTH, &mut p.space_width, "", errors);
        field_row(
            ui,
            tr,
            SURROUNDING_SPACES,
            &mut p.surrounding_spaces,
            "",
            errors,
        );
        field_row(ui, tr, PLAY, &mut p.play, unit, errors);
        field_row(ui, tr, EXTRA_LENGTH, &mut p.extra_length, unit, errors);
    });
    heading(ui, tr, n_!("Laser Settings"));
    grid(ui, "cam-box-laser", |ui| {
        field_row(ui, tr, PASSES, &mut p.passes, "", errors);
        field_row(ui, tr, POWER, &mut p.power, "", errors);
        field_row(ui, tr, FEED_RATE, &mut p.feed_rate, "", errors);
        field_row(ui, tr, Z_STEP_DOWN, &mut p.z_step_down, unit, errors);
    });
    heading(ui, tr, n_!("Work Origin Offsets"));
    grid(ui, "cam-box-offsets", |ui| {
        field_row(ui, tr, OFFSET_X, &mut p.offset_x, unit, errors);
        field_row(ui, tr, OFFSET_Y, &mut p.offset_y, unit, errors);
        check_row(ui, tr, n_!("Home Device Before Start"), &mut p.home_before);
    });
    ui.weak(tr.tr(HOME_HELP));
}
