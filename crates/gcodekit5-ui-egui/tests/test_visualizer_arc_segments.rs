//! The Preferences "Arc Segment Length" reaches the visualizer: arcs are drawn with chords of
//! at most that length (and steps of at most 5 degrees, 4 to 128 segments), the default
//! draws exactly as the fixed 5-degree rule did, and a change rebuilds the program shown.

use std::time::{Duration, Instant};

use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::panes::visualizer::VisualizerPane;
use gcodekit5_ui_egui::panes::visualizer::toolpath::{self, ToolpathMesh};
use gcodekit5_ui_egui::workspace::Pane;
use gcodekit5_visualizer::gcode::arc;

use crate::test_visualizer::{load, pane_harness, sample};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The index of the sample's quarter arc (`G3 X20 Y50 I-20 J0`, radius 20 mm).
const ARC_LINE: u32 = 8;

/// The number of segments the sample's arc is drawn with in `mesh`.
fn arc_segments(mesh: &ToolpathMesh) -> usize {
    mesh.vertices
        .iter()
        .filter(|v| v.line() == ARC_LINE)
        .count()
        / 2
}

/// The configured default arc segment length.
fn configured_default() -> f64 {
    Config::default().file_processing.arc_segment_length
}

#[test]
fn test_visualizer_arc_default_draws_five_degree_steps() {
    assert_eq!(configured_default(), toolpath::DEFAULT_ARC_SEGMENT_LENGTH);
    let lines = sample();
    let mesh = toolpath::build(&lines);
    // A 90-degree arc in 5-degree steps, as before the setting was wired in.
    assert_eq!(arc_segments(&mesh), 18);
    assert_eq!(mesh, toolpath::build_with(&lines, configured_default()));
    // The default length leaves the old rule (5-degree steps, 4 to 128 segments) unchanged
    // for every arc up to a radius of a metre.
    for radius in [0.1, 1.0, 5.0, 20.0, 100.0, 500.0, 1000.0] {
        for degrees in [1.0_f64, 10.0, 90.0, 180.0, 359.0, 360.0] {
            let sweep = degrees.to_radians();
            let old = arc::ArcSweep {
                radius,
                start_angle: 0.0,
                sweep,
            }
            .segment_count(std::f64::consts::PI / 36.0, 4, 128);
            assert_eq!(
                toolpath::arc_segment_count(sweep, radius, configured_default()),
                old,
                "radius {radius} sweep {degrees}"
            );
        }
    }
}

#[test]
fn test_visualizer_arc_segment_length_changes_segments() {
    let lines = sample();
    // The arc is 20 * pi / 2 = 31.4 mm long.
    assert_eq!(arc_segments(&toolpath::build_with(&lines, 1.0)), 32);
    assert_eq!(arc_segments(&toolpath::build_with(&lines, 0.5)), 63);
    // Longer than a 5-degree step: the angle rule holds.
    assert_eq!(arc_segments(&toolpath::build_with(&lines, 5.0)), 18);
}

#[test]
fn test_visualizer_arc_segment_length_is_clamped() {
    let lines = sample();
    // Below the range: clamped to 0.01 mm, and the segment count is capped at 128.
    assert_eq!(arc_segments(&toolpath::build_with(&lines, 1e-9)), 128);
    assert_eq!(arc_segments(&toolpath::build_with(&lines, -3.0)), 128);
    assert_eq!(
        toolpath::arc_segment_count(std::f64::consts::TAU, 1e6, 0.01),
        128
    );
    // Not a number: the default.
    assert_eq!(arc_segments(&toolpath::build_with(&lines, f64::NAN)), 18);
    assert_eq!(
        toolpath::clamp_arc_segment_length(f64::INFINITY),
        *toolpath::ARC_SEGMENT_LENGTH_RANGE.end()
    );
}

#[test]
fn test_visualizer_arc_segment_length_setting_rebuilds() -> TestResult {
    let mut harness = pane_harness(VisualizerPane::new());
    harness.run();
    load(&mut harness, sample())?;
    let generation = harness.state().toolpath_generation();
    let points = harness.state().preview().len();
    let angles = harness.state().camera().angles();

    // The default again: nothing is rebuilt.
    harness.state_mut().configure(&Config::default());
    harness.run();
    assert!(!harness.state().is_building());
    assert_eq!(harness.state().toolpath_generation(), generation);

    let mut config = Config::default();
    config.file_processing.arc_segment_length = 1.0;
    harness.state_mut().configure(&config);
    harness.step();
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().is_building() {
        if Instant::now() > deadline {
            return Err("toolpath never rebuilt".into());
        }
        harness.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    harness.run();
    assert_ne!(harness.state().toolpath_generation(), generation);
    // The arc's 18 chords became 32, each adding one preview point.
    assert_eq!(harness.state().preview().len(), points + 14);
    // An edit-like rebuild keeps the user's view.
    assert_eq!(harness.state().camera().angles(), angles, "view kept");
    Ok(())
}

/// A program loaded after the change is built with it too.
#[test]
fn test_visualizer_arc_segment_length_applies_to_next_program() -> TestResult {
    let mut pane = VisualizerPane::new();
    let mut config = Config::default();
    config.file_processing.arc_segment_length = 1.0;
    pane.configure(&config);
    let mut harness = pane_harness(pane);
    harness.run();
    load(&mut harness, sample())?;
    let default_points = toolpath::build(&sample()).preview.len();
    assert_eq!(harness.state().preview().len(), default_points + 14);
    Ok(())
}
