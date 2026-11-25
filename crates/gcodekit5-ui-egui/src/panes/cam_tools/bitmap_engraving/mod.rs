//! The Laser Image Engraver page: a raster image engraved line by line with the laser's
//! power following the image's darkness.
//!
//! GTK's defaults: 100 mm wide at 10 pixels per mm, 1000 / 3000 mm/min, power 0-100 % of
//! S1000, horizontal bidirectional scans, no transformation, 10 mm offsets. GTK silently
//! used a default for a field it could not read; here the field shows the problem and the
//! actions wait. The engraving runs on a worker thread ([`job`]), as GTK's did, so a large
//! image never stalls the UI. Beyond GTK, an output wider than [`MAX_LINE_PIXELS`] or with
//! more than [`MAX_PIXELS`] pixels in all is refused: the engraver resizes the whole image
//! to the output size in memory, and its height follows the image's aspect, so a tall
//! narrow image could ask for gigabytes. The total is checked on the page once the preview
//! knows the image's size, and again by the worker after decoding, before the resize.
//!
//! GTK's page has no laser-mode (`$32`) or connection check, so neither has this one: the
//! program goes to the editor, whose send path has its own guards.
//!
//! Dependencies: `egui`, `gcodekit5-camtools` (engraver, presets), `gcodekit5-app`
//! (translations), `gcodekit5-core` (units).

pub mod job;
mod page;

use std::path::{Path, PathBuf};

use gcodekit5_app::i18n::Translator;
use gcodekit5_camtools::cam_presets::BitmapEngravingPreset;
use gcodekit5_camtools::laser_engraver::{
    BitmapImageEngraver, EngravingParameters, HalftoneMethod, ImageTransformations, RotationAngle,
    ScanDirection,
};
use gcodekit5_core::units::MeasurementSystem;

use super::common::{
    self, FieldError, at_least, between, check, parse_len, parse_number, parse_whole, positive,
};
pub use page::show;

/// Field label msgids.
const IMAGE_PATH: &str = n_!("Image Path:");
const WIDTH: &str = n_!("Width:");
const FEED_RATE: &str = n_!("Feed Rate:");
const TRAVEL_RATE: &str = n_!("Travel Rate:");
const MIN_POWER: &str = n_!("Min Power (%):");
const MAX_POWER: &str = n_!("Max Power (%):");
const POWER_SCALE: &str = n_!("Power Scale (S):");
const SCAN_DIRECTION: &str = n_!("Scan Direction:");
const PIXELS_PER_MM: &str = n_!("Pixels per mm:");
const LINE_SPACING: &str = n_!("Line Spacing:");
const BIDIRECTIONAL: &str = n_!("Bidirectional:");
const INVERT: &str = n_!("Invert:");
const MIRROR_X: &str = n_!("Mirror X:");
const MIRROR_Y: &str = n_!("Mirror Y:");
const ROTATION: &str = n_!("Rotation:");
const METHOD: &str = n_!("Method:");
const DOT_SIZE: &str = n_!("Dot Size:");
const THRESHOLD: &str = n_!("Threshold:");
const OFFSET_X: &str = n_!("Offset X:");
const OFFSET_Y: &str = n_!("Offset Y:");

/// Problem messages.
const NO_IMAGE: &str = n_!("Please select an image file first.");
const PERCENT_RANGE: &str = n_!("Must be between 0 and 100");
const MIN_ABOVE_MAX: &str = n_!("Must not be more than the maximum power");
const AT_LEAST_ONE: &str = n_!("Must be at least 1");
const BYTE_RANGE: &str = n_!("Must be between 0 and 255");
const TOO_LARGE: &str = n_!("The engraving is too large: at most 10000 pixels per line");
const TOO_MANY_PIXELS: &str = n_!("The engraving is too large: at most 25 million pixels");
/// The engraving worker's refusals: the image could not be read (followed by why), or the
/// worker ended without a result.
const LOAD_FAILED: &str = n_!("Failed to load image file");
pub(super) const WORKER_STOPPED: &str = n_!("The engraving worker stopped");

/// The worker's own texts, which its refusals start with; see [`refusal_text`].
const WORKER_TEXTS: [&str; 3] = [TOO_MANY_PIXELS, LOAD_FAILED, WORKER_STOPPED];

/// The text shown for an engraving worker's refusal: its own texts translated, a detail
/// after them (and the engraver's own wording) kept as it is.
///
/// # Arguments
/// * `error` - The refusal as the worker reported it.
/// * `tr` - Translator for the current UI language.
pub fn refusal_text(error: &str, tr: &gcodekit5_app::i18n::Translator) -> String {
    crate::machine::translate_prefixed(error, &WORKER_TEXTS, tr)
}

