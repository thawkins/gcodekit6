//! Gerber G36/G37 regions, aperture macros (%AM) and clear polarity (%LPC) are drawn
//! (feature P2CAM-1b): each case checks the layer's area and bounds against the exact
//! shape. Arcs and circles are chords at most 0.005 mm from the true curve, so areas are
//! compared within 0.01 mm times the shape's perimeter (the boundary within 0.01 mm), and
//! bounds within 0.01 mm.

use csgrs::sketch::Sketch;
use csgrs::traits::CSG;
use gcodekit5_camtools::gerber::{UnsupportedConstructs, layer_geometry, unsupported_constructs};
use std::f64::consts::PI;

const HEADER: &str = "%FSLAX46Y46*%\n%MOMM*%\n";

/// The full file for `body` (between the format header and `M02`).
fn file(body: &str) -> String {
    format!("{HEADER}{body}M02*\n")
}

/// The layer geometry of `body`.
fn geometry(body: &str) -> Sketch<()> {
    layer_geometry(&file(body))
        .expect("parse")
        .expect("the layer draws something")
}

/// Shoelace area of a closed ring.
fn ring_area(pts: impl Iterator<Item = (f64, f64)>) -> f64 {
    let pts: Vec<(f64, f64)> = pts.collect();
    pts.windows(2)
        .map(|w| w[0].0 * w[1].1 - w[1].0 * w[0].1)
        .sum::<f64>()
        .abs()
        / 2.0
}

/// Area of the sketch: exteriors minus holes.
fn area(sketch: &Sketch<()>) -> f64 {
    sketch
        .to_multipolygon()
        .0
        .iter()
        .map(|p| {
            let holes: f64 = p
                .interiors()
                .iter()
                .map(|r| ring_area(r.0.iter().map(|c| (c.x, c.y))))
                .sum();
            ring_area(p.exterior().0.iter().map(|c| (c.x, c.y))) - holes
        })
        .sum()
}

/// Asserts the sketch's bounds are `[x0, y0, x1, y1]` within 0.01 mm.
fn assert_bounds(sketch: &Sketch<()>, expected: [f64; 4]) {
    let bb = sketch.bounding_box();
    let got = [bb.mins.x, bb.mins.y, bb.maxs.x, bb.maxs.y];
    for (g, e) in got.iter().zip(expected) {
        assert!(
            (g - e).abs() <= 0.01,
            "bounds {got:?}, expected {expected:?}"
        );
    }
}

/// Asserts the area within 0.01 mm times `perimeter`.
fn assert_area(sketch: &Sketch<()>, expected: f64, perimeter: f64) {
    let got = area(sketch);
    assert!(
        (got - expected).abs() <= 0.01 * perimeter,
        "area {got}, expected {expected}"
    );
}

/// Area of the strip `|y| < h` inside a disc of radius `r`.
fn strip_in_disc(h: f64, r: f64) -> f64 {
    2.0 * (h * (r * r - h * h).sqrt() + r * r * (h / r).asin())
}

#[test]
fn test_gerber_region_square_is_filled() {
    // The stroke aperture does not matter inside a region.
    let s = geometry(
        "%ADD10C,0.5*%\nD10*\nG36*\nX0Y0D02*\nX10000000Y0D01*\nX10000000Y10000000D01*\n\
         X0Y10000000D01*\nX0Y0D01*\nG37*\n",
    );
    assert_area(&s, 100.0, 40.0);
    assert_bounds(&s, [0.0, 0.0, 10.0, 10.0]);
    assert!(
        s.to_multipolygon().0[0].interiors().is_empty(),
        "a filled region has no hole"
    );
}

#[test]
fn test_gerber_region_with_arcs_is_filled() {
    // Multi-quadrant (G75): a half disc of radius 5 above the X axis.
    let half = geometry("G36*\nX0Y0D02*\nX10000000Y0D01*\nG75*\nG03X0Y0I-5000000J0D01*\nG37*\n");
    assert_area(&half, PI * 25.0 / 2.0, 10.0 + PI * 5.0);
    assert_bounds(&half, [0.0, 0.0, 10.0, 5.0]);

    // Single quadrant (G74, unsigned I/J): a quarter disc of radius 10.
    let quarter = geometry(
        "G36*\nX0Y0D02*\nX10000000Y0D01*\nG74*\nG03X0Y10000000I10000000J0D01*\n\
         G01*\nX0Y0D01*\nG37*\n",
    );
    assert_area(&quarter, PI * 100.0 / 4.0, 20.0 + PI * 5.0);
    assert_bounds(&quarter, [0.0, 0.0, 10.0, 10.0]);
}

