//! Designer layers panel: the list in drawing order, selection synced both ways with the
//! canvas, visibility (a hidden shape is not drawn and not hit-testable), lock, rename, the
//! position field, the z-order buttons, group and ungroup, and Delete. Every change to the
//! design is one undo step and marks the design modified.

use std::cell::RefCell;
use std::rc::Rc;

use egui::{Modifiers, PointerButton, Pos2};
use egui_kittest::kittest::Queryable;
use egui_kittest::{Harness, SnapshotOptions};
use gcodekit5_app::i18n::Translator;
use gcodekit5_designer::model::Point;
use gcodekit5_ui_egui::panes::designer::DesignerPane;
use gcodekit5_ui_egui::panes::designer::canvas::picking::topmost_shape_at;
use gcodekit5_ui_egui::panes::designer::canvas::rendering::{self, LineWidths};
use gcodekit5_ui_egui::panes::designer::canvas::{CanvasInput, CanvasView};
use gcodekit5_ui_egui::panes::designer::layers::LayersPane;
use gcodekit5_ui_egui::theme;
use gcodekit5_ui_egui::workspace::{Pane, PaneContext};

use crate::test_designer_canvas::{STEP_DT, click, click_with, ids, pane_with_rects, selected};

/// The Designer and its Layers pane side by side, over one shared design.
pub struct Both {
    pub designer: Rc<RefCell<DesignerPane>>,
    pub layers: LayersPane,
}

/// Three 20 x 20 mm squares along X, ids in `ids()` order, none selected.
pub fn three() -> Rc<RefCell<DesignerPane>> {
    let rects = [
        (10.0, 10.0, 20.0, 20.0),
        (50.0, 10.0, 20.0, 20.0),
        (90.0, 10.0, 20.0, 20.0),
    ];
    Rc::new(RefCell::new(pane_with_rects(&rects)))
}

/// Both panes in a harness at `ppp` pixels per point, without a GPU renderer.
pub fn both_at(designer: Rc<RefCell<DesignerPane>>, ppp: f32) -> Harness<'static, Both> {
    let tr = Translator::new("en");
    let layers = LayersPane::new(Rc::clone(&designer));
    let mut h = Harness::builder()
        .with_size(egui::vec2(800.0, 480.0))
        .with_pixels_per_point(ppp)
        .with_step_dt(STEP_DT)
        .build_ui_state(
            move |ui, both: &mut Both| {
                let mut requested = Vec::new();
                let mut ctx = PaneContext::new(&tr, &mut requested);
                egui::Panel::right("layers")
                    .default_size(330.0)
                    .show(ui, |ui| both.layers.ui(ui, &mut ctx));
                both.designer.borrow_mut().ui(ui, &mut ctx);
            },
            Both { designer, layers },
        );
    h.run();
    h
}

/// Both panes at 1 pixel per point.
fn both(designer: Rc<RefCell<DesignerPane>>) -> Harness<'static, Both> {
    both_at(designer, 1.0)
}

/// The ids of the selected shapes.
pub fn sel(h: &Harness<'_, Both>) -> Vec<u64> {
    selected(&h.state().designer.borrow())
}

/// The shape ids in drawing order.
pub fn order(h: &Harness<'_, Both>) -> Vec<u64> {
    ids(&h.state().designer.borrow())
}

/// Clicks the row label `Rect #id` with `modifiers` held.
pub fn click_row(h: &mut Harness<'_, Both>, id: u64, modifiers: Modifiers) {
    let pos = h.get_by_label(&format!("Rect #{id}")).rect().center();
    click_with(h, pos, PointerButton::Primary, modifiers);
}

/// The centre of the row label `Rect #id`.
fn row_pos(h: &Harness<'_, Both>, id: u64) -> Pos2 {
    h.get_by_label(&format!("Rect #{id}")).rect().center()
}

/// The screen point of the middle of shape `index` (0-based) on the canvas.
fn canvas_pos(h: &Harness<'_, Both>, index: usize) -> Pos2 {
    let x = 20.0 + 40.0 * index as f32;
    h.state()
        .designer
        .borrow()
        .view()
        .world_to_screen(glam::Vec2::new(x, 20.0))
}

/// The centre of row `row`'s (0-based) toggle, whichever of `labels` it shows now.
pub fn toggle_pos(h: &Harness<'_, Both>, labels: [&str; 2], row: usize) -> Pos2 {
    let mut found: Vec<Pos2> = labels
        .iter()
        .flat_map(|l| h.query_all_by_label(l).map(|n| n.rect().center()))
        .collect();
    found.sort_by(|a, b| a.y.total_cmp(&b.y));
    found[row]
}

