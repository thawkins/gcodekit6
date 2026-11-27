//! Locks `Visualizer::parse_gcode` as a pure re-parse after the hash-keyed
//! `ToolpathCache` was removed (audit T51, CAMVIS-3b): every call replaces the
//! commands with exactly those of the new program, and parsing the same text
//! again yields the same commands.

use gcodekit5_visualizer::{GCodeCommand, Visualizer};

fn move_targets(vis: &Visualizer) -> Vec<(f32, f32, f32)> {
    vis.commands()
        .iter()
        .map(|c| match c {
            GCodeCommand::Move { to, .. } | GCodeCommand::Arc { to, .. } => (to.x, to.y, to.z),
            GCodeCommand::Dwell { pos, .. } => (pos.x, pos.y, pos.z),
        })
        .collect()
}

#[test]
fn test_visualizer_reparse_replaces_commands() {
    let first = "G0 X1 Y1\nG1 X2 Y2\nG1 X3 Y3\nG2 X5 Y3 I1 J0";
    let second = "G0 X10 Y0\nG1 X20 Y0";

    let mut fresh = Visualizer::new();
    fresh.parse_gcode(second);

    let mut vis = Visualizer::new();
    vis.parse_gcode(first);
    assert_eq!(vis.commands().len(), 4);
    vis.parse_gcode(second);
    assert_eq!(move_targets(&vis), move_targets(&fresh));

    vis.parse_gcode(second);
    assert_eq!(
        move_targets(&vis),
        move_targets(&fresh),
        "same text again: same commands"
    );
}