#[test]
fn test_gerber_region_cut_in_makes_a_hole() {
    // One contour: the outer square, a cut-in to a 4 x 4 square traversed the other way,
    // and back along the cut-in (how KiCad fractures a pour with a hole).
    let s = geometry(
        "G36*\nX0Y0D02*\nX10000000Y0D01*\nX10000000Y10000000D01*\nX0Y10000000D01*\n\
         X0Y0D01*\nX3000000Y3000000D01*\nX3000000Y7000000D01*\nX7000000Y7000000D01*\n\
         X7000000Y3000000D01*\nX3000000Y3000000D01*\nX0Y0D01*\nG37*\n",
    );
    assert_area(&s, 100.0 - 16.0, 56.0);
    assert_bounds(&s, [0.0, 0.0, 10.0, 10.0]);
    let polys = s.to_multipolygon().0;
    assert_eq!(polys.len(), 1);
    assert_eq!(polys[0].interiors().len(), 1, "the cut-in square is a hole");
}

#[test]
fn test_gerber_region_contours_are_separate_objects() {
    // A D02 inside one G36 starts a second contour.
    let s = geometry(
        "G36*\nX0Y0D02*\nX1000000Y0D01*\nX1000000Y1000000D01*\nX0Y1000000D01*\nX0Y0D01*\n\
         X5000000Y0D02*\nX7000000Y0D01*\nX7000000Y2000000D01*\nX5000000Y2000000D01*\n\
         X5000000Y0D01*\nG37*\n",
    );
    assert_area(&s, 1.0 + 4.0, 12.0);
    assert_eq!(s.to_multipolygon().0.len(), 2);
}

#[test]
fn test_gerber_macro_circle_outline_and_expressions_flash() {
    let circle = geometry("%AMCIRC*\n1,1,$1,0,0*%\n%ADD10CIRC,2*%\nD10*\nX5000000Y5000000D03*\n");
    assert_area(&circle, PI, 2.0 * PI);
    assert_bounds(&circle, [4.0, 4.0, 6.0, 6.0]);

    // Outline rectangle 2 x 1 rotated 90 degrees about the macro origin.
    let outline =
        geometry("%AMOUT*\n4,1,4,0,0,2,0,2,1,0,1,0,0,90*%\n%ADD11OUT*%\nD11*\nX0Y0D03*\n");
    assert_area(&outline, 2.0, 6.0);
    assert_bounds(&outline, [-1.0, 0.0, 0.0, 2.0]);

    // Center line with expressions and a variable: ($1+1) x ($3) where $3 = $2x2.
    let center_line = geometry(
        "%AMCL*\n0 comment*\n$3=$2x2*\n21,1,$1+1,$3,0,0,0*%\n%ADD12CL,1X0.5*%\nD12*\nX0Y0D03*\n",
    );
    assert_area(&center_line, 2.0, 6.0);
    assert_bounds(&center_line, [-1.0, -0.5, 1.0, 0.5]);

    // Vector line with exposure off cutting a polygon (5) in two.
    let cut =
        geometry("%AMCUT*\n5,1,4,0,0,2,45*\n20,0,0.2,-2,0,2,0,0*%\n%ADD13CUT*%\nD13*\nX0Y0D03*\n");
    // An axis-aligned square of side sqrt 2 (area 2) minus a 0.2 strip across its middle.
    assert_area(&cut, 2.0 - 0.2 * 2f64.sqrt(), 8.0);
    let h = std::f64::consts::FRAC_1_SQRT_2;
    assert_bounds(&cut, [-h, -h, h, h]);
    assert_eq!(cut.to_multipolygon().0.len(), 2, "the strip splits it");
}

#[test]
fn test_gerber_macro_thermal_flashes() {
    let s = geometry("%AMTH*\n7,0,0,4,2,0.5,0*%\n%ADD10TH*%\nD10*\nX0Y0D03*\n");
    let ring = PI * (4.0 - 1.0);
    let gaps = 2.0 * (strip_in_disc(0.25, 2.0) - strip_in_disc(0.25, 1.0));
    assert_area(&s, ring - gaps, 2.0 * PI * 3.0 + 8.0);
    // The gaps take the ring's extremes on the axes: the copper ends at the gap edge.
    let e = (4.0f64 - 0.25 * 0.25).sqrt();
    assert_bounds(&s, [-e, -e, e, e]);
    assert_eq!(s.to_multipolygon().0.len(), 4, "four spokes of copper");
}

