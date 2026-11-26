//! Designer shape operations of the GTK canvas context menu and keys: Mirror X/Y, Align,
//! Cut/Copy/Paste, the boolean operations and Group/Ungroup. Each runs the
//! `gcodekit5-designer` operation GTK used, as one undo step.

use egui::{Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_designer::model::{DesignRectangle, Shape};
use gcodekit5_ui_egui::panes::designer::DesignerPane;
use glam::Vec2;

use crate::test_designer_canvas::{
    bounds, click_with, close4, harness, ids, pane_with_rects, screen, selected,
};

/// Selects every shape and opens the context menu over world `(x, y)`.
fn open_menu_with_all_selected(h: &mut Harness<'_, DesignerPane>, x: f32, y: f32) {
    h.state_mut().state_mut().select_all();
    h.run();
    let p = screen(h, x, y);
    click_with(h, p, PointerButton::Secondary, Modifiers::NONE);
}

/// Opens the context menu over world `(x, y)` and clicks `item`.
fn menu_item(h: &mut Harness<'_, DesignerPane>, x: f32, y: f32, item: &str) {
    open_menu_with_all_selected(h, x, y);
    h.get_by_label(item).click();
    h.run();
}

#[test]
fn test_mirror_x_and_y_from_context_menu_are_undoable() {
    // Two squares side by side: mirroring the selection on X swaps them about its centre.
    let mut h = harness(pane_with_rects(&[
        (0.0, 0.0, 10.0, 10.0),
        (30.0, 0.0, 10.0, 20.0),
    ]));
    let [a, b] = [ids(h.state())[0], ids(h.state())[1]];
    menu_item(&mut h, 5.0, 5.0, "Mirror on X");
    assert!(close4(bounds(h.state(), a), (30.0, 0.0, 40.0, 10.0)));
    assert!(close4(bounds(h.state(), b), (0.0, 0.0, 10.0, 20.0)));
    assert_eq!(ids(h.state()), vec![a, b], "mirror keeps the shapes");
    h.state_mut().undo();
    h.run();
    assert!(close4(bounds(h.state(), a), (0.0, 0.0, 10.0, 10.0)));
    assert!(close4(bounds(h.state(), b), (30.0, 0.0, 40.0, 20.0)));

    // Mirror on Y flips about the selection's horizontal centre line (y = 10).
    menu_item(&mut h, 5.0, 5.0, "Mirror on Y");
    assert!(close4(bounds(h.state(), a), (0.0, 10.0, 10.0, 20.0)));
    assert!(close4(bounds(h.state(), b), (30.0, 0.0, 40.0, 20.0)));
    h.state_mut().undo();
    h.run();
    assert!(close4(bounds(h.state(), a), (0.0, 0.0, 10.0, 10.0)));
    assert_eq!(selected(h.state()).len(), 2);
}

/// A shape's bounds: min x, min y, max x, max y (mm).
type Bounds = (f64, f64, f64, f64);

/// The Align submenu's entries and the bounds each leaves the squares (0,0)-(10,10) and
/// (30,5)-(40,25) at.
const ALIGNMENTS: [(&str, Bounds, Bounds); 6] = [
    ("Align Left", (0.0, 0.0, 10.0, 10.0), (0.0, 5.0, 10.0, 25.0)),
    (
        "Align Right",
        (30.0, 0.0, 40.0, 10.0),
        (30.0, 5.0, 40.0, 25.0),
    ),
    (
        "Align Top",
        (0.0, 15.0, 10.0, 25.0),
        (30.0, 5.0, 40.0, 25.0),
    ),
    (
        "Align Bottom",
        (0.0, 0.0, 10.0, 10.0),
        (30.0, 0.0, 40.0, 20.0),
    ),
    (
        "Align Center X",
        (15.0, 0.0, 25.0, 10.0),
        (15.0, 5.0, 25.0, 25.0),
    ),
    (
        "Align Center Y",
        (0.0, 7.5, 10.0, 17.5),
        (30.0, 2.5, 40.0, 22.5),
    ),
];

#[test]
fn test_align_submenu_aligns_the_selection_in_one_undo_step() {
    for (item, want_a, want_b) in ALIGNMENTS {
        let mut h = harness(pane_with_rects(&[
            (0.0, 0.0, 10.0, 10.0),
            (30.0, 5.0, 10.0, 20.0),
        ]));
        let [a, b] = [ids(h.state())[0], ids(h.state())[1]];
        open_menu_with_all_selected(&mut h, 5.0, 5.0);
        h.get_by_label("Align").click();
        h.run();
        h.get_by_label(item).click();
        h.run();
        assert!(close4(bounds(h.state(), a), want_a), "{item}: a");
        assert!(close4(bounds(h.state(), b), want_b), "{item}: b");
        h.state_mut().undo();
        h.run();
        assert!(
            close4(bounds(h.state(), a), (0.0, 0.0, 10.0, 10.0)),
            "{item}"
        );
        assert!(
            close4(bounds(h.state(), b), (30.0, 5.0, 40.0, 25.0)),
            "{item}"
        );
    }
}

#[test]
fn test_align_needs_two_shapes() {
    let mut h = harness(pane_with_rects(&[(0.0, 0.0, 10.0, 10.0)]));
    let p = screen(&h, 5.0, 5.0);
    crate::test_designer_canvas::click(&mut h, p);
    click_with(&mut h, p, PointerButton::Secondary, Modifiers::NONE);
    assert!(
        h.query_by_label("Mirror on X").is_some(),
        "the menu is open"
    );
    assert!(
        h.query_by_label("Align").is_none(),
        "GTK shows Align for 2+"
    );
}

/// The app's designer holding the two squares, both selected after a click on the canvas
/// (which gives the canvas the keyboard).
fn app_with_two_selected() -> (
    Harness<'static, gcodekit5_ui_egui::GcodeKitApp>,
    std::rc::Rc<std::cell::RefCell<DesignerPane>>,
) {
    let mut h = crate::test_designer_app::app_with_designer();
    let designer = std::rc::Rc::clone(h.state().designer());
    for (x, y, w, hh) in [(0.0, 0.0, 10.0, 10.0), (30.0, 5.0, 10.0, 20.0)] {
        designer
            .borrow_mut()
            .state_mut()
            .add_shape_with_undo(Shape::Rectangle(DesignRectangle::new(x, y, w, hh)));
    }
    h.run();
    let p = designer
        .borrow()
        .view()
        .world_to_screen(Vec2::new(5.0, 5.0));
    crate::test_designer_app::app_click(&mut h, p);
    designer.borrow_mut().state_mut().select_all();
    h.run();
    (h, designer)
}

/// The bounds of shape `id` in `designer`.
fn bounds_in(designer: &std::rc::Rc<std::cell::RefCell<DesignerPane>>, id: u64) -> Bounds {
    bounds(&designer.borrow(), id)
}

#[test]
fn test_align_keys_run_on_the_canvas_but_not_while_typing() {
    let (mut h, designer) = app_with_two_selected();
    let [a, b] = [ids(&designer.borrow())[0], ids(&designer.borrow())[1]];
    // GTK's Alt+L/R/T/B/H/V while the canvas has the keyboard.
    h.key_press_modifiers(Modifiers::ALT, egui::Key::R);
    h.run();
    assert!(close4(bounds_in(&designer, a), (30.0, 0.0, 40.0, 10.0)));
    h.key_press_modifiers(Modifiers::ALT, egui::Key::T);
    h.run();
    assert!(close4(bounds_in(&designer, a), (30.0, 15.0, 40.0, 25.0)));
    assert!(close4(bounds_in(&designer, b), (30.0, 5.0, 40.0, 25.0)));
    // A text field (the inspector's X) owns the keyboard: Alt+L aligns nothing.
    let field = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "X:");
    field.focus();
    h.run();
    h.key_press_modifiers(Modifiers::ALT, egui::Key::L);
    h.run();
    assert!(close4(bounds_in(&designer, a), (30.0, 15.0, 40.0, 25.0)));
}

