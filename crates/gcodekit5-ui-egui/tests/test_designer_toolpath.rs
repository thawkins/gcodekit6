//! Designer toolpath preview: the toolbox's "Show Toolpaths" builds the toolpaths the G-code
//! is made from (in the background) and keeps them up to date with the design; hidden
//! shapes are not previewed; the exported G-code cuts what the preview shows.

use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_designer::toolpath::ToolpathSegmentType;
use gcodekit5_ui_egui::panes::designer::DesignerPane;

use crate::test_designer_canvas::{harness, pane_with_rects};

/// Runs frames until the preview is built for the current design, at most 10 s.
pub fn wait_for_preview(h: &mut Harness<'_, DesignerPane>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        h.step();
        if h.state().toolpath_preview_ready() {
            h.run();
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the preview was not built in time"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The cutting (non-rapid) segment end points of the preview, rounded to 0.001 mm.
fn cut_points(pane: &DesignerPane) -> Vec<(f64, f64)> {
    pane.preview_toolpaths()
        .iter()
        .flat_map(|tp| tp.segments.iter())
        .filter(|s| s.segment_type != ToolpathSegmentType::RapidMove)
        .map(|s| {
            (
                (s.end.x * 1000.0).round() / 1000.0,
                (s.end.y * 1000.0).round() / 1000.0,
            )
        })
        .collect()
}

#[test]
fn test_show_toolpaths_previews_the_design() {
    let mut h = harness(pane_with_rects(&[(20.0, 20.0, 40.0, 30.0)]));
    assert!(!h.state().state().show_toolpaths);
    assert!(h.state().preview_toolpaths().is_empty());
    h.get_by_label("Show Toolpaths").click();
    h.run();
    assert!(h.state().state().show_toolpaths);
    wait_for_preview(&mut h);
    let points = cut_points(h.state());
    assert!(!points.is_empty(), "the rectangle is cut");
    // The preview follows edits: a second rectangle adds its toolpath.
    h.state_mut()
        .state_mut()
        .add_shape_with_undo(gcodekit5_designer::model::Shape::Rectangle(
            gcodekit5_designer::model::DesignRectangle::new(100.0, 100.0, 20.0, 20.0),
        ));
    wait_for_preview(&mut h);
    assert!(cut_points(h.state()).len() > points.len());
    // Hidden again: the switch turns the preview off.
    h.get_by_label("Show Toolpaths").click();
    h.run();
    assert!(!h.state().state().show_toolpaths);
}

#[test]
fn test_hidden_shapes_are_not_previewed() {
    let mut pane = pane_with_rects(&[(20.0, 20.0, 40.0, 30.0), (100.0, 100.0, 20.0, 20.0)]);
    pane.state_mut().show_toolpaths = true;
    let mut h = harness(pane);
    wait_for_preview(&mut h);
    let both = cut_points(h.state()).len();
    let hidden = h.state().state().canvas.shapes().nth(1).map(|o| o.id);
    h.state_mut()
        .set_hidden(hidden.expect("second shape"), true);
    wait_for_preview(&mut h);
    let points = cut_points(h.state());
    assert!(points.len() < both, "the hidden shape's toolpath is gone");
    assert!(
        points.iter().all(|&(x, y)| x < 90.0 && y < 90.0),
        "{points:?}"
    );
}

/// "Export G-code via toolpath preview": the exported program cuts every point the preview
/// shows, so what the user sees is what is cut.
#[test]
fn test_exported_gcode_cuts_what_the_preview_shows() {
    let mut pane = pane_with_rects(&[(20.0, 20.0, 40.0, 30.0)]);
    pane.state_mut().show_toolpaths = true;
    let mut h = harness(pane);
    wait_for_preview(&mut h);
    let gcode = h.state().generate_gcode(None);
    let points = cut_points(h.state());
    assert!(!points.is_empty());
    for (x, y) in points {
        let x = format!("X{x:.3}");
        let y = format!("Y{y:.3}");
        // A feed move (G01, or a modal coordinate line after one), never a rapid.
        assert!(
            gcode.lines().any(|l| !l.starts_with("G0 ")
                && !l.starts_with("G00")
                && l.contains(&x)
                && l.contains(&y)),
            "{x} {y} not cut in:\n{gcode}"
        );
    }
}
