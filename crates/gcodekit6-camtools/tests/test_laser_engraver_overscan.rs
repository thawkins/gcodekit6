//! Laser engraver overscan (audit T23): with `overscan_mm` set, every burn is preceded by a
//! laser-off run-up of at least that length in its own direction and followed by a run-out;
//! with the default of 0 the program is exactly the one the engraver made before overscan.

use gcodekit6_camtools::laser_engraver::{BitmapImageEngraver, EngravingParameters, ScanDirection};
use image::{DynamicImage, GrayImage, Luma};

/// A 6x4 image of black/white stripes: two separate burns on every row and column.
fn stripes() -> DynamicImage {
    DynamicImage::ImageLuma8(GrayImage::from_fn(6, 4, |x, y| {
        Luma([if (x / 2 + y) % 2 == 0 { 0 } else { 255 }])
    }))
}

fn engrave(params: EngravingParameters) -> String {
    BitmapImageEngraver::from_image(stripes(), params)
        .expect("engraver setup")
        .generate_gcode()
        .expect("gcode")
}

/// One parsed move: G1 or not, the travel along X and Y, and the S word (if any).
struct Move {
    feed: bool,
    dx: f32,
    dy: f32,
    s: Option<u32>,
}

impl Move {
    fn burns(&self) -> bool {
        self.feed && self.s.is_some_and(|s| s > 0)
    }
    fn laser_off_feed(&self) -> bool {
        self.feed && self.s == Some(0)
    }
    /// Travel along one axis only, as (axis, signed length); `None` for a diagonal or no move.
    fn along(&self) -> Option<(char, f32)> {
        match (self.dx.abs() > 1e-4, self.dy.abs() > 1e-4) {
            (true, false) => Some(('X', self.dx)),
            (false, true) => Some(('Y', self.dy)),
            _ => None,
        }
    }
}

/// The program's G0/G1 moves in order.
fn moves(gcode: &str) -> Vec<Move> {
    let (mut x, mut y) = (0.0f32, 0.0f32);
    let mut out = Vec::new();
    for line in gcode.lines() {
        let code = line.split(';').next().unwrap_or_default();
        let words: Vec<&str> = code.split_whitespace().collect();
        let feed = match words.first() {
            Some(&"G1") => true,
            Some(&"G0") => false,
            _ => continue,
        };
        let value = |axis: char| {
            words
                .iter()
                .find_map(|w| w.strip_prefix(axis))
                .and_then(|v| v.parse::<f32>().ok())
        };
        let (nx, ny) = (value('X').unwrap_or(x), value('Y').unwrap_or(y));
        let s = words
            .iter()
            .find_map(|w| w.strip_prefix('S'))
            .and_then(|v| v.parse().ok());
        out.push(Move {
            feed,
            dx: nx - x,
            dy: ny - y,
            s,
        });
        (x, y) = (nx, ny);
    }
    out
}

/// Checks every burn's laser-off run-up and run-out; returns the number of burns.
fn check_overscan(gcode: &str, overscan: f32) -> usize {
    let moves = moves(gcode);
    let mut burns = 0;
    for (i, m) in moves.iter().enumerate() {
        if !m.burns() {
            continue;
        }
        let (axis, len) = m.along().expect("a burn runs along one axis");
        let prev = &moves[i - 1];
        if prev.burns() {
            // A power change within the same burn: same axis and direction.
            let (p_axis, p_len) = prev.along().expect("burn axis");
            assert!(p_axis == axis && p_len.signum() == len.signum(), "{gcode}");
            continue;
        }
        burns += 1;
        assert!(
            prev.laser_off_feed(),
            "burn {i} has no laser-off run-up: {gcode}"
        );
        let (r_axis, r_len) = prev.along().expect("run-up axis");
        assert_eq!(r_axis, axis, "run-up on another axis: {gcode}");
        assert_eq!(
            r_len.signum(),
            len.signum(),
            "run-up the wrong way: {gcode}"
        );
        assert!(
            r_len.abs() >= overscan - 1e-3,
            "run-up {r_len} short: {gcode}"
        );
        // The run-out after this burn.
        let last = moves[i..]
            .iter()
            .position(|m| !m.burns())
            .map(|k| &moves[i + k])
            .expect("a run-out");
        assert!(last.laser_off_feed(), "no laser-off run-out: {gcode}");
        let (o_axis, o_len) = last.along().expect("run-out axis");
        assert_eq!(o_axis, axis, "{gcode}");
        assert_eq!(o_len.signum(), len.signum(), "{gcode}");
        assert!(
            o_len.abs() >= overscan - 1e-3,
            "run-out {o_len} short: {gcode}"
        );
    }
    burns
}

fn params(scan_direction: ScanDirection) -> EngravingParameters {
    EngravingParameters {
        width_mm: 0.6,
        pixels_per_mm: 10.0,
        scan_direction,
        bidirectional: true,
        offset_x: 0.0,
        offset_y: 0.0,
        ..EngravingParameters::default()
    }
}

