//! Board Outline layer (audit P2CAM-3, decision D-5): the cut runs one tool radius outside
//! the board edge (the outline stroke's centreline), so the board keeps its drawn size, and
//! goes down in passes of at most `outline_step_down` to `cut_depth`. A cutout inside the
//! board is cut on its inside.

use gcodekit6_camtools::gerber::{GerberConverter, GerberLayerType, GerberParameters};

/// A 40 x 30 mm board outline (0.1 mm stroke) with a 10 x 10 mm cutout at (15, 10).
const OUTLINE: &str = "%FSLAX24Y24*%\n%MOMM*%\n%ADD10C,0.1*%\nG01*\nD10*\n\
X0Y0D02*\nX400000Y0D01*\nX400000Y300000D01*\nX0Y300000D01*\nX0Y0D01*\n\
X150000Y100000D02*\nX250000Y100000D01*\nX250000Y200000D01*\nX150000Y200000D01*\n\
X150000Y100000D01*\nM02*\n";

fn outline_params() -> GerberParameters {
    GerberParameters {
        layer_type: GerberLayerType::BoardOutline,
        tool_diameter: 1.0,
        cut_depth: -1.6,
        outline_step_down: 0.5,
        ..Default::default()
    }
}

/// Distance from `(x, y)` to the edge of the axis-aligned rectangle `[x0, x1] x [y0, y1]`
/// (positive outside and inside alike).
fn distance_to_rect(x: f64, y: f64, (x0, y0, x1, y1): (f64, f64, f64, f64)) -> f64 {
    let dx = (x0 - x).max(x - x1).max(0.0);
    let dy = (y0 - y).max(y - y1).max(0.0);
    if dx > 0.0 || dy > 0.0 {
        dx.hypot(dy)
    } else {
        (x - x0).min(x1 - x).min(y - y0).min(y1 - y)
    }
}

/// One cut path: its XY points and its plunge depths.
type CutPath = (Vec<(f64, f64)>, Vec<f64>);

/// The cut moves of each path: (XY points, plunge depths), split at each `G0 X`.
fn paths(gcode: &str) -> Vec<CutPath> {
    let word = |line: &str, axis: char| -> Option<f64> {
        line.split_whitespace()
            .find_map(|w| w.strip_prefix(axis))
            .and_then(|v| v.parse().ok())
    };
    let mut out: Vec<CutPath> = Vec::new();
    for line in gcode.lines().filter(|l| !l.contains(';')) {
        if line.starts_with("G0 X") && !line.starts_with("G0 X0 Y0") {
            out.push((Vec::new(), Vec::new()));
        } else if let Some((points, depths)) = out.last_mut() {
            if line.starts_with("G1 Z") {
                depths.push(word(line, 'Z').expect("Z"));
            } else if line.starts_with("G1 X") {
                points.push((word(line, 'X').expect("X"), word(line, 'Y').expect("Y")));
            }
        }
    }
    out
}

#[test]
fn test_gerber_outline_cut_is_tool_radius_outside_the_board() {
    let gcode = GerberConverter::generate(&outline_params(), OUTLINE).expect("outline");
    let paths = paths(&gcode);
    assert_eq!(paths.len(), 2, "one loop per closed stroke:\n{gcode}");
    let board = (0.0, 0.0, 40.0, 30.0);
    let cutout = (15.0, 10.0, 25.0, 20.0);
    let mut seen = [false; 2];
    for (points, _) in &paths {
        let (x, y) = points[0];
        let inside_cutout = x > 15.0 && x < 25.0 && y > 10.0 && y < 20.0;
        let outside_board = !(0.0..=40.0).contains(&x) || !(0.0..=30.0).contains(&y);
        assert!(
            inside_cutout || outside_board,
            "cut on the board at ({x}, {y})"
        );
        let (rect, index) = if inside_cutout {
            (cutout, 1)
        } else {
            (board, 0)
        };
        seen[index] = true;
        for &(x, y) in points {
            let d = distance_to_rect(x, y, rect);
            assert!(
                (d - 0.5).abs() <= 0.01,
                "({x}, {y}) is {d} mm from the edge"
            );
            if index == 1 {
                assert!(x > 15.0 && x < 25.0 && y > 10.0 && y < 20.0, "({x}, {y})");
            } else {
                assert!(!(0.0..=40.0).contains(&x) || !(0.0..=30.0).contains(&y));
            }
        }
    }
    assert_eq!(seen, [true, true], "board and cutout both cut");
}

#[test]
fn test_gerber_outline_descends_in_step_down_passes() {
    let gcode = GerberConverter::generate(&outline_params(), OUTLINE).expect("outline");
    for (_, depths) in paths(&gcode) {
        assert_eq!(depths, [-0.5, -1.0, -1.5, -1.6]);
    }

    let thin = GerberParameters {
        cut_depth: -0.1,
        ..outline_params()
    };
    let gcode = GerberConverter::generate(&thin, OUTLINE).expect("thin");
    for (_, depths) in paths(&gcode) {
        assert_eq!(depths, [-0.1], "thin stock is one pass");
    }

    let bad = GerberParameters {
        outline_step_down: 0.0,
        ..outline_params()
    };
    assert!(GerberConverter::generate(&bad, OUTLINE).is_err());
}
