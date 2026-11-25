//! Designer canvas: every canvas interaction of the GTK `designer_canvas/input.rs` (draw by
//! drag with each tool, select, shift multi-select, marquee, move, resize by handle, rotate,
//! delete, undo/redo, snap-to-grid, zoom at the cursor, pan). The app routing is in
//! `test_designer_app.rs`, the snapshots in `test_designer_snapshots.rs`.

use egui::{Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_app::i18n::Translator;
use gcodekit5_designer::model::{DesignRectangle, Shape, ShapeType};
use gcodekit5_ui_egui::commands::Command;
use gcodekit5_ui_egui::panes::designer::canvas::{CanvasView, DesignerTool};
use gcodekit5_ui_egui::panes::designer::{DesignerPane, msgids};
use gcodekit5_ui_egui::workspace::{Pane, PaneContext, PaneId};
use glam::Vec2;

/// Seconds per harness frame (kittest's default of 0.25 s puts every click outside egui's
/// 0.3 s double-click window).
pub const STEP_DT: f32 = 1.0 / 60.0;

/// The pane in a harness without a GPU renderer, drawn with an English translator.
pub fn harness(pane: DesignerPane) -> Harness<'static, DesignerPane> {
    harness_at(pane, 1.0)
}

/// The pane in a harness at `ppp` pixels per point.
pub fn harness_at(pane: DesignerPane, ppp: f32) -> Harness<'static, DesignerPane> {
    let tr = Translator::new("en");
    let mut harness = Harness::builder()
        .with_size(egui::vec2(640.0, 480.0))
        .with_pixels_per_point(ppp)
        // Real frame times, so two clicks a few frames apart make a double-click.
        .with_step_dt(STEP_DT)
        .build_ui_state(
            move |ui, pane: &mut DesignerPane| {
                let mut requested = Vec::new();
                let mut ctx = PaneContext::new(&tr, &mut requested);
                pane.ui(ui, &mut ctx);
            },
            pane,
        );
    harness.run();
    harness
}

/// The screen point (in points) of world `(x, y)` mm.
pub fn screen(harness: &Harness<'_, DesignerPane>, x: f32, y: f32) -> Pos2 {
    harness.state().view().world_to_screen(Vec2::new(x, y))
}

/// Presses or releases `button` at `pos` with `modifiers` held.
fn button(
    harness: &Harness<'_, DesignerPane>,
    pos: Pos2,
    button: PointerButton,
    pressed: bool,
    modifiers: Modifiers,
) {
    harness.event(egui::Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers,
    });
}

/// Clicks `pos` with `button` and `modifiers` held.
pub fn click_with(
    harness: &mut Harness<'_, DesignerPane>,
    pos: Pos2,
    which: PointerButton,
    modifiers: Modifiers,
) {
    harness.event(egui::Event::ModifiersChanged(modifiers));
    harness.event(egui::Event::PointerMoved(pos));
    harness.step();
    button(harness, pos, which, true, modifiers);
    harness.step();
    button(harness, pos, which, false, modifiers);
    harness.step();
    harness.event(egui::Event::ModifiersChanged(Modifiers::NONE));
    harness.run();
}

/// Clicks `pos` with the primary button.
pub fn click(harness: &mut Harness<'_, DesignerPane>, pos: Pos2) {
    click_with(harness, pos, PointerButton::Primary, Modifiers::NONE);
}

/// Drags from `from` to `to` with `which` held, in a few pointer moves.
fn drag_with(
    harness: &mut Harness<'_, DesignerPane>,
    from: Pos2,
    to: Pos2,
    which: PointerButton,
    modifiers: Modifiers,
) {
    harness.event(egui::Event::ModifiersChanged(modifiers));
    harness.event(egui::Event::PointerMoved(from));
    harness.step();
    button(harness, from, which, true, modifiers);
    harness.step();
    for t in [0.25, 0.5, 0.75, 1.0] {
        harness.event(egui::Event::PointerMoved(from.lerp(to, t)));
        harness.step();
    }
    button(harness, to, which, false, modifiers);
    harness.step();
    harness.event(egui::Event::ModifiersChanged(Modifiers::NONE));
    harness.run();
}

/// Drags from `from` to `to` with the primary button.
pub fn drag(harness: &mut Harness<'_, DesignerPane>, from: Pos2, to: Pos2) {
    drag_with(harness, from, to, PointerButton::Primary, Modifiers::NONE);
}

/// Presses `key` (the canvas has the keyboard after a click).
pub fn press(harness: &mut Harness<'_, DesignerPane>, key: egui::Key) {
    harness.key_press(key);
    harness.run();
}

