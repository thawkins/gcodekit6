//! Designer canvas editing and view: move (snap, axis lock), resize by handle (aspect lock,
//! the 5 mm / no-flip guard), rotate (about the centre), delete, undo/redo (ignored during a
//! drag), snap-to-grid, zoom at the cursor (within the zoom limits), zoom commands and pan.

use egui::{Modifiers, PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use gcodekit6_app::i18n::Translator;
use gcodekit6_ui_egui::commands::Command;
use gcodekit6_ui_egui::panes::designer::DesignerPane;
use gcodekit6_ui_egui::panes::designer::canvas::{DesignerTool, editing};
use gcodekit6_ui_egui::workspace::{Pane, PaneContext};

use crate::test_designer_canvas::{
    bounds, button, click, click_with, close, close4, drag, drag_with, draw_rect, harness, ids,
    pane_with_rects, press, screen, selected,
};

#[test]
fn test_drag_moves_selection_and_undo_restores() {
    let mut h = harness(pane_with_rects(&[(20.0, 20.0, 40.0, 40.0)]));
    let id = ids(h.state())[0];
    let (from, to) = (screen(&h, 40.0, 40.0), screen(&h, 50.0, 45.0));
    drag(&mut h, from, to);
    assert!(
        close4(bounds(h.state(), id), (30.0, 25.0, 70.0, 65.0)),
        "{:?}",
        bounds(h.state(), id)
    );
    // The move is one undo step, and the moved shape is still hit at its new place.
    h.state_mut().undo();
    h.run();
    assert!(close4(bounds(h.state(), id), (20.0, 20.0, 60.0, 60.0)));
    h.state_mut().redo();
    h.run();
    assert!(close4(bounds(h.state(), id), (30.0, 25.0, 70.0, 65.0)));
}

#[test]
fn test_resize_by_corner_handle_keeps_aspect() {
    let mut h = harness(pane_with_rects(&[(20.0, 20.0, 60.0, 40.0)]));
    let id = ids(h.state())[0];
    let p = screen(&h, 50.0, 40.0);
    click(&mut h, p);
    assert_eq!(selected(h.state()), vec![id]);
    // Top-right handle at (80, 60), dragged to (100, 70). New shapes lock their aspect
    // ratio (GTK's default), so the width leads: 80 wide, 80 / 1.5 high, anchored at the
    // bottom-left corner.
    let (from, to) = (screen(&h, 80.0, 60.0), screen(&h, 100.0, 70.0));
    drag(&mut h, from, to);
    let b = bounds(h.state(), id);
    assert!(close4(b, (20.0, 20.0, 100.0, 20.0 + 80.0 / 1.5)), "{b:?}");
    h.state_mut().undo();
    h.run();
    assert!(close4(bounds(h.state(), id), (20.0, 20.0, 80.0, 60.0)));
}

#[test]
fn test_resize_without_aspect_lock_follows_handle() {
    let mut pane = pane_with_rects(&[(20.0, 20.0, 60.0, 40.0)]);
    let id = ids(&pane)[0];
    if let Some(obj) = pane.state_mut().canvas.get_shape_mut(id) {
        obj.lock_aspect_ratio = false;
    }
    let mut h = harness(pane);
    let p = screen(&h, 50.0, 40.0);
    click(&mut h, p);
    // Bottom-left handle at (20, 20) dragged to (10, 5).
    let (from, to) = (screen(&h, 20.0, 20.0), screen(&h, 10.0, 5.0));
    drag(&mut h, from, to);
    assert!(close4(bounds(h.state(), id), (10.0, 5.0, 80.0, 60.0)));
}

#[test]
fn test_rotate_from_context_menu() {
    let mut h = harness(pane_with_rects(&[(20.0, 20.0, 60.0, 40.0)]));
    let id = ids(h.state())[0];
    let p = screen(&h, 50.0, 40.0);
    click(&mut h, p);
    click_with(&mut h, p, PointerButton::Secondary, Modifiers::NONE);
    h.get_by_label("Rotate").click();
    h.run();
    h.get_by_label("90° CCW").click();
    h.run();
    let rotation = h
        .state()
        .state()
        .canvas
        .get_shape(id)
        .map(|o| o.shape.rotation())
        .expect("shape");
    assert!(close(rotation, 90.0), "{rotation}");
    h.state_mut().undo();
    h.run();
    let rotation = h
        .state()
        .state()
        .canvas
        .get_shape(id)
        .map(|o| o.shape.rotation())
        .expect("shape");
    assert!(close(rotation, 0.0));
}

#[test]
fn test_rotate_by_is_relative() {
    let mut pane = pane_with_rects(&[(20.0, 20.0, 60.0, 40.0)]);
    pane.state_mut().select_all();
    pane.rotate_selected_by(-90.0);
    pane.rotate_selected_by(-90.0);
    let rotation = pane
        .state()
        .canvas
        .shapes()
        .map(|o| o.shape.rotation())
        .sum::<f64>();
    assert!(close(rotation.abs(), 180.0), "{rotation}");
}

#[test]
fn test_delete_key_removes_selection_and_undo_restores() {
    let mut h = harness(pane_with_rects(&[
        (20.0, 20.0, 40.0, 40.0),
        (100.0, 20.0, 40.0, 40.0),
    ]));
    let all = ids(h.state());
    let p = screen(&h, 40.0, 40.0);
    click(&mut h, p);
    press(&mut h, egui::Key::Delete);
    assert_eq!(ids(h.state()), vec![all[1]]);
    h.state_mut().undo();
    h.run();
    assert_eq!(
        ids(h.state()),
        all,
        "one undo restores the deleted shape in place"
    );
}

#[test]
fn test_escape_deselects() {
    let mut h = harness(pane_with_rects(&[(20.0, 20.0, 40.0, 40.0)]));
    let p = screen(&h, 40.0, 40.0);
    click(&mut h, p);
    assert_eq!(selected(h.state()).len(), 1);
    press(&mut h, egui::Key::Escape);
    assert!(selected(h.state()).is_empty());
}

#[test]
fn test_undo_redo_commands() {
    let mut h = harness(DesignerPane::new());
    draw_rect(&mut h, 20.0, 20.0, 80.0, 60.0);
    assert_eq!(ids(h.state()).len(), 1);
    let tr = Translator::new("en");
    let mut requested = Vec::new();
    let mut ctx = PaneContext::new(&tr, &mut requested);
    assert!(h.state_mut().command(Command::Undo, &mut ctx));
    assert!(ids(h.state()).is_empty());
    assert!(h.state_mut().command(Command::Redo, &mut ctx));
    assert_eq!(ids(h.state()).len(), 1);
    assert!(
        !h.state_mut().command(Command::Home, &mut ctx),
        "not the designer's"
    );
}

#[test]
fn test_snap_to_grid() {
    let mut h = harness(DesignerPane::new());
    assert!(
        h.state().state().snap_enabled,
        "snap is on by default (GTK)"
    );
    draw_rect(&mut h, 20.3, 20.4, 79.6, 60.2);
    let b = bounds(h.state(), ids(h.state())[0]);
    assert!(close4(b, (20.0, 20.0, 80.0, 60.0)), "{b:?}");

    let mut h = harness(DesignerPane::new());
    h.state_mut().state_mut().snap_enabled = false;
    draw_rect(&mut h, 20.3, 20.4, 79.6, 60.2);
    let b = bounds(h.state(), ids(h.state())[0]);
    assert!(
        (b.0 - 20.3).abs() < 0.05 && (b.3 - 60.2).abs() < 0.05,
        "{b:?}"
    );
}

#[test]
fn test_zoom_at_cursor_keeps_world_point_fixed() {
    let mut h = harness(DesignerPane::new());
    let p = screen(&h, 60.0, 90.0);
    h.event(egui::Event::PointerMoved(p));
    h.run();
    let before = h.state().view();
    let world = before.screen_to_world(p);
    for factor in [1.5, 0.5, 2.0] {
        h.event(egui::Event::Zoom(factor));
        h.run();
        let after = h.state().view();
        assert!((after.zoom() - before.zoom()).abs() > 1e-3, "zoom changed");
        let now = after.screen_to_world(p);
        assert!((now - world).length() < 1e-3, "{now:?} vs {world:?}");
    }
}

#[test]
fn test_zoom_commands_and_fit() {
    let mut h = harness(pane_with_rects(&[(20.0, 20.0, 40.0, 40.0)]));
    let tr = Translator::new("en");
    let mut requested = Vec::new();
    let mut ctx = PaneContext::new(&tr, &mut requested);
    let z0 = h.state().view().zoom();
    assert!(h.state_mut().command(Command::ZoomIn, &mut ctx));
    h.run();
    assert!((h.state().view().zoom() - z0 * 1.2).abs() < 1e-3);
    assert!(h.state_mut().command(Command::ZoomOut, &mut ctx));
    h.run();
    assert!((h.state().view().zoom() - z0).abs() < 1e-3);
    assert!(h.state_mut().command(Command::ZoomFit, &mut ctx));
    h.run();
    // The shape fills the view (5% padding each side).
    let view = h.state().view();
    let (a, b) = (screen(&h, 20.0, 20.0), screen(&h, 60.0, 60.0));
    assert!(view.rect().contains(a) && view.rect().contains(b));
    let side = (b.y - a.y).abs().max((b.x - a.x).abs());
    // The limiting side of the canvas (the toolbox leaves it narrower than tall).
    let limit = view.rect().width().min(view.rect().height());
    assert!(side > limit * 0.85, "{side}");
    assert!(h.state_mut().command(Command::ResetView, &mut ctx));
    h.run();
    assert!((h.state().view().zoom() - 1.0).abs() < 1e-6);
}

#[test]
fn test_middle_drag_pans() {
    let mut h = harness(DesignerPane::new());
    let before = screen(&h, 50.0, 50.0);
    let from = pos2(300.0, 250.0);
    drag_with(
        &mut h,
        from,
        from + egui::vec2(40.0, 30.0),
        PointerButton::Middle,
        Modifiers::NONE,
    );
    let after = screen(&h, 50.0, 50.0);
    assert!(
        (after - before - egui::vec2(40.0, 30.0)).length() < 0.5,
        "{before:?} -> {after:?}"
    );
    assert!(ids(h.state()).is_empty());
}

#[test]
fn test_pan_tool_pans() {
    let mut h = harness(pane_with_rects(&[(20.0, 20.0, 40.0, 40.0)]));
    h.state_mut().set_tool(DesignerTool::Pan);
    let before = screen(&h, 40.0, 40.0);
    drag(&mut h, before, before + egui::vec2(-25.0, 15.0));
    let after = screen(&h, 40.0, 40.0);
    assert!((after - before - egui::vec2(-25.0, 15.0)).length() < 0.5);
    let id = ids(h.state())[0];
    assert!(
        close4(bounds(h.state(), id), (20.0, 20.0, 60.0, 60.0)),
        "pan moves no shape"
    );
}

/// Runs one of the pane's commands outside a frame.
fn run_command(h: &mut egui_kittest::Harness<'_, DesignerPane>, command: Command) -> bool {
    let tr = Translator::new("en");
    let mut requested = Vec::new();
    let mut ctx = PaneContext::new(&tr, &mut requested);
    h.state_mut().command(command, &mut ctx)
}

#[test]
fn test_undo_and_delete_wait_for_a_resize_drag() {
    let mut pane = pane_with_rects(&[(20.0, 20.0, 60.0, 40.0)]);
    let id = ids(&pane)[0];
    if let Some(obj) = pane.state_mut().canvas.get_shape_mut(id) {
        obj.lock_aspect_ratio = false;
    }
    let mut h = harness(pane);
    let p = screen(&h, 50.0, 40.0);
    click(&mut h, p);
    // Grab the top-right handle and drag half way.
    let (from, mid, to) = (
        screen(&h, 80.0, 60.0),
        screen(&h, 90.0, 65.0),
        screen(&h, 100.0, 70.0),
    );
    h.event(egui::Event::PointerMoved(from));
    h.step();
    button(&h, from, PointerButton::Primary, true, Modifiers::NONE);
    h.step();
    h.event(egui::Event::PointerMoved(mid));
    h.step();
    assert!(h.state().dragging());
    // Cmd+Z (the app routes it as this command) and Delete arrive mid-drag.
    assert!(run_command(&mut h, Command::Undo));
    h.key_press(egui::Key::Delete);
    h.step();
    assert_eq!(ids(h.state()), vec![id], "the shape survives");
    h.event(egui::Event::PointerMoved(to));
    h.step();
    button(&h, to, PointerButton::Primary, false, Modifiers::NONE);
    h.run();
    assert!(!h.state().dragging());

    assert!(close4(bounds(h.state(), id), (20.0, 20.0, 100.0, 70.0)));
    // One undo step takes the shape back to where the drag started, the next removes it.
    h.state_mut().undo();
    h.run();
    assert!(close4(bounds(h.state(), id), (20.0, 20.0, 80.0, 60.0)));
    h.state_mut().undo();
    h.run();
    assert!(ids(h.state()).is_empty());
}

#[test]
fn test_resize_past_opposite_corner_keeps_minimum_and_no_flip() {
    let mut pane = pane_with_rects(&[(20.0, 20.0, 60.0, 40.0)]);
    let id = ids(&pane)[0];
    if let Some(obj) = pane.state_mut().canvas.get_shape_mut(id) {
        obj.lock_aspect_ratio = false;
    }
    let mut h = harness(pane);
    let p = screen(&h, 50.0, 40.0);
    click(&mut h, p);
    // Top-right handle dragged past the bottom-left corner.
    let (from, to) = (screen(&h, 80.0, 60.0), screen(&h, 0.0, 0.0));
    drag(&mut h, from, to);
    let b = bounds(h.state(), id);
    assert!(close(b.0, 20.0) && close(b.1, 20.0), "anchor stays: {b:?}");
    assert!(b.2 - b.0 >= 5.0 && b.3 - b.1 >= 5.0, "5 mm minimum: {b:?}");
}

#[test]
fn test_zoom_stays_within_limits_with_point_fixed() {
    let mut h = harness(DesignerPane::new());
    let p = screen(&h, 60.0, 90.0);
    h.event(egui::Event::PointerMoved(p));
    h.run();
    let world = h.state().view().screen_to_world(p);
    for (factor, limit) in [
        (1000.0, editing::ZOOM_LIMITS.1),
        (1e-5, editing::ZOOM_LIMITS.0),
    ] {
        for _ in 0..3 {
            h.event(egui::Event::Zoom(factor));
            h.run();
        }
        let view = h.state().view();
        assert!(
            (f64::from(view.zoom()) - limit).abs() < 1e-3,
            "{} vs {limit}",
            view.zoom()
        );
        let now = view.screen_to_world(p);
        assert!(
            (now - world).length() < 1e-2,
            "{now:?} vs {world:?} at zoom {}",
            view.zoom()
        );
    }
}

/// The centre of the selection box.
fn selection_centre(pane: &DesignerPane) -> (f64, f64) {
    let b = editing::selection_box(pane.state()).expect("a selection");
    ((b.0 + b.2) / 2.0, (b.1 + b.3) / 2.0)
}

#[test]
fn test_rotation_keeps_the_centre() {
    let mut pane = pane_with_rects(&[(20.0, 20.0, 60.0, 40.0)]);
    pane.state_mut().select_all();
    let before = selection_centre(&pane);
    pane.rotate_selected_by(90.0);
    let after = selection_centre(&pane);
    assert!(
        close(before.0, after.0) && close(before.1, after.1),
        "{before:?} {after:?}"
    );
    // Rotated a quarter turn, the 60 x 40 box is 40 x 60.
    let b = editing::selection_box(pane.state()).expect("a selection");
    assert!(close(b.2 - b.0, 40.0) && close(b.3 - b.1, 60.0), "{b:?}");

    // Several shapes turn about their common centre.
    let mut pane = pane_with_rects(&[(20.0, 20.0, 20.0, 20.0), (60.0, 20.0, 20.0, 20.0)]);
    pane.state_mut().select_all();
    let before = selection_centre(&pane);
    pane.rotate_selected_by(90.0);
    let after = selection_centre(&pane);
    assert!(
        close(before.0, after.0) && close(before.1, after.1),
        "{before:?} {after:?}"
    );
}

#[test]
fn test_move_snaps_to_grid() {
    let mut h = harness(pane_with_rects(&[(20.0, 20.0, 40.0, 40.0)]));
    let id = ids(h.state())[0];
    let (from, to) = (screen(&h, 40.3, 40.3), screen(&h, 50.6, 45.2));
    drag(&mut h, from, to);
    // From the snapped press (40, 40) to the snapped pointer (51, 45).
    let b = bounds(h.state(), id);
    assert!(close4(b, (31.0, 25.0, 71.0, 65.0)), "{b:?}");
}

#[test]
fn test_shift_locks_move_to_one_axis() {
    let mut h = harness(pane_with_rects(&[(20.0, 20.0, 40.0, 40.0)]));
    let id = ids(h.state())[0];
    let p = screen(&h, 40.0, 40.0);
    click(&mut h, p);
    let to = screen(&h, 50.0, 43.0);
    drag_with(&mut h, p, to, PointerButton::Primary, Modifiers::SHIFT);
    let b = bounds(h.state(), id);
    assert!(close4(b, (30.0, 20.0, 70.0, 60.0)), "{b:?}");
}

#[test]
fn test_shift_squares_box_and_snaps_line_to_45_degrees() {
    let mut h = harness(DesignerPane::new());
    h.state_mut().set_tool(DesignerTool::Rectangle);
    let (from, to) = (screen(&h, 20.0, 20.0), screen(&h, 80.0, 50.0));
    drag_with(&mut h, from, to, PointerButton::Primary, Modifiers::SHIFT);
    let rect = ids(h.state())[0];
    assert!(close4(bounds(h.state(), rect), (20.0, 20.0, 80.0, 80.0)));

    h.state_mut().set_tool(DesignerTool::Line);
    let (from, to) = (screen(&h, 20.0, 100.0), screen(&h, 80.0, 110.0));
    drag_with(&mut h, from, to, PointerButton::Primary, Modifiers::SHIFT);
    let line = ids(h.state())[1];
    let b = bounds(h.state(), line);
    // 9.5 degrees rounds to horizontal.
    assert!(close(b.1, 100.0) && close(b.3, 100.0), "{b:?}");

    let (from, to) = (screen(&h, 20.0, 140.0), screen(&h, 60.0, 178.0));
    drag_with(&mut h, from, to, PointerButton::Primary, Modifiers::SHIFT);
    let diagonal = ids(h.state())[2];
    let b = bounds(h.state(), diagonal);
    assert!(close(b.2 - b.0, b.3 - b.1), "45 degrees: {b:?}");
}

#[test]
fn test_ctrl_click_toggles_selection() {
    let mut h = harness(pane_with_rects(&[
        (20.0, 20.0, 40.0, 40.0),
        (100.0, 20.0, 40.0, 40.0),
    ]));
    let all = ids(h.state());
    let (a, b) = (screen(&h, 40.0, 40.0), screen(&h, 120.0, 40.0));
    click(&mut h, a);
    click_with(&mut h, b, PointerButton::Primary, Modifiers::COMMAND);
    assert_eq!(selected(h.state()), all);
    click_with(&mut h, a, PointerButton::Primary, Modifiers::COMMAND);
    assert_eq!(selected(h.state()), vec![all[1]]);
    // A Ctrl/Cmd click on empty space keeps the selection.
    let empty = screen(&h, 200.0, 200.0);
    click_with(&mut h, empty, PointerButton::Primary, Modifiers::COMMAND);
    assert_eq!(selected(h.state()), vec![all[1]]);
}
