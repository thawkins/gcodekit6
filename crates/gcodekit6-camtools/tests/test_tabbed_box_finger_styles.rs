//! Tabbed box finger styles Springs, Barbs and Snap: every panel outline stays closed and
//! simple, the fingers still match the mating notches within the play, each style changes
//! the outline, and joints a style cannot draw are refused.

use gcodekit6_camtools::tabbed_box::{BoxParameters, FingerStyle, Point, TabbedBoxMaker};

const STYLES: [FingerStyle; 3] = [FingerStyle::Springs, FingerStyle::Barbs, FingerStyle::Snap];

fn params(style: FingerStyle) -> BoxParameters {
    let mut p = BoxParameters::default();
    p.finger_joint.style = style;
    p
}

/// The panel outlines (not the slot holes) of a generated box.
fn outlines(p: BoxParameters) -> Vec<Vec<Point>> {
    let mut maker = TabbedBoxMaker::new(p).expect("valid parameters");
    maker.generate().expect("generates");
    maker
        .paths()
        .iter()
        .zip(maker.path_labels())
        .filter(|(_, label)| !label.contains("slot"))
        .map(|(path, _)| path.clone())
        .collect()
}

fn cross(o: &Point, a: &Point, b: &Point) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

/// Segments `ab` and `cd` cross or touch (collinear overlap included).
fn segments_meet(a: &Point, b: &Point, c: &Point, d: &Point) -> bool {
    const EPS: f64 = 1e-4;
    let on = |p: &Point, q: &Point, r: &Point| {
        r.x >= p.x.min(q.x) - EPS
            && r.x <= p.x.max(q.x) + EPS
            && r.y >= p.y.min(q.y) - EPS
            && r.y <= p.y.max(q.y) + EPS
    };
    let d1 = cross(c, d, a);
    let d2 = cross(c, d, b);
    let d3 = cross(a, b, c);
    let d4 = cross(a, b, d);
    if ((d1 > EPS && d2 < -EPS) || (d1 < -EPS && d2 > EPS))
        && ((d3 > EPS && d4 < -EPS) || (d3 < -EPS && d4 > EPS))
    {
        return true;
    }
    (d1.abs() <= EPS && on(c, d, a))
        || (d2.abs() <= EPS && on(c, d, b))
        || (d3.abs() <= EPS && on(a, b, c))
        || (d4.abs() <= EPS && on(a, b, d))
}

/// The first pair of non-adjacent segments of a closed outline that meet.
fn self_intersection(path: &[Point]) -> Option<(usize, usize)> {
    let n = path.len() - 1;
    for i in 0..n {
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue; // The closing segment shares the start point.
            }
            if segments_meet(&path[i], &path[i + 1], &path[j], &path[j + 1]) {
                return Some((i, j));
            }
        }
    }
    None
}

#[test]
fn test_tabbed_box_finger_styles_outlines_are_closed_and_simple() {
    let bowtie =
        [(0.0, 0.0), (1.0, 1.0), (1.0, 0.0), (0.0, 1.0), (0.0, 0.0)].map(|(x, y)| Point::new(x, y));
    assert!(
        self_intersection(&bowtie).is_some(),
        "the check finds a crossing"
    );
    for style in [FingerStyle::Rectangular].into_iter().chain(STYLES) {
        for (n, path) in outlines(params(style)).iter().enumerate() {
            let (first, last) = (&path[0], &path[path.len() - 1]);
            assert!(
                (first.x - last.x).abs() < 1e-4 && (first.y - last.y).abs() < 1e-4,
                "{style:?} panel {n} is not closed"
            );
            assert_eq!(
                self_intersection(path),
                None,
                "{style:?} panel {n} crosses itself"
            );
        }
    }
}

#[test]
fn test_tabbed_box_finger_styles_simple_with_play_extra_and_thick_stock() {
    let mut p = params(FingerStyle::Rectangular);
    p.x = 150.0;
    p.y = 120.0;
    p.h = 80.0;
    p.thickness = 6.0;
    p.burn = 0.2;
    p.finger_joint.play = 0.05;
    p.finger_joint.extra_length = 0.2;
    for style in STYLES {
        p.finger_joint.style = style;
        for (n, path) in outlines(p.clone()).iter().enumerate() {
            assert_eq!(
                self_intersection(path),
                None,
                "{style:?} panel {n} crosses itself"
            );
        }
    }
}

#[test]
fn test_tabbed_box_finger_styles_change_the_outline() {
    let plain = outlines(params(FingerStyle::Rectangular));
    for style in STYLES {
        let styled = outlines(params(style));
        assert_eq!(styled.len(), plain.len(), "{style:?} panel count");
        let changed = plain.iter().zip(&styled).any(|(a, b)| {
            a.len() != b.len()
                || a.iter()
                    .zip(b)
                    .any(|(p, q)| (p.x - q.x).abs() > 1e-3 || (p.y - q.y).abs() > 1e-3)
        });
        assert!(changed, "{style:?} draws the same outline as plain fingers");
    }
}