/// The most pixels one engraved line may have (width times pixels per mm).
pub const MAX_LINE_PIXELS: f32 = 10_000.0;
/// The most pixels the engraving may have in all (width times height in pixels).
pub const MAX_PIXELS: f64 = 25_000_000.0;

/// The engraving's size in pixels for an image of `size` pixels, as the engraver computes
/// it: the width from the output width, the height from the image's aspect after the
/// rotation.
///
/// # Arguments
/// * `params` - The engraving parameters.
/// * `size` - The image's width and height in pixels.
pub fn output_pixels(params: &EngravingParameters, size: [u32; 2]) -> f64 {
    let [w, h] = match params.transformations.rotation {
        RotationAngle::Degrees90 | RotationAngle::Degrees270 => [size[1], size[0]],
        RotationAngle::Degrees0 | RotationAngle::Degrees180 => size,
    };
    let width = (params.width_mm * params.pixels_per_mm) as u32;
    let height = match params.height_mm {
        Some(mm) => (mm * params.pixels_per_mm) as u32,
        None => (width as f32 * (h as f32 / w.max(1) as f32)) as u32,
    };
    f64::from(width) * f64::from(height)
}

/// The msgids the page shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        IMAGE_PATH,
        WIDTH,
        FEED_RATE,
        TRAVEL_RATE,
        MIN_POWER,
        MAX_POWER,
        POWER_SCALE,
        SCAN_DIRECTION,
        PIXELS_PER_MM,
        LINE_SPACING,
        BIDIRECTIONAL,
        INVERT,
        MIRROR_X,
        MIRROR_Y,
        ROTATION,
        METHOD,
        DOT_SIZE,
        THRESHOLD,
        OFFSET_X,
        OFFSET_Y,
        NO_IMAGE,
        PERCENT_RANGE,
        MIN_ABOVE_MAX,
        AT_LEAST_ONE,
        BYTE_RANGE,
        TOO_LARGE,
        TOO_MANY_PIXELS,
        LOAD_FAILED,
        WORKER_STOPPED,
    ];
    ids.extend(page::msgids());
    ids.extend(job::msgids());
    ids
}

/// The page's input: lengths as typed in [`BitmapEngravingParams::units`], the rest as
/// typed.
#[derive(Clone, Debug, PartialEq)]
pub struct BitmapEngravingParams {
    /// The units the lengths are typed in.
    pub units: MeasurementSystem,
    /// The image to engrave.
    image: Option<PathBuf>,
    /// Output width.
    pub width: String,
    /// Engraving feed rate (mm/min).
    pub feed_rate: String,
    /// Travel feed rate (mm/min).
    pub travel_rate: String,
    /// Minimum power (%).
    pub min_power: String,
    /// Maximum power (%).
    pub max_power: String,
    /// Resolution (pixels per mm).
    pub pixels_per_mm: String,
    /// Line spacing (a multiplier of the pixel pitch).
    pub line_spacing: String,
    /// The S value of 100 % power.
    pub power_scale: String,
    /// X of the image's lower-left corner.
    pub offset_x: String,
    /// Y of the image's lower-left corner.
    pub offset_y: String,
    /// Scan direction.
    pub scan_direction: ScanDirection,
    /// Scan both ways.
    pub bidirectional: bool,
    /// Engrave the light areas instead of the dark ones.
    pub invert: bool,
    /// Mirror horizontally.
    pub mirror_x: bool,
    /// Mirror vertically.
    pub mirror_y: bool,
    /// Rotation.
    pub rotation: RotationAngle,
    /// Halftoning method.
    pub halftone: HalftoneMethod,
    /// Halftone dot size (pixels).
    pub halftone_dot_size: String,
    /// Halftone threshold (0-255).
    pub halftone_threshold: String,
    /// Home the machine before engraving.
    pub home_before: bool,
    /// The active device's axes (the pane keeps this up to date).
    pub num_axes: u8,
}

