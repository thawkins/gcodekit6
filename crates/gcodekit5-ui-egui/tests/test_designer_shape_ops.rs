//! Designer shape operations of the GTK canvas context menu and keys: Mirror X/Y, Align,
//! Cut/Copy/Paste, the boolean operations and Group/Ungroup. Each runs the
//! `gcodekit5-designer` operation GTK used, as one undo step.

use egui::{Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_ui_egui::panes::designer::DesignerPane;

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