/// Clicks row `row`'s toggle (see [`toggle_pos`]).
fn click_toggle(h: &mut Harness<'_, Both>, labels: [&str; 2], row: usize) {
    let pos = toggle_pos(h, labels, row);
    click(h, pos);
}

/// Presses and releases `key`.
fn press(h: &mut Harness<'_, Both>, key: egui::Key) {
    h.key_press(key);
    h.run();
}

/// Clicks the button labelled `label`.
fn button(h: &mut Harness<'_, Both>, label: &str) {
    h.get_by_label(label).click();
    h.run();
}

/// Undoes one step.
fn undo(h: &Harness<'_, Both>) {
    h.state().designer.borrow_mut().undo();
}

#[test]
fn test_rows_list_shapes_in_drawing_order_with_names() {
    let h = both(three());
    let shapes = order(&h);
    assert_eq!(shapes.len(), 3);
    let ys: Vec<f32> = shapes.iter().map(|id| row_pos(&h, *id).y).collect();
    assert!(
        ys[0] < ys[1] && ys[1] < ys[2],
        "rows run first to last: {ys:?}"
    );
    assert!(h.query_by_label("Objects").is_some());
}

#[test]
fn test_row_click_selects_the_shape_on_the_canvas() {
    let mut h = both(three());
    let shapes = order(&h);
    click_row(&mut h, shapes[1], Modifiers::NONE);
    assert_eq!(sel(&h), vec![shapes[1]]);
    click_row(&mut h, shapes[2], Modifiers::NONE);
    assert_eq!(sel(&h), vec![shapes[2]]);
}

#[test]
fn test_ctrl_click_toggles_and_shift_click_adds_a_range() {
    let mut h = both(three());
    let s = order(&h);
    click_row(&mut h, s[0], Modifiers::NONE);
    click_row(&mut h, s[2], Modifiers::COMMAND);
    assert_eq!(sel(&h), vec![s[0], s[2]]);
    click_row(&mut h, s[0], Modifiers::COMMAND);
    assert_eq!(sel(&h), vec![s[2]]);
    click_row(&mut h, s[0], Modifiers::NONE);
    click_row(&mut h, s[2], Modifiers::SHIFT);
    assert_eq!(sel(&h), s);
}

/// The number of rows the Layers pane paints with the selection fill.
fn highlighted_rows(designer: &Rc<RefCell<DesignerPane>>) -> usize {
    let ctx = egui::Context::default();
    let tr = Translator::new("en");
    let mut layers = LayersPane::new(Rc::clone(designer));
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let mut requested = Vec::new();
        let mut ctx = PaneContext::new(&tr, &mut requested);
        layers.ui(ui, &mut ctx);
    });
    output.textures_delta.clear();
    let fill = ctx.global_style().visuals.selection.bg_fill;
    output
        .shapes
        .iter()
        .filter(|c| matches!(&c.shape, egui::Shape::Rect(r) if r.fill == fill))
        .count()
}

#[test]
fn test_canvas_selection_shows_in_the_rows() {
    let designer = three();
    let mut h = both(Rc::clone(&designer));
    let s = order(&h);
    assert_eq!(highlighted_rows(&designer), 0);
    let pos = canvas_pos(&h, 1);
    click(&mut h, pos);
    assert_eq!(sel(&h), vec![s[1]]);
    assert_eq!(highlighted_rows(&designer), 1);
    let (a, b) = (canvas_pos(&h, 0), canvas_pos(&h, 2));
    click(&mut h, a);
    click_with(&mut h, b, PointerButton::Primary, Modifiers::SHIFT);
    assert_eq!(highlighted_rows(&designer), 2);
}

/// The number of painted outline paths in the design's shape colour, for a design where the
/// grid is off.
fn painted_outlines(designer: &DesignerPane) -> usize {
    let ctx = egui::Context::default();
    let palette = theme::palette(egui::Theme::Dark);
    let mut count = 0;
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let view = CanvasView::new(ui.max_rect(), designer.state().canvas.viewport());
        rendering::paint(
            ui.painter(),
            &view,
            designer.state(),
            designer.flags(),
            &CanvasInput::default(),
            palette,
            LineWidths::default(),
            &[],
            designer.work_area(),
        );
    });
    output.textures_delta.clear();
    for clipped in &output.shapes {
        if let egui::Shape::Path(path) = &clipped.shape
            && let egui::epaint::PathStroke {
                color: egui::epaint::ColorMode::Solid(c),
                ..
            } = path.stroke
            && c == palette.window_fg
        {
            count += 1;
        }
    }
    count
}

