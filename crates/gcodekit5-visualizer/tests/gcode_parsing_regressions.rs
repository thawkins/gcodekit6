//! Regression tests for G-code parsing and arc handling.

use gcodekit5_visualizer::{GCodeCommand, Visualizer};

fn parse(gcode: &str) -> Visualizer {
    let mut vis = Visualizer::new();
    vis.parse_gcode(gcode);
    vis
}

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn test_visualizer_g90_prefix_updates_motion_mode() {
    let vis = parse("G90 G0 X10\nG1 Y10");
    match &vis.commands()[0] {
        GCodeCommand::Move { rapid, to, .. } => {
            assert!(*rapid);
            assert!(approx(to.x, 10.0));
        }
        other => panic!("unexpected {:?}", other),
    }
    assert!(matches!(
        vis.commands()[1],
        GCodeCommand::Move { rapid: false, .. }
    ));
}

#[test]
fn test_visualizer_incremental_and_inch_modes() {
    let vis = parse("G91\nG1 X10\nX10");
    assert!(approx(vis.current_pos.x, 20.0));

    let vis = parse("G20 G1 X1");
    assert!(approx(vis.current_pos.x, 25.4));
}

#[test]
fn test_visualizer_paren_comment_axis_ignored() {
    let vis = parse("G0 X10 (to X99)");
    assert!(approx(vis.current_pos.x, 10.0));
}

#[test]
fn test_visualizer_arc_with_only_i_is_arc() {
    let vis = parse("G1 X0 Y0\nG2 X10 Y0 I5");
    let arc = vis
        .commands()
        .iter()
        .find_map(|c| match c {
            GCodeCommand::Arc { center, .. } => Some(*center),
            _ => None,
        })
        .expect("arc command");
    assert!(approx(arc.x, 5.0) && approx(arc.y, 0.0));
}

#[test]
fn test_visualizer_full_circle_is_split_and_bounded() {
    let vis = parse("G0 X10 Y0\nG2 X10 Y0 I-10 J0");
    let arcs: Vec<_> = vis
        .commands()
        .iter()
        .filter(|c| matches!(c, GCodeCommand::Arc { .. }))
        .collect();
    assert_eq!(arcs.len(), 2);
    for arc in arcs {
        if let GCodeCommand::Arc { from, to, .. } = arc {
            assert!(!(approx(from.x, to.x) && approx(from.y, to.y)));
        }
    }
}

#[test]
fn test_visualizer_negative_r_selects_long_arc() {
    let vis = parse("G0 X0 Y0\nG2 X10 Y0 R10\nG0 X0 Y0\nG2 X10 Y0 R-10");
    let centers: Vec<_> = vis
        .commands()
        .iter()
        .filter_map(|c| match c {
            GCodeCommand::Arc { center, .. } => Some(*center),
            _ => None,
        })
        .collect();
    assert!(approx(centers[0].x, 5.0) && approx(centers[0].y, -8.660));
    assert!(approx(centers[1].x, 5.0) && approx(centers[1].y, 8.660));
}

#[test]
fn test_visualizer_dwell_and_g92_do_not_move() {
    let vis = parse("G1 X5 F100\nG4 P1.5\nG4 X2\nG92 X0 Y0");
    let moves = vis
        .commands()
        .iter()
        .filter(|c| matches!(c, GCodeCommand::Move { .. }))
        .count();
    assert_eq!(moves, 1);
    assert!(
        vis.commands()
            .iter()
            .any(|c| matches!(c, GCodeCommand::Dwell { duration, .. } if approx(*duration, 1.5)))
    );
    assert!(approx(vis.current_pos.x, 0.0));
}

#[test]
fn test_visualizer_g18_arc_is_tessellated() {
    let vis = parse("G18\nG0 X0 Z0\nG2 X10 Z0 I5 K0");
    assert!(
        !vis.commands()
            .iter()
            .any(|c| matches!(c, GCodeCommand::Arc { .. }))
    );
    assert!(vis.commands().len() > 3);
    assert!(approx(vis.current_pos.x, 10.0));
}
