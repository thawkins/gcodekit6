//! Designer raster engraving: maps a raster image's laser settings onto the camtools engraver.
//!
//! The engraving itself (scaling, halftoning, scan lines, G-code) is done by
//! [`gcodekit6_camtools::laser_engraver::BitmapImageEngraver`], the one image engraver in the
//! workspace. This module only translates the designer's user-visible parameters:
//!
//! - `ppi` becomes pixels per millimetre;
//! - `invert` keeps the designer meaning (`true`, the raster default, burns the dark areas),
//!   which is the opposite of the camtools flag;
//! - a halftoned off dot is laser off, so `min_power` is not applied to halftoned images;
//! - halftoning works per pixel (cell size 1);
//! - every burn gets an [`OVERSCAN_MM`] laser-off run-up and run-out, and the image origin is
//!   kept at least [`OVERSCAN_MM`] from 0 so the run-up stays at or above X/Y 0;
//! - transparent pixels are composited onto white before engraving.

use anyhow::{Context, Result};
use gcodekit6_camtools::laser_engraver::{
    BitmapImageEngraver, EngravingParameters, ImageTransformations,
};
use image::DynamicImage;
use std::path::Path;

pub use gcodekit6_camtools::laser_engraver::{HalftoneMethod, RotationAngle, ScanDirection};

/// A raster image's engraving settings, as the designer presents them.
#[derive(Debug, Clone)]
pub struct EngravingParams {
    /// Output width (mm).
    pub width_mm: f64,
    /// Output height (mm); from the image's aspect ratio when `None`.
    pub height_mm: Option<f64>,
    /// Engraving feed rate (mm/min).
    pub feed_rate: f64,
    /// Travel feed rate (mm/min).
    pub travel_rate: f64,
    /// Minimum laser power (%), used for grey levels only.
    pub min_power: f32,
    /// Maximum laser power (%).
    pub max_power: f32,
    /// Resolution (pixels per inch).
    pub ppi: f64,
    /// Scan direction.
    pub scan_direction: ScanDirection,
    /// Alternate the scan direction line by line.
    pub bidirectional: bool,
    /// `true` burns the dark areas (the raster default); `false` burns the light ones.
    pub invert: bool,
    /// Mirror horizontally.
    pub mirror_x: bool,
    /// Mirror vertically.
    pub mirror_y: bool,
    /// Rotation.
    pub rotation: RotationAngle,
    /// Halftoning applied after scaling (and inversion).
    pub halftone: HalftoneMethod,
    /// Cut-off for [`HalftoneMethod::Threshold`] (0-255).
    pub halftone_threshold: u8,
    /// X of the image's lower-left corner (mm).
    pub offset_x: f64,
    /// Y of the image's lower-left corner (mm).
    pub offset_y: f64,
    /// S value of 100 % power.
    pub power_scale: f32,
    /// Line spacing as a multiple of the pixel pitch.
    pub line_spacing: f64,
}

impl Default for EngravingParams {
    fn default() -> Self {
        Self {
            width_mm: 100.0,
            height_mm: None,
            feed_rate: 1000.0,
            travel_rate: 3000.0,
            min_power: 0.0,
            max_power: 100.0,
            ppi: 254.0,
            scan_direction: ScanDirection::Horizontal,
            bidirectional: true,
            invert: false,
            mirror_x: false,
            mirror_y: false,
            rotation: RotationAngle::Degrees0,
            halftone: HalftoneMethod::None,
            halftone_threshold: 127,
            offset_x: 10.0,
            offset_y: 10.0,
            power_scale: 1000.0,
            line_spacing: 1.0,
        }
    }
}

/// Laser-off run-up and run-out (mm) around every burn of a designer raster image.
///
/// The image origin is kept at least this far from 0 on both axes, so a run-up never goes
/// below X/Y 0.
pub const OVERSCAN_MM: f64 = 2.5;

impl EngravingParams {
    /// Convert PPI to pixels per mm
    pub fn pixels_per_mm(&self) -> f64 {
        self.ppi / 25.4
    }

    /// The camtools parameters for these designer settings (see the module docs).
    fn to_camtools(&self) -> EngravingParameters {
        let halftoned = self.halftone != HalftoneMethod::None;
        EngravingParameters {
            width_mm: self.width_mm,
            height_mm: self.height_mm,
            feed_rate: self.feed_rate,
            travel_rate: self.travel_rate,
            min_power: if halftoned { 0.0 } else { self.min_power },
            max_power: self.max_power,
            pixels_per_mm: self.pixels_per_mm(),
            scan_direction: self.scan_direction,
            bidirectional: self.bidirectional,
            line_spacing: self.line_spacing,
            power_scale: self.power_scale,
            transformations: ImageTransformations {
                mirror_x: self.mirror_x,
                mirror_y: self.mirror_y,
                rotation: self.rotation,
                halftone: self.halftone,
                halftone_dot_size: 1,
                halftone_threshold: self.halftone_threshold,
                invert: !self.invert,
            },
            offset_x: self.offset_x.max(OVERSCAN_MM),
            offset_y: self.offset_y.max(OVERSCAN_MM),
            overscan_mm: OVERSCAN_MM,
            ..EngravingParameters::default()
        }
    }
}

/// Engraves one designer raster image through [`BitmapImageEngraver`].
pub struct ImageEngraver {
    engraver: BitmapImageEngraver,
}

impl ImageEngraver {
    /// Create a new engraver from a file
    ///
    /// # Errors
    /// The file cannot be read or decoded, or the parameters are invalid.
    pub fn from_file<P: AsRef<Path>>(path: P, params: EngravingParams) -> Result<Self> {
        let img = image::open(path.as_ref()).context("Failed to load image file")?;
        Self::from_image(img, params)
    }

    /// Create a new engraver from a designer RasterImage
    ///
    /// # Errors
    /// The image has no original path, the file cannot be loaded, or the parameters are
    /// invalid.
    pub fn from_raster_image(
        image: &crate::model::RasterImage,
        params: EngravingParams,
    ) -> Result<Self> {
        let path = image
            .original_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Image has no original path"))?;
        Self::from_file(path, params)
    }

    /// Create a new engraver from a decoded image.
    ///
    /// # Errors
    /// The parameters are invalid (see [`BitmapImageEngraver::from_image`]).
    pub fn from_image(img: DynamicImage, params: EngravingParams) -> Result<Self> {
        let engraver = BitmapImageEngraver::from_image(on_white(img), params.to_camtools())?;
        Ok(Self { engraver })
    }

    /// Get the output dimensions in millimeters
    pub fn output_size_mm(&self) -> (f64, f64) {
        self.engraver.output_size_mm()
    }

    /// Generate G-code
    ///
    /// # Errors
    /// The camtools engraver failed to generate the program.
    pub fn generate_gcode(&self) -> Result<String> {
        self.engraver.generate_gcode()
    }
}

/// `img` with any transparency composited onto a white background.
fn on_white(img: DynamicImage) -> DynamicImage {
    if !img.color().has_alpha() {
        return img;
    }
    let mut composed = img.to_rgba8();
    for pixel in composed.pixels_mut() {
        let alpha = f32::from(pixel[3]) / 255.0;
        for channel in 0..3 {
            pixel[channel] = (f32::from(pixel[channel]) * alpha + 255.0 * (1.0 - alpha)) as u8;
        }
        pixel[3] = 255;
    }
    DynamicImage::ImageRgba8(composed)
}