impl BitmapEngravingParams {
    /// GTK's initial values in `units`, with no image chosen.
    ///
    /// # Arguments
    /// * `units` - The display units.
    pub fn new(units: MeasurementSystem) -> Self {
        let len = |mm: f64| common::format_len(mm, units);
        Self {
            units,
            image: None,
            width: len(100.0),
            feed_rate: "1000".into(),
            travel_rate: "3000".into(),
            min_power: "0".into(),
            max_power: "100".into(),
            pixels_per_mm: "10".into(),
            line_spacing: "1.0".into(),
            power_scale: "1000".into(),
            offset_x: len(10.0),
            offset_y: len(10.0),
            scan_direction: ScanDirection::Horizontal,
            bidirectional: true,
            invert: false,
            mirror_x: false,
            mirror_y: false,
            rotation: RotationAngle::Degrees0,
            halftone: HalftoneMethod::None,
            halftone_dot_size: "4".into(),
            halftone_threshold: "127".into(),
            home_before: false,
            num_axes: 3,
        }
    }

    /// The image to engrave, if one is chosen.
    pub fn image(&self) -> Option<&Path> {
        self.image.as_deref()
    }

    /// Chooses the image to engrave (`None` for none).
    ///
    /// # Arguments
    /// * `image` - The image file.
    pub fn set_image(&mut self, image: Option<PathBuf>) {
        self.image = image;
    }

    /// Shows the lengths in `units` instead, converting the typed values.
    ///
    /// # Arguments
    /// * `units` - The new display units.
    pub fn set_units(&mut self, units: MeasurementSystem) {
        let from = self.units;
        for text in [&mut self.width, &mut self.offset_x, &mut self.offset_y] {
            common::convert_len(text, from, units);
        }
        self.units = units;
    }