#[test]
fn test_layer_visibility_hides_shape() {
    let designer = three();
    designer.borrow_mut().state_mut().show_grid = false;
    let mut h = both(designer);
    let s = order(&h);
    assert_eq!(painted_outlines(&h.state().designer.borrow()), 3);
    // The "V" toggles of the rows, in row order: hide the middle shape.
    click_toggle(&mut h, ["Hide", "Show"], 1);
    let d = h.state().designer.borrow();
    assert!(d.flags().is_hidden(s[1]));
    // Not drawn.
    assert_eq!(painted_outlines(&d), 2);
    // Not hit-testable, by the canvas helper or by a click.
    let at = Point::new(60.0, 20.0);
    assert_eq!(topmost_shape_at(d.state(), d.flags(), &at, 1.0), None);
    drop(d);
    let pos = canvas_pos(&h, 1);
    click(&mut h, pos);
    assert!(
        sel(&h).is_empty(),
        "a click on the hidden shape selects nothing"
    );
    // Nor by the marquee.
    let (from, to) = (
        canvas_pos(&h, 1) - egui::vec2(15.0, 15.0),
        canvas_pos(&h, 1) + egui::vec2(15.0, 15.0),
    );
    crate::test_designer_canvas::drag(&mut h, from, to);
    assert!(sel(&h).is_empty(), "the marquee skips the hidden shape");
    // Showing it again restores both.
    click_toggle(&mut h, ["Hide", "Show"], 1);
    assert_eq!(painted_outlines(&h.state().designer.borrow()), 3);
    let pos = canvas_pos(&h, 1);
    click(&mut h, pos);
    assert_eq!(sel(&h), vec![s[1]]);
}

#[test]
fn test_hiding_deselects_and_a_hidden_row_cannot_be_selected() {
    let mut h = both(three());
    let s = order(&h);
    h.state().designer.borrow_mut().state_mut().is_modified = false;
    click_row(&mut h, s[0], Modifiers::NONE);
    click_toggle(&mut h, ["Hide", "Show"], 0);
    assert!(sel(&h).is_empty());
    click_row(&mut h, s[0], Modifiers::NONE);
    assert!(sel(&h).is_empty());
    // Visibility is view state: it neither marks the design modified nor adds an undo step.
    assert!(!h.state().designer.borrow().is_modified());
}

#[test]
fn test_locked_shape_is_drawn_but_not_pickable() {
    let designer = three();
    designer.borrow_mut().state_mut().show_grid = false;
    let mut h = both(designer);
    let s = order(&h);
    click_toggle(&mut h, ["Lock", "Unlock"], 2);
    {
        let d = h.state().designer.borrow();
        assert!(d.flags().is_locked(s[2]));
        assert_eq!(painted_outlines(&d), 3, "still drawn");
    }
    let pos = canvas_pos(&h, 2);
    click(&mut h, pos);
    assert!(sel(&h).is_empty());
    click_row(&mut h, s[2], Modifiers::NONE);
    assert!(sel(&h).is_empty());
    // An unlocked neighbour still selects, and unlocking restores the lock's shape.
    click_row(&mut h, s[1], Modifiers::NONE);
    assert_eq!(sel(&h), vec![s[1]]);
    click_toggle(&mut h, ["Lock", "Unlock"], 2);
    click_row(&mut h, s[2], Modifiers::NONE);
    assert_eq!(sel(&h), vec![s[2]]);
}

#[test]
fn test_rename_is_one_undo_step_and_marks_modified() {
    let mut h = both(three());
    let s = order(&h);
    h.state().designer.borrow_mut().state_mut().is_modified = false;
    let old = h
        .state()
        .designer
        .borrow()
        .state()
        .canvas
        .get_shape(s[1])
        .map(|o| o.name.clone());
    let fields: Vec<_> = h
        .get_all_by_role(egui::accesskit::Role::TextInput)
        .map(|n| n.rect().center())
        .collect();
    // Per row: the position field, then the name field.
    click(&mut h, fields[3]);
    h.key_combination_modifiers(Modifiers::COMMAND, &[egui::Key::A]);
    h.run();
    h.event(egui::Event::Text("Lid".to_string()));
    h.run();
    press(&mut h, egui::Key::Enter);
    let d = h.state().designer.borrow();
    assert_eq!(
        d.state().canvas.get_shape(s[1]).map(|o| o.name.as_str()),
        Some("Lid")
    );
    assert!(d.is_modified());
    drop(d);
    undo(&h);
    let after = h
        .state()
        .designer
        .borrow()
        .state()
        .canvas
        .get_shape(s[1])
        .map(|o| o.name.clone());
    assert_eq!(after, old);
}

