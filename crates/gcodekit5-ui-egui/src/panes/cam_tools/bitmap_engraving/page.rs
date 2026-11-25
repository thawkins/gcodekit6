//! Drawing of the Laser Image Engraver page: GTK's groups (image, output, power, scanning,
//! transformations, halftoning, offsets), the image preview and the actions.
//!
//! Dependencies: `egui`, `gcodekit5-camtools` (option types), `gcodekit5-app`
//! (translations), `gcodekit5-core` (units).

use gcodekit5_app::i18n::Translator;
use gcodekit5_camtools::laser_engraver::{HalftoneMethod, RotationAngle, ScanDirection};
use gcodekit5_core::units;

use super::super::common::{self, CamAction, check_row, combo_row, field_row, grid, heading};
use super::{
    BIDIRECTIONAL, BitmapEngravingParams, DOT_SIZE, FEED_RATE, IMAGE_PATH, INVERT, LINE_SPACING,
    MAX_POWER, METHOD, MIN_POWER, MIRROR_X, MIRROR_Y, OFFSET_X, OFFSET_Y, PIXELS_PER_MM,
    POWER_SCALE, ROTATION, SCAN_DIRECTION, THRESHOLD, TRAVEL_RATE, WIDTH,
};

const DESCRIPTION: &str = n_!(
    "Convert bitmap images to G-code for laser engraving. Supports various halftoning methods \
     and image transformations."
);
const INVERT_HELP: &str = n_!("Engrave light areas instead of dark ones");
const HOME_HELP: &str =
    n_!("Runs $H, then engraves from the G54 work zero you set. The engraver never moves Z.");
const NO_IMAGE_SELECTED: &str = n_!("No image selected");

/// The msgids the page shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        DESCRIPTION,
        INVERT_HELP,
        HOME_HELP,
        NO_IMAGE_SELECTED,
        n_!("Browse..."),
        n_!("Image File"),
        n_!("Output Settings"),
        n_!("Laser Power"),
        n_!("Scanning"),
        n_!("Image Transformations"),
        n_!("Halftoning"),
        n_!("Work Offsets"),
        n_!("Horizontal"),
        n_!("Vertical"),
        n_!("None"),
        n_!("Threshold"),
        n_!("Atkinson"),
    ]
}

/// The options of the drop-downs (names GTK did not translate are shown as they are).
const SCANS: [(ScanDirection, &str); 2] = [
    (ScanDirection::Horizontal, n_!("Horizontal")),
    (ScanDirection::Vertical, n_!("Vertical")),
];
const ROTATIONS: [(RotationAngle, &str); 4] = [
    (RotationAngle::Degrees0, "0\u{b0}"),
    (RotationAngle::Degrees90, "90\u{b0}"),
    (RotationAngle::Degrees180, "180\u{b0}"),
    (RotationAngle::Degrees270, "270\u{b0}"),
];
const HALFTONES: [(HalftoneMethod, &str); 5] = [
    (HalftoneMethod::None, n_!("None")),
    (HalftoneMethod::Threshold, n_!("Threshold")),
    (HalftoneMethod::Bayer4x4, "Bayer 4x4"),
    (HalftoneMethod::FloydSteinberg, "Floyd-Steinberg"),
    (HalftoneMethod::Atkinson, n_!("Atkinson")),
];

