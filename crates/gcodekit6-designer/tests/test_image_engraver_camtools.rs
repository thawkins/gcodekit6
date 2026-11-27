//! The designer's raster engraving is the camtools `BitmapImageEngraver` (audit T23, D-8 (a)):
//! the designer only maps its parameters, and keeps their user-visible meaning.

use gcodekit6_camtools::laser_engraver::{
    BitmapImageEngraver, EngravingParameters, ImageTransformations,
};
use gcodekit6_designer::engraving::{
    EngravingParams, HalftoneMethod, ImageEngraver, RotationAngle, ScanDirection,
};

/// The program without its `; Generated:` timestamp line.
fn untimed(gcode: &str) -> String {
    gcode
        .lines()
        .filter(|l| !l.starts_with("; Generated:"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A 32x8 grey ramp, black on the left.
fn ramp() -> image::DynamicImage {
    image::DynamicImage::ImageLuma8(image::GrayImage::from_fn(32, 8, |x, _| {
        image::Luma([(x * 8) as u8])
    }))
}

#[test]
fn test_designer_raster_is_the_camtools_program() {
    let params = EngravingParams {
        width_mm: 8.0,
        height_mm: Some(2.0),
        feed_rate: 1200.0,
        travel_rate: 4000.0,
        min_power: 10.0,
        max_power: 60.0,
        ppi: 101.6,
        scan_direction: ScanDirection::Vertical,
        bidirectional: false,
        invert: true,
        mirror_x: false,
        mirror_y: false,
        rotation: RotationAngle::Degrees0,
        halftone: HalftoneMethod::None,
        halftone_threshold: 127,
        offset_x: 1.0,
        offset_y: 2.0,
        power_scale: 1000.0,
        line_spacing: 1.0,
    };
    let designer = ImageEngraver::from_image(ramp(), params)
        .expect("designer engraver")
        .generate_gcode()
        .expect("designer gcode");

    let camtools = BitmapImageEngraver::from_image(
        ramp(),
        EngravingParameters {
            width_mm: 8.0,
            height_mm: Some(2.0),
            feed_rate: 1200.0,
            travel_rate: 4000.0,
            min_power: 10.0,
            max_power: 60.0,
            pixels_per_mm: 4.0,
            scan_direction: ScanDirection::Vertical,
            bidirectional: false,
            line_spacing: 1.0,
            power_scale: 1000.0,
            transformations: ImageTransformations {
                halftone_dot_size: 1,
                // The designer's `invert: true` (burn the dark areas) is camtools' default.
                invert: false,
                ..ImageTransformations::default()
            },
            // The designer keeps the origin 2.5 mm from 0 for the run-up.
            offset_x: 2.5,
            offset_y: 2.5,
            overscan_mm: 2.5,
            ..EngravingParameters::default()
        },
    )
    .expect("camtools engraver")
    .generate_gcode()
    .expect("camtools gcode");

    assert!(
        designer.starts_with("; Laser Image Engraving G-code"),
        "{designer}"
    );
    assert_eq!(untimed(&designer), untimed(&camtools));
}

#[test]
fn test_designer_raster_default_invert_burns_the_dark_side() {
    // A raster image's default `invert: true` burns black at Max Power; `false` burns white.
    let engrave = |invert| {
        let params = EngravingParams {
            width_mm: 8.0,
            ppi: 101.6,
            bidirectional: false,
            invert,
            ..EngravingParams::default()
        };
        ImageEngraver::from_image(ramp(), params)
            .expect("engraver")
            .generate_gcode()
            .expect("gcode")
    };
    // The first burn of a line starts at the image's left edge (X10, run-up from X7.5) only
    // when black burns.
    let first_segment = |gcode: &str| {
        gcode
            .lines()
            .find(|l| l.starts_with("G0 X") && l.ends_with("S0"))
            .map(str::to_owned)
            .expect("a segment start")
    };
    let dark = engrave(true);
    assert!(dark.contains("S1000"), "black burns at Max Power: {dark}");
    assert!(first_segment(&dark).starts_with("G0 X7.500 "), "{dark}");
    let light = engrave(false);
    assert!(!first_segment(&light).starts_with("G0 X7.500 "), "{light}");
}