#[test]
fn test_delete_key_does_not_fire_while_renaming() {
    let mut h = both(three());
    let s = order(&h);
    click_row(&mut h, s[1], Modifiers::NONE);
    let fields: Vec<_> = h
        .get_all_by_role(egui::accesskit::Role::TextInput)
        .map(|n| n.rect().center())
        .collect();
    click(&mut h, fields[3]);
    press(&mut h, egui::Key::Delete);
    press(&mut h, egui::Key::Backspace);
    assert_eq!(order(&h), s, "no shape was deleted");
}

#[test]
fn test_delete_key_over_the_panel_deletes_the_selection_in_one_undo_step() {
    let mut h = both(three());
    let s = order(&h);
    click_row(&mut h, s[0], Modifiers::NONE);
    click_row(&mut h, s[2], Modifiers::COMMAND);
    h.event(egui::Event::PointerMoved(row_pos(&h, s[1])));
    h.run();
    press(&mut h, egui::Key::Delete);
    assert_eq!(order(&h), vec![s[1]]);
    assert!(h.state().designer.borrow().is_modified());
    undo(&h);
    assert_eq!(order(&h).len(), 3, "one undo restores both");
}

#[test]
fn test_move_buttons_reorder_one_undo_step_each() {
    let mut h = both(three());
    let s = order(&h);
    click_row(&mut h, s[0], Modifiers::NONE);
    button(&mut h, "Move down");
    assert_eq!(order(&h), vec![s[1], s[0], s[2]]);
    button(&mut h, "Move to end");
    assert_eq!(order(&h), vec![s[1], s[2], s[0]]);
    button(&mut h, "Move to start");
    assert_eq!(order(&h), vec![s[0], s[1], s[2]]);
    button(&mut h, "Move up");
    // Already first: the buttons toward the start are disabled, so the order is unchanged.
    assert_eq!(order(&h), vec![s[0], s[1], s[2]]);
    undo(&h);
    assert_eq!(order(&h), vec![s[1], s[2], s[0]]);
    undo(&h);
    assert_eq!(order(&h), vec![s[1], s[0], s[2]]);
    undo(&h);
    assert_eq!(order(&h), s);
}

#[test]
fn test_position_field_moves_the_shape_on_enter() {
    let mut h = both(three());
    let s = order(&h);
    let fields: Vec<_> = h
        .get_all_by_role(egui::accesskit::Role::TextInput)
        .map(|n| n.rect().center())
        .collect();
    // The first row's position field: type 3, Enter.
    click(&mut h, fields[0]);
    h.key_combination_modifiers(Modifiers::COMMAND, &[egui::Key::A]);
    h.event(egui::Event::Text("3".to_string()));
    h.run();
    press(&mut h, egui::Key::Enter);
    assert_eq!(order(&h), vec![s[1], s[2], s[0]]);
    undo(&h);
    assert_eq!(order(&h), s);
}

#[test]
fn test_position_field_rejects_out_of_range_and_text() {
    let mut h = both(three());
    let s = order(&h);
    for typed in ["9", "0", "abc"] {
        let fields: Vec<_> = h
            .get_all_by_role(egui::accesskit::Role::TextInput)
            .map(|n| n.rect().center())
            .collect();
        click(&mut h, fields[0]);
        h.key_combination_modifiers(Modifiers::COMMAND, &[egui::Key::A]);
        h.event(egui::Event::Text(typed.to_string()));
        h.run();
        press(&mut h, egui::Key::Enter);
        assert_eq!(order(&h), s, "{typed}");
    }
}

