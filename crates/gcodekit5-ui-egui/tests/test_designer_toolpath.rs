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

/// Many quick changes: at most one build runs at a time, and the preview ends up matching
/// the final design.
#[test]
fn test_rapid_changes_run_one_build_at_a_time_and_end_current() {
    let mut pane = pane_with_rects(&[(10.0, 10.0, 10.0, 10.0)]);
    pane.state_mut().show_toolpaths = true;
    let mut h = harness(pane);
    for i in 0..20 {
        h.state_mut()
            .state_mut()
            .add_shape_with_undo(gcodekit5_designer::model::Shape::Rectangle(
                gcodekit5_designer::model::DesignRectangle::new(
                    10.0 + 6.0 * f64::from(i),
                    40.0,
                    5.0,
                    5.0,
                ),
            ));
        h.step();
        assert!(h.state().toolpath_builds_running() <= 1);
    }
    wait_for_preview(&mut h);
    let rapid = cut_points(h.state());
    // The same design previewed from scratch.
    let mut fresh = DesignerPane::new();
    for obj in h.state().state().canvas.shapes() {
        fresh.state_mut().add_shape_with_undo(obj.shape.clone());
    }
    fresh.state_mut().show_toolpaths = true;
    let mut f = harness(fresh);
    wait_for_preview(&mut f);
    let mut a = rapid.clone();
    let mut b = cut_points(f.state());
    a.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    b.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    assert_eq!(a, b);
}

/// No build starts while a shape is dragged; one runs after the release.
#[test]
fn test_no_preview_build_during_a_drag() {
    use crate::test_designer_canvas::{button, screen};
    use egui::{Modifiers, PointerButton};
    let mut pane = pane_with_rects(&[(20.0, 20.0, 40.0, 30.0)]);
    pane.state_mut().show_toolpaths = true;
    let mut h = harness(pane);
    wait_for_preview(&mut h);
    let before = h.state().toolpath_builds_started();
    let from = screen(&h, 40.0, 35.0);
    h.event(egui::Event::PointerMoved(from));
    h.step();
    button(&h, from, PointerButton::Primary, true, Modifiers::NONE);
    h.step();
    for t in 1..=8 {
        let to = screen(&h, 40.0 + 5.0 * t as f32, 35.0);
        h.event(egui::Event::PointerMoved(to));
        h.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(h.state().dragging());
    assert_eq!(
        h.state().toolpath_builds_started(),
        before,
        "no build mid-drag"
    );
    let to = screen(&h, 80.0, 35.0);
    button(&h, to, PointerButton::Primary, false, Modifiers::NONE);
    h.step();
    wait_for_preview(&mut h);
    assert!(h.state().toolpath_builds_started() > before);
    assert!(
        cut_points(h.state())
            .iter()
            .any(|&(x, _)| (x - 100.0).abs() < 0.01),
        "the preview follows the moved shape"
    );
}

/// Runs `n` frames a few milliseconds apart, so a build thread has time to end.
fn spin(h: &mut Harness<'_, DesignerPane>, n: usize) {
    for _ in 0..n {
        h.step();
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A preview build that panics is tried once per design revision, not once per frame; the
/// next edit tries again, and a working build then shows the design.
#[test]
fn test_panicking_preview_build_is_tried_once_per_revision() {
    let mut pane = pane_with_rects(&[(20.0, 20.0, 40.0, 30.0)]);
    pane.state_mut().show_toolpaths = true;
    pane.set_toolpath_build_panics(true);
    let mut h = harness(pane);
    spin(&mut h, 30);
    assert_eq!(
        h.state().toolpath_builds_started(),
        1,
        "rebuilt every frame"
    );
    assert!(
        h.state().preview_toolpaths().is_empty(),
        "nothing previewed for the failed revision"
    );
    // An edit is a new revision: one more try.
    h.state_mut()
        .state_mut()
        .add_shape_with_undo(gcodekit5_designer::model::Shape::Rectangle(
            gcodekit5_designer::model::DesignRectangle::new(80.0, 20.0, 10.0, 10.0),
        ));
    spin(&mut h, 30);
    assert_eq!(h.state().toolpath_builds_started(), 2);
    // Fixed: the next revision builds.
    h.state_mut().set_toolpath_build_panics(false);
    h.state_mut()
        .state_mut()
        .add_shape_with_undo(gcodekit5_designer::model::Shape::Rectangle(
            gcodekit5_designer::model::DesignRectangle::new(100.0, 20.0, 10.0, 10.0),
        ));
    wait_for_preview(&mut h);
    assert_eq!(h.state().toolpath_builds_started(), 3);
    assert!(!cut_points(h.state()).is_empty());
}

/// GTK's spinner and Cancel while the preview builds: the share of shapes done is shown
/// beside Show Toolpaths, Cancel stops the build and clears the preview, and the next design
/// change builds again.
#[test]
fn test_preview_build_shows_progress_and_cancel() {
    let mut pane = pane_with_rects(&[(20.0, 20.0, 40.0, 30.0)]);
    pane.set_toolpath_build_delay(Some(Duration::from_secs(5)));
    let mut h = harness(pane);
    h.state_mut().state_mut().show_toolpaths = true;
    h.step();
    h.step();
    assert_eq!(h.state().toolpath_builds_running(), 1);
    let _ = h.get_by_label("0%");
    h.get_by_label("Cancel").click();
    h.step();
    h.step();
    assert_eq!(h.state().toolpath_builds_running(), 0);
    assert!(
        h.query_by_label("Cancel").is_none(),
        "Cancel goes with the build"
    );
    assert!(h.state().preview_toolpaths().is_empty());
    for _ in 0..10 {
        h.step();
    }
    assert_eq!(
        h.state().toolpath_builds_started(),
        1,
        "no rebuild by itself"
    );
    h.state_mut().set_toolpath_build_delay(None);
    h.state_mut()
        .state_mut()
        .add_shape_with_undo(gcodekit5_designer::model::Shape::Rectangle(
            gcodekit5_designer::model::DesignRectangle::new(80.0, 20.0, 10.0, 10.0),
        ));
    wait_for_preview(&mut h);
    assert_eq!(h.state().toolpath_builds_started(), 2);
    assert!(!cut_points(h.state()).is_empty());
}