/// Right-clicks world `(x, y)` (keeping the selection) and clicks `item` in the menu.
fn right_click_item(h: &mut Harness<'_, DesignerPane>, x: f32, y: f32, item: &str) {
    let p = screen(h, x, y);
    click_with(h, p, PointerButton::Secondary, Modifiers::NONE);
    h.get_by_label(item).click();
    h.run();
}

#[test]
fn test_copy_and_paste_from_context_menu_make_new_shapes_at_the_pointer() {
    let mut h = harness(pane_with_rects(&[(0.0, 0.0, 10.0, 10.0)]));
    let original = ids(h.state())[0];
    let p = screen(&h, 5.0, 5.0);
    crate::test_designer_canvas::click(&mut h, p);
    right_click_item(&mut h, 5.0, 5.0, "Copy");
    assert_eq!(ids(h.state()), vec![original], "copy changes nothing");
    // GTK pastes the clipboard centred on the pointer, selected, as one undo step.
    h.state_mut().state_mut().deselect_all();
    h.run();
    right_click_item(&mut h, 50.0, 40.0, "Paste");
    let after = ids(h.state());
    assert_eq!(after.len(), 2);
    let pasted = after[1];
    assert_ne!(pasted, original);
    assert!(close4(bounds(h.state(), pasted), (45.0, 35.0, 55.0, 45.0)));
    assert!(close4(bounds(h.state(), original), (0.0, 0.0, 10.0, 10.0)));
    assert_eq!(selected(h.state()), vec![pasted]);
    // A second paste is another new shape with its own id.
    right_click_item(&mut h, 80.0, 40.0, "Paste");
    let third = ids(h.state());
    assert_eq!(third.len(), 3);
    assert!(!after.contains(&third[2]));
    h.state_mut().undo();
    h.run();
    h.state_mut().undo();
    h.run();
    assert_eq!(ids(h.state()), vec![original]);
    assert!(close4(bounds(h.state(), original), (0.0, 0.0, 10.0, 10.0)));
}

