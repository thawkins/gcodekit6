//! Locking tests for the laser segment optimiser and the arc-length ramp.
//!
//! Covers audit T09: the collinear merge must stay within
//! `curve_simplification_tolerance` of the source points (DESIGNER-4), the
//! laser point filter must not use a stale pre-arc reference or drop a
//! contour's closing point (P2DES-11), and ramps over arcs must descend at
//! the requested angle measured along the arc, not the chord (DESIGNER-3).

use gcodekit5_core::Units;
use gcodekit5_designer::model::{DesignCircle, Point};
use gcodekit5_designer::{
    Toolpath, ToolpathGenerator, ToolpathSegment, ToolpathSegmentType, ToolpathToGcode,
};

fn rapid(to: Point) -> ToolpathSegment {
    ToolpathSegment::new(ToolpathSegmentType::RapidMove, to, to, 800.0, 500)
}

fn line(from: Point, to: Point) -> ToolpathSegment {
    ToolpathSegment::new(ToolpathSegmentType::LinearMove, from, to, 800.0, 500)
}

/// Distance from `p` to the segment `a`-`b`.
fn dist_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len_sq = dx * dx + dy * dy;
    let t = if len_sq > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance_to(&Point::new(a.x + t * dx, a.y + t * dy))
}

/// Toolpath for a quarter arc of radius `r` flattened at 0.005 mm sag.
fn flattened_arc(r: f64) -> (Toolpath, Vec<Point>) {
    let step = 2.0 * (1.0 - 0.005 / r).acos();
    let n = (std::f64::consts::FRAC_PI_2 / step).ceil() as usize;
    let points: Vec<Point> = (0..=n)
        .map(|i| {
            let a = i as f64 / n as f64 * std::f64::consts::FRAC_PI_2;
            Point::new(r * a.cos(), r * a.sin())
        })
        .collect();
    let mut tp = Toolpath::new(0.1, 0.0);
    tp.add_segment(rapid(points[0]));
    for w in points.windows(2) {
        tp.add_segment(line(w[0], w[1]));
    }
    (tp, points)
}

#[test]
fn test_laser_merge_sag_stays_within_curve_tolerance() {
    let generator = ToolpathToGcode::new(Units::MM, 5.0).with_laser_2d();
    let tolerance = generator.curve_simplification_tolerance;
    let (tp, points) = flattened_arc(50.0);
    let optimized = generator.optimize_toolpath_for_laser(&tp);
    let cuts: Vec<&ToolpathSegment> = optimized
        .segments
        .iter()
        .filter(|s| s.segment_type == ToolpathSegmentType::LinearMove)
        .collect();
    assert!(cuts.len() < points.len() - 1, "nothing was merged");

    let sag = points
        .iter()
        .map(|p| {
            cuts.iter()
                .map(|s| dist_to_segment(*p, s.start, s.end))
                .fold(f64::INFINITY, f64::min)
        })
        .fold(0.0_f64, f64::max);
    assert!(
        sag <= tolerance + 1e-9,
        "merged chord sag {sag:.4} mm exceeds tolerance {tolerance} mm"
    );
}

#[test]
fn test_laser_merge_keeps_straight_lines_as_one_segment() {
    let generator = ToolpathToGcode::new(Units::MM, 5.0).with_laser_2d();
    let mut tp = Toolpath::new(0.1, 0.0);
    tp.add_segment(rapid(Point::new(0.0, 0.0)));
    for i in 0..10 {
        tp.add_segment(line(
            Point::new(i as f64, 0.0),
            Point::new(i as f64 + 1.0, 0.0),
        ));
    }
    let optimized = generator.optimize_toolpath_for_laser(&tp);
    assert_eq!(optimized.segments.len(), 2, "{:?}", optimized.segments);
    assert!(
        optimized.segments[1]
            .end
            .distance_to(&Point::new(10.0, 0.0))
            < 1e-9
    );
}

#[test]
fn test_laser_filter_keeps_closing_point_of_contour() {
    // The last edge is shorter than `min_point_distance`; the contour must still close.
    let generator = ToolpathToGcode::new(Units::MM, 5.0).with_laser_2d();
    let pts = [
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(10.0, 10.0),
        Point::new(0.0, 10.0),
        Point::new(0.05, 0.05),
        Point::new(0.0, 0.0),
    ];
    let mut tp = Toolpath::new(0.1, 0.0);
    tp.add_segment(rapid(pts[0]));
    for w in pts.windows(2) {
        tp.add_segment(line(w[0], w[1]));
    }

    let optimized = generator.optimize_toolpath_for_laser(&tp);
    let last = optimized.segments.last().expect("segments");
    assert!(
        last.end.distance_to(&pts[0]) < 1e-9,
        "contour ends at {:?}",
        last.end
    );
    for w in optimized.segments.windows(2) {
        assert!(
            w[0].end.distance_to(&w[1].start) < 1e-9,
            "gap between {:?} and {:?}",
            w[0].end,
            w[1].start
        );
    }

    let body = generator.generate_body(&optimized, 10);
    let last_move = body.lines().rfind(|l| l.contains('X')).expect("moves");
    assert!(last_move.contains("X0.000 Y0.000"), "{body}");
}