#[test]
fn test_group_and_ungroup_are_one_undo_step_each() {
    let mut h = both(three());
    let s = order(&h);
    click_row(&mut h, s[0], Modifiers::NONE);
    click_row(&mut h, s[1], Modifiers::COMMAND);
    button(&mut h, "Group");
    let group_of = |h: &Harness<'_, Both>, id: u64| {
        h.state()
            .designer
            .borrow()
            .state()
            .canvas
            .get_shape(id)
            .and_then(|o| o.group_id)
    };
    let g = group_of(&h, s[0]);
    assert!(g.is_some());
    assert_eq!(g, group_of(&h, s[1]));
    assert_eq!(group_of(&h, s[2]), None);
    assert!(h.state().designer.borrow().is_modified());
    button(&mut h, "Ungroup");
    assert_eq!(group_of(&h, s[0]), None);
    undo(&h);
    assert_eq!(group_of(&h, s[0]), g, "undo restores the group");
    undo(&h);
    assert_eq!(group_of(&h, s[0]), None, "undo removes it");
}

#[test]
fn test_group_badge_selects_the_whole_group() {
    let mut h = both(three());
    let s = order(&h);
    click_row(&mut h, s[0], Modifiers::NONE);
    click_row(&mut h, s[2], Modifiers::COMMAND);
    button(&mut h, "Group");
    click_row(&mut h, s[1], Modifiers::NONE);
    assert_eq!(sel(&h), vec![s[1]]);
    let g = h
        .state()
        .designer
        .borrow()
        .state()
        .canvas
        .get_shape(s[0])
        .and_then(|o| o.group_id)
        .expect("group");
    let pos = h
        .get_all_by_label(&format!("G:{g}"))
        .next()
        .expect("badge")
        .rect()
        .center();
    click(&mut h, pos);
    assert_eq!(sel(&h), vec![s[0], s[2]]);
}

#[test]
fn test_move_that_would_split_a_group_is_refused_with_a_notice() {
    let mut h = both(three());
    let s = order(&h);
    click_row(&mut h, s[0], Modifiers::NONE);
    click_row(&mut h, s[1], Modifiers::COMMAND);
    button(&mut h, "Group");
    // Moving the lone third shape to position 1 puts it before the group: fine. Moving the
    // first group member to position 3 would split nothing either; moving one member to the
    // middle of another group's span is what splits, so group the last two and try again.
    click_row(&mut h, s[2], Modifiers::NONE);
    let fields: Vec<_> = h
        .get_all_by_role(egui::accesskit::Role::TextInput)
        .map(|n| n.rect().center())
        .collect();
    // Row 3's position field is index 4 (position, name per row): put s[2] at position 2,
    // between the two members of the group.
    click(&mut h, fields[4]);
    h.key_combination_modifiers(Modifiers::COMMAND, &[egui::Key::A]);
    h.event(egui::Event::Text("2".to_string()));
    h.run();
    press(&mut h, egui::Key::Enter);
    assert_eq!(order(&h), s, "refused");
    assert!(h.query_by_label("It cannot be moved").is_some());
    button(&mut h, "Close");
    assert!(h.query_by_label("It cannot be moved").is_none());
}