#[test]
fn test_laser_overscan_runs_up_before_every_burn() {
    for direction in [ScanDirection::Horizontal, ScanDirection::Vertical] {
        let gcode = engrave(EngravingParameters {
            overscan_mm: 2.5,
            ..params(direction)
        });
        // 6 burns over 4 rows, or 12 over 6 columns.
        let burns = check_overscan(&gcode, 2.5);
        assert!(burns >= 6, "{direction:?}: {burns} burns: {gcode}");
        // The run-up is not clamped: at origin 0 it starts below 0 with the laser off.
        assert!(gcode.contains("-2.500"), "{direction:?}: {gcode}");
    }
}

#[test]
fn test_laser_overscan_must_be_finite_and_not_negative() {
    for overscan in [-1.0, f64::NAN, f64::INFINITY] {
        let result = BitmapImageEngraver::from_image(
            stripes(),
            EngravingParameters {
                overscan_mm: overscan,
                ..params(ScanDirection::Horizontal)
            },
        );
        assert!(result.is_err(), "{overscan}");
    }
}

/// The program without its `; Generated:` timestamp line.
fn untimed(gcode: &str) -> String {
    gcode
        .lines()
        .filter(|l| !l.starts_with("; Generated:"))
        .map(|l| format!("{l}\n"))
        .collect()
}

#[test]
fn test_laser_overscan_zero_keeps_the_old_program() {
    assert_eq!(EngravingParameters::default().overscan_mm, 0.0);
    let horizontal = engrave(params(ScanDirection::Horizontal));
    assert_eq!(untimed(&horizontal), OLD_HORIZONTAL);
    let vertical = engrave(params(ScanDirection::Vertical));
    assert_eq!(untimed(&vertical), OLD_VERTICAL);
}

/// The engraver's output before overscan existed (commit 4fbab79), timestamp removed.
const OLD_HORIZONTAL: &str = r#"; Laser Image Engraving G-code
; Image size: 0.60mm x 0.40mm
; Resolution: 10.0 pixels/mm
; Feed rate: 1000 mm/min
; Power range: 0%-100%
; Estimated time: 0.0 minutes
;
G21 ; Set units to millimeters
G90 ; Absolute positioning
G17 ; XY plane selection

; Runs in the active work coordinate system: set the X/Y/Z work zero first
; Image origin (lower-left) at X0.00 Y0.00

M5 ; Laser off

G0 X0.000 Y0.000
G0 X0.200 Y0.000 S0
G1 X0.400 S1000 F1000 M4
M5
M5 ; Laser off

G0 X0.600 Y0.100
G0 X0.600 Y0.100 S0
G1 X0.400 S1000 F1000 M4
M5
G0 X0.200 Y0.100 S0
G1 X0.000 S1000 F1000 M4
M5
M5 ; Laser off

G0 X0.000 Y0.200
G0 X0.200 Y0.200 S0
G1 X0.400 S1000 F1000 M4
M5
M5 ; Laser off

G0 X0.600 Y0.300
G0 X0.600 Y0.300 S0
G1 X0.400 S1000 F1000 M4
M5
G0 X0.200 Y0.300 S0
G1 X0.000 S1000 F1000 M4
M5

; End of engraving
M5 ; Laser off

G0 X0.000 Y0.000 ; Return to work origin offset
"#;

/// The engraver's vertical output before overscan existed (commit 4fbab79).
const OLD_VERTICAL: &str = r#"; Laser Image Engraving G-code
; Image size: 0.60mm x 0.40mm
; Resolution: 10.0 pixels/mm
; Feed rate: 1000 mm/min
; Power range: 0%-100%
; Estimated time: 0.0 minutes
;
G21 ; Set units to millimeters
G90 ; Absolute positioning
G17 ; XY plane selection

; Runs in the active work coordinate system: set the X/Y/Z work zero first
; Image origin (lower-left) at X0.00 Y0.00

M5
G0 X0.000 Y0.000
G0 X0.000 Y0.100 S0
G1 Y0.200 S1000 F1000 M4
M5
G0 X0.000 Y0.300 S0
G1 Y0.400 S1000 F1000 M4
M5
M5
G0 X0.100 Y0.400
G0 X0.100 Y0.400 S0
G1 Y0.300 S1000 F1000 M4
M5
G0 X0.100 Y0.200 S0
G1 Y0.100 S1000 F1000 M4
M5
M5
G0 X0.200 Y0.000
G0 X0.200 Y0.000 S0
G1 Y0.100 S1000 F1000 M4
M5
G0 X0.200 Y0.200 S0
G1 Y0.300 S1000 F1000 M4
M5
M5
G0 X0.300 Y0.400
G0 X0.300 Y0.300 S0
G1 Y0.200 S1000 F1000 M4
M5
G0 X0.300 Y0.100 S0
G1 Y0.000 S1000 F1000 M4
M5
M5
G0 X0.400 Y0.000
G0 X0.400 Y0.100 S0
G1 Y0.200 S1000 F1000 M4
M5
G0 X0.400 Y0.300 S0
G1 Y0.400 S1000 F1000 M4
M5
M5
G0 X0.500 Y0.400
G0 X0.500 Y0.400 S0
G1 Y0.300 S1000 F1000 M4
M5
G0 X0.500 Y0.200 S0
G1 Y0.100 S1000 F1000 M4
M5

; End of engraving
M5 ; Laser off

G0 X0.000 Y0.000 ; Return to work origin offset
"#;