#[test]
fn test_laser_body_filter_resets_reference_after_arc() {
    // After the arc the tool is at (30, 0). The next move ends at (10.05, 0), 0.05 mm
    // from the pre-arc point (10, 0), but 20 mm from the real position.
    let generator = ToolpathToGcode::new(Units::MM, 5.0).with_laser_2d();
    let mut tp = Toolpath::new(0.1, 0.0);
    tp.add_segment(rapid(Point::new(0.0, 0.0)));
    tp.add_segment(line(Point::new(0.0, 0.0), Point::new(10.0, 0.0)));
    tp.add_segment(ToolpathSegment::new_arc(
        ToolpathSegmentType::ArcCCW,
        Point::new(10.0, 0.0),
        Point::new(30.0, 0.0),
        Point::new(20.0, 0.0),
        800.0,
        500,
    ));
    tp.add_segment(line(Point::new(30.0, 0.0), Point::new(10.05, 0.0)));
    tp.add_segment(line(Point::new(10.05, 0.0), Point::new(10.05, 5.0)));

    let body = generator.generate_body(&tp, 10);
    assert!(body.contains("X10.050 Y0.000"), "{body}");
}

#[test]
fn test_arc_length_is_measured_along_the_arc() {
    let r = 10.0;
    let quarter = ToolpathSegment::new_arc(
        ToolpathSegmentType::ArcCCW,
        Point::new(r, 0.0),
        Point::new(0.0, r),
        Point::new(0.0, 0.0),
        800.0,
        500,
    );
    assert!((quarter.arc_length() - r * std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    let three_quarter = ToolpathSegment::new_arc(
        ToolpathSegmentType::ArcCW,
        Point::new(r, 0.0),
        Point::new(0.0, r),
        Point::new(0.0, 0.0),
        800.0,
        500,
    );
    assert!((three_quarter.arc_length() - r * 1.5 * std::f64::consts::PI).abs() < 1e-9);
    let straight = line(Point::new(0.0, 0.0), Point::new(3.0, 4.0));
    assert!((straight.arc_length() - 5.0).abs() < 1e-12);
}

#[test]
fn test_ramp_on_circle_follows_requested_slope_along_arcs() {
    // r = 10: a lap is 62.8 mm, so at 3 degrees it can drop 3.29 mm. With a 3 mm
    // step-down three quarter arcs ramp at the full angle and the fourth reaches the floor.
    let circle = DesignCircle::new(Point::new(0.0, 0.0), 10.0);
    let mut generator = ToolpathGenerator::new();
    generator.set_start_depth(0.0);
    generator.set_cut_depth(6.0);
    generator.set_ramp_angle(3.0);
    let step_down = 3.0;
    let toolpaths = generator.generate_circle_contour(&circle, step_down);
    let tan_angle = 3.0_f64.to_radians().tan();

    let mut previous_floor = 0.0_f64;
    let mut full_ramps = 0;
    for tp in &toolpaths[..toolpaths.len() - 1] {
        let arcs: Vec<&ToolpathSegment> = tp
            .segments
            .iter()
            .filter(|s| s.segment_type == ToolpathSegmentType::ArcCCW)
            .collect();
        let floor = arcs
            .iter()
            .filter_map(|s| s.z_depth)
            .fold(f64::INFINITY, f64::min);
        assert!(
            (previous_floor - floor - step_down).abs() < 1e-6,
            "lap descends {} mm, expected {step_down}",
            previous_floor - floor
        );
        previous_floor = floor;

        for s in &arcs {
            let (Some(z0), Some(z1)) = (s.start_z, s.z_depth) else {
                continue;
            };
            let slope = (z0 - z1) / s.arc_length();
            assert!(slope <= tan_angle * 1.01, "arc too steep: {slope}");
            if (z1 - floor).abs() > 1e-9 {
                full_ramps += 1;
                assert!(
                    (slope / tan_angle - 1.0).abs() < 0.01,
                    "ramp slope {slope:.5} is not within 1% of {tan_angle:.5}"
                );
            }
        }
    }
    assert!(
        full_ramps >= 3,
        "expected full-angle ramp arcs, got {full_ramps}"
    );
}