#[test]
fn test_layer_row_snapshot() {
    let designer = three();
    {
        let mut d = designer.borrow_mut();
        let first = ids(&d)[0];
        d.set_hidden(first, true);
    }
    let tr = Translator::new("en");
    let layers = LayersPane::new(Rc::clone(&designer));
    let mut h = Harness::builder()
        .with_size(egui::vec2(360.0, 200.0))
        .with_step_dt(STEP_DT)
        .with_theme(egui::Theme::Dark)
        .wgpu()
        .build_ui_state(
            move |ui, layers: &mut LayersPane| {
                theme::install(ui.ctx());
                let mut requested = Vec::new();
                let mut ctx = PaneContext::new(&tr, &mut requested);
                layers.ui(ui, &mut ctx);
            },
            layers,
        );
    h.run();
    let options = SnapshotOptions::new()
        .threshold(1.0)
        .max_failed_pixels(40)
        .output_path(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots"));
    if let Err(error) = h.try_snapshot_options("designer_layers", &options) {
        panic!("designer_layers: {error}");
    }
}

/// Clicks the `which`th text field of the panel (per row: position, then name), types
/// `text` over it and presses Enter.
fn type_into(h: &mut Harness<'_, Both>, which: usize, text: &str) {
    let fields: Vec<_> = h
        .get_all_by_role(egui::accesskit::Role::TextInput)
        .map(|n| n.rect().center())
        .collect();
    click(h, fields[which]);
    h.key_combination_modifiers(Modifiers::COMMAND, &[egui::Key::A]);
    h.event(egui::Event::Text(text.to_string()));
    h.run();
    press(h, egui::Key::Enter);
}

#[test]
fn test_undo_does_not_reselect_a_hidden_or_locked_shape() {
    for lock in [false, true] {
        let mut h = both(three());
        let s = order(&h);
        click_row(&mut h, s[1], Modifiers::NONE);
        type_into(&mut h, 3, "Lid");
        click_row(&mut h, s[1], Modifiers::NONE);
        if lock {
            click_toggle(&mut h, ["Lock", "Unlock"], 1);
        } else {
            click_toggle(&mut h, ["Hide", "Show"], 1);
        }
        assert!(sel(&h).is_empty());
        undo(&h);
        assert!(sel(&h).is_empty(), "lock {lock}: undo reselected it");
        h.state().designer.borrow_mut().redo();
        assert!(sel(&h).is_empty(), "lock {lock}: redo reselected it");
        // Delete removes nothing.
        h.event(egui::Event::PointerMoved(row_pos(&h, s[0])));
        h.run();
        press(&mut h, egui::Key::Delete);
        assert_eq!(order(&h), s, "lock {lock}");
    }
}

#[test]
fn test_gcode_export_skips_hidden_shapes() {
    let designer = three();
    let mut h = both(Rc::clone(&designer));
    let s = order(&h);
    let all = designer.borrow().generate_gcode(None);
    // The squares span x 10..30, 50..70 and 90..110; the middle one has x 70 only.
    assert!(all.contains("X70"), "{all}");
    click_toggle(&mut h, ["Hide", "Show"], 1);
    assert!(designer.borrow().flags().is_hidden(s[1]));
    let code = designer.borrow().generate_gcode(None);
    assert!(!code.contains("X70"), "hidden shape was cut:\n{code}");
    assert!(code.contains("X30") && code.contains("X110"), "{code}");
    // The design itself is untouched.
    assert_eq!(order(&h), s);
}

#[test]
fn test_panel_delete_waits_for_a_canvas_drag() {
    let mut h = both(three());
    let s = order(&h);
    let start = canvas_pos(&h, 1);
    click(&mut h, start);
    // Press on the selected shape and start moving it, without releasing.
    crate::test_designer_canvas::button(&h, start, PointerButton::Primary, true, Modifiers::NONE);
    h.step();
    let into_panel = row_pos(&h, s[0]);
    for t in [0.25, 0.5, 0.75, 1.0] {
        h.event(egui::Event::PointerMoved(start.lerp(into_panel, t)));
        h.step();
    }
    assert!(h.state().designer.borrow().dragging());
    // The canvas holds the keyboard during a drag; take it away so only the panel could act.
    h.ctx.memory_mut(|m| {
        if let Some(id) = m.focused() {
            m.surrender_focus(id);
        }
    });
    press(&mut h, egui::Key::Delete);
    assert_eq!(order(&h), s, "no delete mid-drag");
}

#[test]
fn test_locked_shape_cannot_be_renamed_or_repositioned_from_the_panel() {
    let mut h = both(three());
    let s = order(&h);
    click_toggle(&mut h, ["Lock", "Unlock"], 0);
    let name = |h: &Harness<'_, Both>| {
        h.state()
            .designer
            .borrow()
            .state()
            .canvas
            .get_shape(s[0])
            .map(|o| o.name.clone())
    };
    let before = name(&h);
    type_into(&mut h, 1, "Nope");
    assert_eq!(name(&h), before);
    type_into(&mut h, 0, "3");
    assert_eq!(order(&h), s);
}

#[test]
fn test_group_with_a_locked_member_cannot_be_picked_from_the_canvas() {
    let mut h = both(three());
    let s = order(&h);
    click_row(&mut h, s[0], Modifiers::NONE);
    click_row(&mut h, s[1], Modifiers::COMMAND);
    button(&mut h, "Group");
    click_toggle(&mut h, ["Lock", "Unlock"], 0);
    for index in [0, 1] {
        let pos = canvas_pos(&h, index);
        click(&mut h, pos);
        assert!(sel(&h).is_empty(), "member {index} picked part of a group");
    }
    // The marquee skips the whole group too.
    let (a, b) = (
        canvas_pos(&h, 0) - egui::vec2(15.0, 15.0),
        canvas_pos(&h, 1) + egui::vec2(15.0, 15.0),
    );
    crate::test_designer_canvas::drag(&mut h, a, b);
    assert!(sel(&h).is_empty());
}
