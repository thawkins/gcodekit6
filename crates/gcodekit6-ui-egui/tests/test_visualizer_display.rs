//! The visualizer sidebar's Toolpath and Guides toggles (GTK's Show Rapid Moves, Show
//! Cutting Moves, Show Laser/Spindle, Show Grid and Grid spacing): their defaults, the
//! sidebar controls, and what the renderer draws with each off.

use std::sync::Arc;

use egui_kittest::kittest::Queryable;
use gcodekit6_ui_egui::panes::visualizer::sidebar::GRID_SPACINGS;
use gcodekit6_ui_egui::panes::visualizer::toolpath;
use gcodekit6_ui_egui::panes::visualizer::{self, Display, StdView, VisualizerPane};
use glam::Vec3;

use crate::test_visualizer::{gpu_harness, load, pane_harness, pixel_near};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A rapid from the origin to (30, 40), then a cut to (60, 40).
fn rapid_and_cut() -> Arc<[String]> {
    [
        "G21",
        "G90",
        "G0 X30 Y40",
        "M3 S1000",
        "G1 X60 Y40 F300",
        "M5",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Travel grey, as the renderer draws it.
fn is_travel(p: [u8; 4]) -> bool {
    p == toolpath::TRAVEL_COLOR
}

/// The cut colour at S1000.
fn is_cut(p: [u8; 4]) -> bool {
    p == [255, 255, 0, 255]
}

/// The grid's 30% grey, within rounding.
fn is_grid(p: [u8; 4]) -> bool {
    p[..3].iter().all(|c| c.abs_diff(77) <= 2)
}

#[test]
fn test_toolpath_marks_travel() {
    let mesh = toolpath::build(&rapid_and_cut());
    let travel: Vec<(u32, bool)> = mesh
        .vertices
        .iter()
        .map(|v| (v.line(), v.is_travel()))
        .collect();
    assert_eq!(
        travel,
        vec![(2, true), (2, true), (4, false), (4, false)],
        "the rapid is travel, the cut is not, and both keep their line"
    );
}

#[test]
fn test_display_starts_as_gtk() {
    let display = VisualizerPane::new().display();
    assert_eq!(
        display,
        Display {
            rapids: true,
            cuts: true,
            marker: true,
            grid: true,
            grid_spacing: 50.0,
            bounds: true,
        }
    );
    assert_eq!(GRID_SPACINGS, [1.0, 5.0, 10.0, 25.0, 50.0]);
}

#[test]
fn test_sidebar_toggles_change_the_display() {
    let mut harness = pane_harness(VisualizerPane::new());
    harness.run();
    for label in [
        "Show Rapid Moves",
        "Show Cutting Moves",
        "Show Laser/Spindle",
        "Show Grid",
        "Show Machine Bounds",
    ] {
        harness.get_by_label(label).click();
        harness.run();
    }
    let display = harness.state().display();
    assert!(!display.rapids && !display.cuts && !display.marker && !display.grid);
    assert!(!display.bounds);
    harness.get_by_label("Grid spacing").click();
    harness.run();
    harness.get_by_label("10.00 mm").click();
    harness.run();
    assert_eq!(harness.state().display().grid_spacing, 10.0);
}

/// Renders the rapid and the cut in the top view with `display`, and says whether the
/// travel, the cut and a grid line at (10, 35) (only on a 10 mm or finer grid) are drawn.
fn drawn(display: Display) -> Result<(bool, bool, bool), Box<dyn std::error::Error>> {
    let mut pane = VisualizerPane::new();
    pane.set_display(display);
    let mut harness = gpu_harness(pane);
    load(&mut harness, rapid_and_cut())?;
    harness
        .state_mut()
        .apply(visualizer::NavAction::View(StdView::Top));
    harness.run();
    let rendered = harness.render()?;
    let image = |x: u32, y: u32| rendered.get_pixel_checked(x, y).map(|p| p.0);
    let pane = harness.state();
    Ok((
        pixel_near(&image, pane, Vec3::new(15.0, 20.0, 0.0), is_travel),
        pixel_near(&image, pane, Vec3::new(45.0, 40.0, 0.0), is_cut),
        pixel_near(&image, pane, Vec3::new(10.0, 35.0, 0.0), is_grid),
    ))
}

#[test]
fn test_hidden_rapids_and_cuts_are_not_drawn() -> TestResult {
    let shown = Display::default();
    assert_eq!(drawn(shown)?, (true, true, false), "all drawn, 50 mm grid");
    let no_rapids = Display {
        rapids: false,
        ..shown
    };
    assert_eq!(drawn(no_rapids)?, (false, true, false));
    let no_cuts = Display {
        cuts: false,
        ..shown
    };
    assert_eq!(drawn(no_cuts)?, (true, false, false));
    Ok(())
}

#[test]
fn test_grid_spacing_and_hidden_grid() -> TestResult {
    let fine = Display {
        grid_spacing: 10.0,
        ..Display::default()
    };
    assert!(drawn(fine)?.2, "a 10 mm grid has a line at X10");
    let hidden = Display {
        grid: false,
        ..fine
    };
    assert!(!drawn(hidden)?.2, "no grid");
    Ok(())
}