/// Draws a rectangle from world `(x1, y1)` to `(x2, y2)` with the Rectangle tool.
fn draw_rect(harness: &mut Harness<'_, DesignerPane>, x1: f32, y1: f32, x2: f32, y2: f32) {
    harness.state_mut().set_tool(DesignerTool::Rectangle);
    let (from, to) = (screen(harness, x1, y1), screen(harness, x2, y2));
    drag(harness, from, to);
    harness.state_mut().set_tool(DesignerTool::Select);
    harness.run();
}

/// A pane holding rectangles with the given `(x, y, w, h)`, none selected.
pub fn pane_with_rects(rects: &[(f64, f64, f64, f64)]) -> DesignerPane {
    let mut pane = DesignerPane::new();
    for &(x, y, w, h) in rects {
        pane.state_mut()
            .add_shape_with_undo(Shape::Rectangle(DesignRectangle::new(x, y, w, h)));
    }
    pane.state_mut().deselect_all();
    pane
}

/// The ids of the selected shapes, in draw order.
pub fn selected(pane: &DesignerPane) -> Vec<u64> {
    pane.state()
        .canvas
        .shape_store
        .draw_order_iter()
        .filter(|id| {
            pane.state()
                .canvas
                .get_shape(*id)
                .is_some_and(|o| o.selected)
        })
        .collect()
}

/// The ids of every shape, in draw order.
pub fn ids(pane: &DesignerPane) -> Vec<u64> {
    pane.state().canvas.shape_store.draw_order_iter().collect()
}

/// The bounds of shape `id`.
fn bounds(pane: &DesignerPane, id: u64) -> (f64, f64, f64, f64) {
    use gcodekit5_designer::model::DesignerShape;
    pane.state()
        .canvas
        .get_shape(id)
        .map(|o| o.shape.bounds())
        .expect("shape exists")
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-3
}

fn close4(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    close(a.0, b.0) && close(a.1, b.1) && close(a.2, b.2) && close(a.3, b.3)
}

#[test]
fn test_view_round_trips_screen_and_world() {
    let view = CanvasView::from_parts(
        egui::Rect::from_min_size(pos2(10.0, 20.0), egui::vec2(400.0, 300.0)),
        2.0,
        Vec2::new(5.0, 7.0),
    );
    // World (0, 0) sits pan points right of the left edge and above the bottom edge.
    assert_eq!(view.world_to_screen(Vec2::ZERO), pos2(15.0, 313.0));
    // +Y in the world is up on screen.
    assert!(view.world_to_screen(Vec2::new(0.0, 10.0)).y < 313.0);
    let p = pos2(123.0, 45.0);
    let back = view.world_to_screen(view.screen_to_world(p));
    assert!((back - p).length() < 1e-4, "{back:?}");
}

#[test]
fn test_draw_rectangle_by_drag() {
    let mut h = harness(DesignerPane::new());
    draw_rect(&mut h, 20.0, 20.0, 80.0, 60.0);
    let shapes = ids(h.state());
    assert_eq!(shapes.len(), 1);
    let obj = h
        .state()
        .state()
        .canvas
        .get_shape(shapes[0])
        .expect("shape");
    assert_eq!(obj.shape.shape_type(), ShapeType::Rectangle);
    assert!(close4(
        bounds(h.state(), shapes[0]),
        (20.0, 20.0, 80.0, 60.0)
    ));
    assert!(
        h.state().is_modified(),
        "a new shape marks the design modified"
    );
}

#[test]
fn test_tiny_drag_draws_nothing() {
    let mut h = harness(DesignerPane::new());
    // Under GTK's 1 mm minimum in one direction.
    draw_rect(&mut h, 20.0, 20.0, 80.0, 20.4);
    assert!(ids(h.state()).is_empty());
}

#[test]
fn test_each_drawing_tool_draws_its_shape() {
    let tools = [
        (DesignerTool::Circle, ShapeType::Circle),
        (DesignerTool::Line, ShapeType::Line),
        (DesignerTool::Ellipse, ShapeType::Ellipse),
        (DesignerTool::Triangle, ShapeType::Triangle),
        (DesignerTool::Polygon, ShapeType::Polygon),
        (DesignerTool::Gear, ShapeType::Gear),
        (DesignerTool::Sprocket, ShapeType::Sprocket),
    ];
    for (tool, kind) in tools {
        let mut h = harness(DesignerPane::new());
        h.state_mut().set_tool(tool);
        let (from, to) = (screen(&h, 20.0, 20.0), screen(&h, 80.0, 80.0));
        drag(&mut h, from, to);
        let shapes = ids(h.state());
        assert_eq!(shapes.len(), 1, "{tool:?}");
        let obj = h
            .state()
            .state()
            .canvas
            .get_shape(shapes[0])
            .expect("shape");
        assert_eq!(obj.shape.shape_type(), kind, "{tool:?}");
    }
}