    /// The engraver's parameters and the image, or every problem with the input.
    ///
    /// # Errors
    /// The fields that do not parse or that the engraver would refuse, a power range that
    /// is upside down, an output wider than [`MAX_LINE_PIXELS`], and no image.
    pub fn parameters(&self) -> Result<(PathBuf, EngravingParameters), Vec<FieldError>> {
        let mut errors = Vec::new();
        let u = self.units;
        let width = check(&mut errors, WIDTH, parse_len(&self.width, u), positive);
        let offset_x = check(&mut errors, OFFSET_X, parse_len(&self.offset_x, u), any);
        let offset_y = check(&mut errors, OFFSET_Y, parse_len(&self.offset_y, u), any);
        let mut num = |label, text: &str, rule: &dyn Fn(f64) -> Option<&'static str>| {
            check(&mut errors, label, parse_number(text), rule)
        };
        let percent = between(0.0, 100.0, PERCENT_RANGE);
        let feed_rate = num(FEED_RATE, &self.feed_rate, &positive);
        let travel_rate = num(TRAVEL_RATE, &self.travel_rate, &positive);
        let min_power = num(MIN_POWER, &self.min_power, &percent);
        let max_power = num(MAX_POWER, &self.max_power, &percent);
        let pixels_per_mm = num(PIXELS_PER_MM, &self.pixels_per_mm, &positive);
        let line_spacing = num(LINE_SPACING, &self.line_spacing, &positive);
        let power_scale = num(POWER_SCALE, &self.power_scale, &positive);
        let dot_rule = at_least(1.0, AT_LEAST_ONE);
        let dot_size = check(
            &mut errors,
            DOT_SIZE,
            parse_whole(&self.halftone_dot_size),
            |v| dot_rule(v).or_else(|| (v > f64::from(u32::MAX)).then_some(common::OUT_OF_RANGE)),
        );
        let threshold = check(
            &mut errors,
            THRESHOLD,
            parse_whole(&self.halftone_threshold),
            between(0.0, 255.0, BYTE_RANGE),
        );
        if errors.is_empty() {
            if min_power > max_power {
                errors.push(FieldError::field(MIN_POWER, MIN_ABOVE_MAX));
            }
            if (width as f32) * (pixels_per_mm as f32) > MAX_LINE_PIXELS {
                errors.push(FieldError::page(TOO_LARGE));
            }
        }
        let image = self.image.clone();
        if image.is_none() {
            errors.push(FieldError::page(NO_IMAGE));
        }
        let (Some(image), true) = (image, errors.is_empty()) else {
            return Err(errors);
        };
        let params = EngravingParameters {
            width_mm: width as f32,
            height_mm: None,
            feed_rate: feed_rate as f32,
            travel_rate: travel_rate as f32,
            min_power: min_power as f32,
            max_power: max_power as f32,
            pixels_per_mm: pixels_per_mm as f32,
            scan_direction: self.scan_direction,
            bidirectional: self.bidirectional,
            line_spacing: line_spacing as f32,
            power_scale: power_scale as f32,
            transformations: ImageTransformations {
                mirror_x: self.mirror_x,
                mirror_y: self.mirror_y,
                rotation: self.rotation,
                halftone: self.halftone,
                halftone_dot_size: dot_size as usize,
                halftone_threshold: threshold as u8,
                invert: self.invert,
            },
            offset_x: offset_x as f32,
            offset_y: offset_y as f32,
            num_axes: self.num_axes,
            home_before_job: self.home_before,
        };
        Ok((image, params))
    }

    /// The problem with engraving an image of `size` pixels, if its output has more than
    /// [`MAX_PIXELS`] pixels.
    ///
    /// # Arguments
    /// * `size` - The image's width and height in pixels.
    pub fn size_problem(&self, size: [usize; 2]) -> Option<FieldError> {
        let (_, params) = self.parameters().ok()?;
        let size = size.map(|v| u32::try_from(v).unwrap_or(u32::MAX));
        (output_pixels(&params, size) > MAX_PIXELS).then(|| FieldError::page(TOO_MANY_PIXELS))
    }

    /// Engraves the image here, on the calling thread (the pane runs this on a worker
    /// thread, see [`job`]).
    ///
    /// # Errors
    /// The input's first problem, or the engraver's refusal (an image it cannot read).
    pub fn engrave(&self) -> Result<String, String> {
        let (image, params) = self.parameters().map_err(|errors| {
            errors
                .first()
                .map_or_else(String::new, |e| e.message.to_string())
        })?;
        job::engrave(&image, params, |_| {})
    }

    /// The page's input as GTK's preset file (`BitmapEngravingPreset`, lengths in mm).
    ///
    /// # Arguments
    /// * `tr` - Translator for the current UI language.
    ///
    /// # Errors
    /// The first field that does not parse (GTK refused to save then too).
    pub fn preset_json(&self, tr: &Translator) -> Result<String, String> {
        let u = self.units;
        let preset = (|| {
            // GTK parsed these as `f32` directly.
            let number = |label: &'static str, text: &str| {
                text.trim()
                    .parse::<f32>()
                    .map_err(|_| FieldError::field(label, common::NOT_A_NUMBER))
            };
            let whole = |label: &'static str, text: &str| {
                text.trim()
                    .parse::<u32>()
                    .map_err(|_| FieldError::field(label, common::NOT_A_WHOLE_NUMBER))
            };
            Ok(BitmapEngravingPreset {
                image_path: self
                    .image
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                width_mm: common::preset_len(WIDTH, &self.width, u)?,
                feed_rate: number(FEED_RATE, &self.feed_rate)?,
                travel_rate: number(TRAVEL_RATE, &self.travel_rate)?,
                min_power: number(MIN_POWER, &self.min_power)?,
                max_power: number(MAX_POWER, &self.max_power)?,
                pixels_per_mm: number(PIXELS_PER_MM, &self.pixels_per_mm)?,
                line_spacing: number(LINE_SPACING, &self.line_spacing)?,
                power_scale: number(POWER_SCALE, &self.power_scale)?,
                offset_x: common::preset_len(OFFSET_X, &self.offset_x, u)?,
                offset_y: common::preset_len(OFFSET_Y, &self.offset_y, u)?,
                scan_direction: scan_id(self.scan_direction).to_string(),
                bidirectional: self.bidirectional,
                invert: self.invert,
                mirror_x: self.mirror_x,
                mirror_y: self.mirror_y,
                rotation: rotation_id(self.rotation).to_string(),
                halftone: halftone_id(self.halftone).to_string(),
                halftone_dot_size: whole(DOT_SIZE, &self.halftone_dot_size)?,
                halftone_threshold: whole(THRESHOLD, &self.halftone_threshold)?
                    .try_into()
                    .map_err(|_| FieldError::field(THRESHOLD, BYTE_RANGE))?,
                home_before_job: self.home_before,
                ..BitmapEngravingPreset::default()
            })
        })();
        let preset = preset.map_err(|e: FieldError| {
            let field = e.field.map_or_else(String::new, |f| tr.tr(f).into_owned());
            format!("{field} {}", tr.tr(e.message))
        })?;
        preset.to_json().map_err(|e| e.to_string())
    }

    /// Sets the page from a preset file in GTK's format (any version GTK reads; version 1
    /// lengths are read in the page's units, as GTK did). Choices GTK did not know leave
    /// their field as it is.
    ///
    /// # Arguments
    /// * `json` - The file's text.
    ///
    /// # Errors
    /// The text is not a bitmap engraving preset; the page is left as it was.
    pub fn apply_preset(&mut self, json: &str) -> Result<(), String> {
        // GTK read any JSON value as an old preset of defaults; only an object can be one.
        common::preset_object(json)?;
        let loaded =
            BitmapEngravingPreset::from_json(json, self.units).map_err(|e| e.to_string())?;
        for warning in &loaded.warnings {
            tracing::warn!(%warning, "bitmap preset loaded with a warning");
        }
        let p = loaded.preset;
        let u = self.units;
        if !p.image_path.is_empty() {
            self.image = Some(PathBuf::from(&p.image_path));
        }
        self.width = common::format_len(f64::from(p.width_mm), u);
        self.feed_rate = p.feed_rate.to_string();
        self.travel_rate = p.travel_rate.to_string();
        self.min_power = p.min_power.to_string();
        self.max_power = p.max_power.to_string();
        self.pixels_per_mm = p.pixels_per_mm.to_string();
        self.line_spacing = p.line_spacing.to_string();
        self.power_scale = p.power_scale.to_string();
        self.offset_x = common::format_len(f64::from(p.offset_x), u);
        self.offset_y = common::format_len(f64::from(p.offset_y), u);
        if let Some(v) = scan_from_id(&p.scan_direction) {
            self.scan_direction = v;
        }
        self.bidirectional = p.bidirectional;
        self.invert = p.invert;
        self.mirror_x = p.mirror_x;
        self.mirror_y = p.mirror_y;
        if let Some(v) = rotation_from_id(&p.rotation) {
            self.rotation = v;
        }
        if let Some(v) = halftone_from_id(&p.halftone) {
            self.halftone = v;
        }
        self.halftone_dot_size = p.halftone_dot_size.to_string();
        self.halftone_threshold = p.halftone_threshold.to_string();
        self.home_before = p.home_before_job;
        Ok(())
    }
}