/// The x-ranges where an open edge path is crossed by the line `y = level`, with gaps
/// narrower than `merge` (a slit inside one finger) closed up.
fn cross_section(path: &[Point], level: f64, merge: f64) -> Vec<(f64, f64)> {
    let mut xs: Vec<f64> = path
        .windows(2)
        .filter(|w| (w[0].y - level) * (w[1].y - level) < 0.0)
        .map(|w| w[0].x + (level - w[0].y) / (w[1].y - w[0].y) * (w[1].x - w[0].x))
        .collect();
    xs.sort_by(f64::total_cmp);
    let mut ranges: Vec<(f64, f64)> = Vec::new();
    for pair in xs.chunks(2) {
        if let [a, b] = pair {
            match ranges.last_mut() {
                Some(last) if *a - last.1 < merge => last.1 = *b,
                _ => ranges.push((*a, *b)),
            }
        }
    }
    ranges
}

#[test]
fn test_tabbed_box_finger_styles_fingers_still_fit_the_notches() {
    for style in STYLES {
        let mut p = params(style);
        p.finger_joint.play = 0.02;
        let (t, kerf) = (p.thickness, p.burn);
        let play = p.finger_joint.play * t;
        let maker = TabbedBoxMaker::new(p).expect("valid parameters");
        for length in [100.0, 73.0] {
            let fingers = maker.finger_edge(length, true);
            let notches = maker.finger_edge(length, false);
            let base = -kerf / 2.0;
            // Just outside the panel edge for the fingers, just inside for the notches.
            let roots = cross_section(&fingers, base - 0.037 * t, 0.8 * t);
            let holes = cross_section(&notches, base + 0.037 * t, 0.8 * t);
            let (count, _) = maker.calc_fingers(length);
            assert_eq!(roots.len(), count, "{style:?} {length}: finger count");
            assert_eq!(holes.len(), count, "{style:?} {length}: notch count");
            for (f, n) in roots.iter().zip(&holes) {
                let centre = ((f.0 + f.1) - (n.0 + n.1)) / 2.0;
                assert!(
                    centre.abs() < 1e-3,
                    "{style:?} {length}: off centre {centre}"
                );
                let fit = (f.1 - f.0) - (n.1 - n.0);
                assert!(
                    fit.abs() <= play + 2.0 * kerf + 1e-3,
                    "{style:?} {length}: finger and notch widths differ by {fit}"
                );
            }
        }
    }
}

#[test]
fn test_tabbed_box_finger_styles_refuse_joints_they_cannot_draw() {
    let refused = |style: FingerStyle, change: fn(&mut BoxParameters)| {
        let mut p = params(style);
        change(&mut p);
        TabbedBoxMaker::new(p).err()
    };
    assert_eq!(
        refused(FingerStyle::Snap, |p| p.dividers_x = 1).as_deref(),
        Some("The Snap style cannot be used with dividers")
    );
    assert_eq!(
        refused(FingerStyle::Snap, |p| p.finger_joint.finger = 1.5).as_deref(),
        Some("Fingers are too narrow for the Snap style")
    );
    assert_eq!(
        refused(FingerStyle::Snap, |p| p.finger_joint.space = 1.0).as_deref(),
        Some("Spaces are too narrow for the Snap style")
    );
    assert_eq!(
        refused(FingerStyle::Snap, |p| {
            p.thickness = 5.0;
            p.x = 25.0;
        })
        .as_deref(),
        Some("The box is too small for the Snap style")
    );
    assert_eq!(
        refused(FingerStyle::Springs, |p| p.finger_joint.finger = 0.7).as_deref(),
        Some("Fingers are too narrow for the Springs style")
    );
    assert_eq!(
        refused(FingerStyle::Barbs, |p| p.finger_joint.space = 0.2).as_deref(),
        Some("Spaces are too narrow for the Barbs style")
    );
    // Plain fingers take all of those joints, and the burn and play checks still apply.
    assert!(refused(FingerStyle::Rectangular, |p| p.finger_joint.finger = 0.7).is_none());
    assert_eq!(
        refused(FingerStyle::Springs, |p| p.burn = -0.1).as_deref(),
        Some("Burn must not be negative and must be less than the finger and space widths")
    );
    // Springs and Barbs work with dividers; the half-lap edge keeps plain fingers.
    for style in [FingerStyle::Springs, FingerStyle::Barbs] {
        let mut p = params(style);
        p.dividers_x = 1;
        p.dividers_y = 1;
        let mut maker = TabbedBoxMaker::new(p).expect("valid parameters");
        assert!(maker.generate().is_ok(), "{style:?} with dividers");
    }
}

#[test]
fn test_tabbed_box_finger_styles_gcode_names_the_style() {
    let mut maker = TabbedBoxMaker::new(params(FingerStyle::Snap)).expect("valid parameters");
    maker.generate().expect("generates");
    assert!(maker.to_gcode().contains("; Finger Style: Snap\n"));
}