#[test]
fn test_polyline_clicks_then_enter_makes_open_path() {
    let mut h = harness(DesignerPane::new());
    h.state_mut().set_tool(DesignerTool::Polyline);
    for (x, y) in [(10.0, 10.0), (60.0, 10.0), (60.0, 50.0)] {
        let p = screen(&h, x, y);
        click(&mut h, p);
        // Stay clear of the double-click window.
        h.run_steps(30);
    }
    assert_eq!(h.state().polyline_points().len(), 3);
    press(&mut h, egui::Key::Enter);
    let shapes = ids(h.state());
    assert_eq!(shapes.len(), 1);
    let obj = h
        .state()
        .state()
        .canvas
        .get_shape(shapes[0])
        .expect("shape");
    let Shape::Path(path) = &obj.shape else {
        panic!("not a path: {:?}", obj.shape.shape_type());
    };
    assert!(!path.closed, "Enter finishes an open path");
    assert!(h.state().polyline_points().is_empty());
}

#[test]
fn test_polyline_double_click_closes_and_escape_cancels() {
    let mut h = harness(DesignerPane::new());
    h.state_mut().set_tool(DesignerTool::Polyline);
    for (x, y) in [(10.0, 10.0), (60.0, 10.0)] {
        let p = screen(&h, x, y);
        click(&mut h, p);
        h.run_steps(30);
    }
    let last = screen(&h, 60.0, 50.0);
    click(&mut h, last);
    click(&mut h, last);
    let shapes = ids(h.state());
    assert_eq!(shapes.len(), 1);
    let obj = h
        .state()
        .state()
        .canvas
        .get_shape(shapes[0])
        .expect("shape");
    let Shape::Path(path) = &obj.shape else {
        panic!("not a path");
    };
    assert!(path.closed, "double-click finishes a closed path");

    let p = screen(&h, 100.0, 100.0);
    click(&mut h, p);
    assert_eq!(h.state().polyline_points().len(), 1);
    press(&mut h, egui::Key::Escape);
    assert!(h.state().polyline_points().is_empty());
    assert_eq!(ids(h.state()).len(), 1, "Escape draws nothing");
}

#[test]
fn test_click_selects_shape_and_empty_click_deselects() {
    let mut h = harness(pane_with_rects(&[
        (20.0, 20.0, 40.0, 40.0),
        (100.0, 20.0, 40.0, 40.0),
    ]));
    let all = ids(h.state());
    let p = screen(&h, 120.0, 40.0);
    click(&mut h, p);
    assert_eq!(selected(h.state()), vec![all[1]]);
    let p = screen(&h, 40.0, 40.0);
    click(&mut h, p);
    assert_eq!(
        selected(h.state()),
        vec![all[0]],
        "a plain click replaces the selection"
    );
    let p = screen(&h, 200.0, 200.0);
    click(&mut h, p);
    assert!(selected(h.state()).is_empty());
}

#[test]
fn test_shift_click_adds_to_selection() {
    let mut h = harness(pane_with_rects(&[
        (20.0, 20.0, 40.0, 40.0),
        (100.0, 20.0, 40.0, 40.0),
    ]));
    let all = ids(h.state());
    let p = screen(&h, 40.0, 40.0);
    click(&mut h, p);
    let p = screen(&h, 120.0, 40.0);
    click_with(&mut h, p, PointerButton::Primary, Modifiers::SHIFT);
    assert_eq!(selected(h.state()), all);
    // A plain click on one of them collapses the selection to it (on release).
    let p = screen(&h, 120.0, 40.0);
    click(&mut h, p);
    assert_eq!(selected(h.state()), vec![all[1]]);
}

#[test]
fn test_marquee_selects_shapes_it_touches() {
    let mut h = harness(pane_with_rects(&[
        (20.0, 20.0, 40.0, 40.0),
        (100.0, 20.0, 40.0, 40.0),
        (20.0, 150.0, 20.0, 20.0),
    ]));
    let all = ids(h.state());
    let (from, to) = (screen(&h, 10.0, 10.0), screen(&h, 150.0, 70.0));
    drag(&mut h, from, to);
    assert_eq!(selected(h.state()), vec![all[0], all[1]]);
}

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
    assert!(side > view.rect().height() * 0.85, "{side}");
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

#[test]
fn test_tool_labels_are_msgids() {
    let all = msgids();
    for tool in DesignerTool::ALL {
        assert!(all.contains(&tool.label_msgid()), "{tool:?}");
    }
    assert_eq!(PaneId::Designer, DesignerPane::new().id());
}
