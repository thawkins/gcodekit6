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