#[test]
fn test_cut_removes_the_selection_and_paste_brings_it_back() {
    let mut h = harness(pane_with_rects(&[
        (0.0, 0.0, 10.0, 10.0),
        (30.0, 0.0, 10.0, 10.0),
    ]));
    let [a, b] = [ids(h.state())[0], ids(h.state())[1]];
    let p = screen(&h, 5.0, 5.0);
    crate::test_designer_canvas::click(&mut h, p);
    right_click_item(&mut h, 5.0, 5.0, "Cut");
    assert_eq!(ids(h.state()), vec![b]);
    // With nothing selected the menu still offers Paste while the clipboard holds shapes.
    right_click_item(&mut h, 20.0, 20.0, "Paste");
    let now = ids(h.state());
    assert_eq!(now.len(), 2);
    assert!(!now.contains(&a), "the pasted shape has a new id");
    assert!(close4(bounds(h.state(), now[1]), (15.0, 15.0, 25.0, 25.0)));
    // Undo the paste, then the cut.
    h.state_mut().undo();
    h.run();
    h.state_mut().undo();
    h.run();
    assert_eq!(ids(h.state()).len(), 2);
    assert!(ids(h.state()).contains(&a));
}

#[test]
fn test_paste_gives_a_copied_group_its_own_group_id() {
    let mut pane = pane_with_rects(&[(0.0, 0.0, 10.0, 10.0), (20.0, 0.0, 10.0, 10.0)]);
    pane.state_mut().select_all();
    pane.state_mut().group_selected();
    let mut h = harness(pane);
    let group = |h: &Harness<'_, DesignerPane>, id: u64| {
        h.state()
            .state()
            .canvas
            .get_shape(id)
            .and_then(|o| o.group_id)
    };
    let original = group(&h, ids(h.state())[0]);
    assert!(original.is_some());
    let p = screen(&h, 5.0, 5.0);
    crate::test_designer_canvas::click(&mut h, p);
    right_click_item(&mut h, 5.0, 5.0, "Copy");
    right_click_item(&mut h, 60.0, 60.0, "Paste");
    let all = ids(h.state());
    assert_eq!(all.len(), 4);
    let pasted = [group(&h, all[2]), group(&h, all[3])];
    assert_eq!(pasted[0], pasted[1], "the copies stay one group");
    assert!(pasted[0].is_some());
    assert_ne!(pasted[0], original, "a group of their own");
    assert_eq!(group(&h, all[0]), original, "the originals keep theirs");
}

#[test]
fn test_clipboard_keys_run_on_the_canvas_but_not_while_typing() {
    let (mut h, designer) = app_with_two_selected();
    let before = ids(&designer.borrow());
    // The pointer is over (5, 5) after the click: Cmd+C then Cmd+V pastes there.
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::C);
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::V);
    h.run();
    let after = ids(&designer.borrow());
    assert_eq!(after.len(), 4, "Cmd+V pasted both shapes");
    // Cmd+X cuts the pasted (selected) copies.
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::X);
    h.run();
    assert_eq!(ids(&designer.borrow()), before);
    // A text field (the inspector's X, shown for a selection) owns the keyboard: Cmd+V goes
    // to it, not to the canvas.
    designer.borrow_mut().state_mut().select_all();
    h.run();
    let field = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "X:");
    field.focus();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::V);
    h.run();
    assert_eq!(ids(&designer.borrow()), before);
}