#[test]
fn test_gerber_macro_kicad_round_rect_flashes() {
    // KiCad's rounded-rectangle pad macro, 2.0 x 1.5 mm with 0.25 mm corners.
    let s = geometry(
        "%AMRoundRect*\n0 Rectangle with rounded corners*\n0 $1 Rounding radius*\n\
         0 $2 $3 $4 $5 $6 $7 $8 $9 X,Y pos of 4 corners*\n\
         0 Add a 4 corners polygon primitive as box body*\n\
         4,1,4,$2,$3,$4,$5,$6,$7,$8,$9,$2,$3,0*\n\
         0 Add four circle primitives for the rounded corners*\n\
         1,1,$1+$1,$2,$3*\n1,1,$1+$1,$4,$5*\n1,1,$1+$1,$6,$7*\n1,1,$1+$1,$8,$9*\n\
         0 Add four rect primitives between the rounded corners*\n\
         20,1,$1+$1,$2,$3,$4,$5,0*\n20,1,$1+$1,$4,$5,$6,$7,0*\n\
         20,1,$1+$1,$6,$7,$8,$9,0*\n20,1,$1+$1,$8,$9,$2,$3,0*%\n\
         %ADD10RoundRect,0.25X-0.75X-0.5X0.75X-0.5X0.75X0.5X-0.75X0.5*%\n\
         D10*\nX10000000Y10000000D03*\n",
    );
    assert_area(&s, 3.0 - (4.0 - PI) * 0.0625, 7.0);
    assert_bounds(&s, [9.0, 9.25, 11.0, 10.75]);
}

#[test]
fn test_gerber_clear_polarity_cuts_a_hole_in_a_pour() {
    let pour = "G36*\nX0Y0D02*\nX10000000Y0D01*\nX10000000Y10000000D01*\nX0Y10000000D01*\n\
                X0Y0D01*\nG37*\n";
    let holed = geometry(&format!(
        "%ADD10R,4X2*%\n{pour}%LPC*%\nD10*\nX5000000Y5000000D03*\n"
    ));
    assert_area(&holed, 100.0 - 8.0, 52.0);
    assert_bounds(&holed, [0.0, 0.0, 10.0, 10.0]);
    let polys = holed.to_multipolygon().0;
    assert_eq!(polys.len(), 1);
    assert_eq!(polys[0].interiors().len(), 1, "the clear flash is a hole");

    // Dark objects after the clear one are added back on top, in order.
    let refilled = geometry(&format!(
        "%ADD10R,4X2*%\n%ADD11R,1X1*%\n{pour}%LPC*%\nD10*\nX5000000Y5000000D03*\n\
         %LPD*%\nD11*\nX5000000Y5000000D03*\n"
    ));
    assert_area(&refilled, 100.0 - 8.0 + 1.0, 56.0);
}

#[test]
fn test_gerber_drawn_constructs_are_not_counted_as_unsupported() {
    let region = file("G36*\nX0Y0D02*\nX10000000Y0D01*\nX10000000Y10000000D01*\nX0Y0D01*\nG37*\n");
    let polarity = file("%ADD10C,1*%\nD10*\n%LPC*%\nX0Y0D03*\n%LPD*%\n");
    let macro_pad = file("%AMPAD*\n1,1,1.0,0,0*%\n%ADD11PAD*%\nD11*\nX0Y0D03*\n");
    for content in [region, polarity, macro_pad] {
        assert_eq!(
            unsupported_constructs(&content),
            UnsupportedConstructs::default(),
            "{content}"
        );
    }
}

#[test]
fn test_gerber_undrawable_macro_and_step_repeat_are_counted() {
    // A moire primitive (6) and an undefined macro cannot be drawn.
    let moire = file("%AMMO*\n6,0,0,5,0.5,0.5,2,0.1,6,0*%\n%ADD10MO*%\nD10*\nX0Y0D03*\n");
    assert_eq!(unsupported_constructs(&moire).aperture_macros, 1);
    let undefined = file("%ADD10NOPE,1*%\nD10*\nX0Y0D03*\n");
    assert_eq!(unsupported_constructs(&undefined).aperture_macros, 1);

    let step = file("%ADD10C,1*%\n%SRX3Y2I5.0J5.0*%\nD10*\nX0Y0D03*\n%SR*%\n");
    let counts = unsupported_constructs(&step);
    assert_eq!(counts.step_repeat, 1, "{counts:?}");
    assert!(!counts.is_empty());
}