/// Draws the page.
///
/// # Arguments
/// * `ui` - The page's `Ui`.
/// * `params` - The page's input.
/// * `preview` - The chosen image, once decoded.
/// * `busy` - Whether an engraving is running (the actions wait for it).
/// * `tr` - Translator for the current UI language.
///
/// # Returns
/// The action button pressed, if any; never a generating one while the input has problems
/// or an engraving runs.
pub fn show(
    ui: &mut egui::Ui,
    params: &mut BitmapEngravingParams,
    preview: Option<&egui::TextureHandle>,
    busy: bool,
    tr: &Translator,
) -> CamAction {
    let mut action = CamAction::None;
    ui.label(tr.tr(DESCRIPTION));
    let errors = params.parameters().err().unwrap_or_default();
    let unit = units::get_unit_label(params.units);
    heading(ui, tr, n_!("Image File"));
    ui.horizontal(|ui| {
        let name = ui.label(tr.tr(IMAGE_PATH));
        let shown = params.image().map_or_else(
            || tr.tr(NO_IMAGE_SELECTED).into_owned(),
            |p| p.display().to_string(),
        );
        ui.add(egui::Label::new(shown).truncate())
            .labelled_by(name.id);
        if ui.button(tr.tr("Browse...")).clicked() {
            action = CamAction::ChooseImage;
        }
    });
    if let Some(texture) = preview {
        ui.add(
            egui::Image::new(texture)
                .max_height(240.0)
                .max_width(ui.available_width()),
        );
    }
    heading(ui, tr, n_!("Output Settings"));
    grid(ui, "cam-bitmap-output", |ui| {
        field_row(ui, tr, WIDTH, &mut params.width, unit, &errors);
        field_row(ui, tr, FEED_RATE, &mut params.feed_rate, "", &errors);
        field_row(ui, tr, TRAVEL_RATE, &mut params.travel_rate, "", &errors);
    });
    heading(ui, tr, n_!("Laser Power"));
    grid(ui, "cam-bitmap-power", |ui| {
        field_row(ui, tr, MIN_POWER, &mut params.min_power, "", &errors);
        field_row(ui, tr, MAX_POWER, &mut params.max_power, "", &errors);
        field_row(ui, tr, POWER_SCALE, &mut params.power_scale, "", &errors);
    });
    heading(ui, tr, n_!("Scanning"));
    grid(ui, "cam-bitmap-scan", |ui| {
        combo_row(
            ui,
            tr,
            "cam-bitmap-scan-direction",
            SCAN_DIRECTION,
            &mut params.scan_direction,
            &SCANS,
        );
        field_row(
            ui,
            tr,
            PIXELS_PER_MM,
            &mut params.pixels_per_mm,
            "",
            &errors,
        );
        field_row(ui, tr, LINE_SPACING, &mut params.line_spacing, "", &errors);
        check_row(ui, tr, BIDIRECTIONAL, &mut params.bidirectional);
    });
    heading(ui, tr, n_!("Image Transformations"));
    grid(ui, "cam-bitmap-transform", |ui| {
        let name = ui.label(tr.tr(INVERT));
        ui.checkbox(&mut params.invert, tr.tr(INVERT_HELP))
            .labelled_by(name.id);
        ui.end_row();
        check_row(ui, tr, MIRROR_X, &mut params.mirror_x);
        check_row(ui, tr, MIRROR_Y, &mut params.mirror_y);
        combo_row(
            ui,
            tr,
            "cam-bitmap-rotation",
            ROTATION,
            &mut params.rotation,
            &ROTATIONS,
        );
    });
    heading(ui, tr, n_!("Halftoning"));
    grid(ui, "cam-bitmap-halftone", |ui| {
        combo_row(
            ui,
            tr,
            "cam-bitmap-halftone",
            METHOD,
            &mut params.halftone,
            &HALFTONES,
        );
        field_row(ui, tr, DOT_SIZE, &mut params.halftone_dot_size, "", &errors);
        field_row(
            ui,
            tr,
            THRESHOLD,
            &mut params.halftone_threshold,
            "",
            &errors,
        );
    });
    heading(ui, tr, n_!("Work Offsets"));
    grid(ui, "cam-bitmap-offsets", |ui| {
        field_row(ui, tr, OFFSET_X, &mut params.offset_x, unit, &errors);
        field_row(ui, tr, OFFSET_Y, &mut params.offset_y, unit, &errors);
        check_row(
            ui,
            tr,
            n_!("Home Device Before Start"),
            &mut params.home_before,
        );
    });
    ui.weak(tr.tr(HOME_HELP));
    common::page_errors(ui, tr, &errors);
    let pressed = common::gcode_actions(ui, tr, errors.is_empty() && !busy);
    if pressed != CamAction::None {
        action = pressed;
    }
    action
}