/// The boolean entries of the context menu and the bounds each leaves of the squares
/// (0,0)-(20,20) and (10,0)-(30,20).
const BOOLEANS: [(&str, Bounds); 3] = [
    ("Union", (0.0, 0.0, 30.0, 20.0)),
    ("Diff", (0.0, 0.0, 10.0, 20.0)),
    ("Inter", (10.0, 0.0, 20.0, 20.0)),
];

#[test]
fn test_boolean_ops_replace_the_selection_by_one_new_shape_in_one_undo_step() {
    for (item, want) in BOOLEANS {
        let mut h = harness(pane_with_rects(&[
            (0.0, 0.0, 20.0, 20.0),
            (10.0, 0.0, 20.0, 20.0),
        ]));
        let originals = ids(h.state());
        menu_item(&mut h, 5.0, 5.0, item);
        let now = ids(h.state());
        assert_eq!(now.len(), 1, "{item}");
        assert!(!originals.contains(&now[0]), "{item}: a new id");
        let b = bounds(h.state(), now[0]);
        let near = |a: f64, b: f64| (a - b).abs() < 0.05;
        assert!(
            near(b.0, want.0) && near(b.1, want.1) && near(b.2, want.2) && near(b.3, want.3),
            "{item}: {b:?}"
        );
        assert_eq!(selected(h.state()), now, "{item}: the result is selected");
        h.state_mut().undo();
        h.run();
        assert_eq!(ids(h.state()), originals, "{item}");
        assert!(close4(
            bounds(h.state(), originals[0]),
            (0.0, 0.0, 20.0, 20.0)
        ));
        assert!(close4(
            bounds(h.state(), originals[1]),
            (10.0, 0.0, 30.0, 20.0)
        ));
    }
}

#[test]
fn test_boolean_ops_need_two_shapes() {
    let mut h = harness(pane_with_rects(&[(0.0, 0.0, 10.0, 10.0)]));
    open_menu_with_all_selected(&mut h, 5.0, 5.0);
    assert!(
        h.query_by_label("Mirror on X").is_some(),
        "the menu is open"
    );
    for (item, _) in BOOLEANS {
        assert!(h.query_by_label(item).is_none(), "{item}");
    }
}

/// The group id of each shape of `designer`, in draw order.
fn groups(designer: &DesignerPane) -> Vec<Option<u64>> {
    ids(designer)
        .into_iter()
        .map(|id| {
            designer
                .state()
                .canvas
                .get_shape(id)
                .and_then(|o| o.group_id)
        })
        .collect()
}

#[test]
fn test_group_and_ungroup_keys_run_on_the_canvas_but_not_while_typing() {
    let (mut h, designer) = app_with_two_selected();
    // GTK's Ctrl+G groups the selection, Ctrl+Shift+G ungroups it, one undo step each.
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::G);
    h.run();
    let grouped = groups(&designer.borrow());
    assert!(
        grouped[0].is_some() && grouped[0] == grouped[1],
        "{grouped:?}"
    );
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, egui::Key::G);
    h.run();
    assert_eq!(groups(&designer.borrow()), vec![None, None]);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Z);
    h.run();
    assert_eq!(groups(&designer.borrow()), grouped, "undo regroups");
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Z);
    h.run();
    assert_eq!(
        groups(&designer.borrow()),
        vec![None, None],
        "undo ungroups"
    );
    // A text field owns the keyboard: Ctrl+G groups nothing.
    let field = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "X:");
    field.focus();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::G);
    h.run();
    assert_eq!(groups(&designer.borrow()), vec![None, None]);
}

#[test]
fn test_group_and_ungroup_from_context_menu() {
    let mut h = harness(pane_with_rects(&[
        (0.0, 0.0, 10.0, 10.0),
        (30.0, 0.0, 10.0, 10.0),
    ]));
    menu_item(&mut h, 5.0, 5.0, "Group");
    let grouped = groups(h.state());
    assert!(
        grouped[0].is_some() && grouped[0] == grouped[1],
        "{grouped:?}"
    );
    // A grouped selection offers Ungroup, not Group.
    open_menu_with_all_selected(&mut h, 5.0, 5.0);
    assert!(h.query_by_label("Group").is_none());
    h.get_by_label("Ungroup").click();
    h.run();
    assert_eq!(groups(h.state()), vec![None, None]);
}