/// Rule: any value.
fn any(_v: f64) -> Option<&'static str> {
    None
}

/// GTK's scan direction ids.
const SCANS: [(ScanDirection, &str); 2] = [
    (ScanDirection::Horizontal, "0"),
    (ScanDirection::Vertical, "1"),
];
/// GTK's rotation ids.
const ROTATIONS: [(RotationAngle, &str); 4] = [
    (RotationAngle::Degrees0, "0"),
    (RotationAngle::Degrees90, "90"),
    (RotationAngle::Degrees180, "180"),
    (RotationAngle::Degrees270, "270"),
];
/// GTK's halftone ids.
const HALFTONES: [(HalftoneMethod, &str); 5] = [
    (HalftoneMethod::None, "none"),
    (HalftoneMethod::Threshold, "threshold"),
    (HalftoneMethod::Bayer4x4, "bayer"),
    (HalftoneMethod::FloydSteinberg, "floyd"),
    (HalftoneMethod::Atkinson, "atkinson"),
];

fn scan_id(v: ScanDirection) -> &'static str {
    SCANS
        .iter()
        .find(|(d, _)| *d == v)
        .map_or("0", |(_, id)| id)
}

fn scan_from_id(id: &str) -> Option<ScanDirection> {
    SCANS.iter().find(|(_, i)| *i == id).map(|(d, _)| *d)
}

fn rotation_id(v: RotationAngle) -> &'static str {
    ROTATIONS
        .iter()
        .find(|(r, _)| *r == v)
        .map_or("0", |(_, id)| id)
}

fn rotation_from_id(id: &str) -> Option<RotationAngle> {
    ROTATIONS.iter().find(|(_, i)| *i == id).map(|(r, _)| *r)
}

fn halftone_id(v: HalftoneMethod) -> &'static str {
    HALFTONES
        .iter()
        .find(|(h, _)| *h == v)
        .map_or("none", |(_, id)| id)
}

fn halftone_from_id(id: &str) -> Option<HalftoneMethod> {
    HALFTONES.iter().find(|(_, i)| *i == id).map(|(h, _)| *h)
}

/// Engraves `image` with `params` on the calling thread.
///
/// # Errors
/// The engraver's refusal.
fn run_engraver(
    image: &Path,
    params: EngravingParameters,
    progress: impl FnMut(f32),
) -> Result<String, String> {
    let img = image::open(image).map_err(|e| format!("{LOAD_FAILED}: {e}"))?;
    // Before the engraver resizes the whole image to the output size.
    if output_pixels(&params, [img.width(), img.height()]) > MAX_PIXELS {
        return Err(TOO_MANY_PIXELS.to_string());
    }
    BitmapImageEngraver::from_image(img, params)
        .and_then(|engraver| engraver.generate_gcode_with_progress(progress))
        .map_err(|e| format!("{e:#}"))
}
