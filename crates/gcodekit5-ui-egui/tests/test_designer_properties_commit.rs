//! Designer properties panel: where a committed edit lands when the selection changed while
//! the field was being edited (the shapes it was typed for, never the new selection), the
//! notice when those shapes are gone or locked, locked shapes, invalid input with its
//! inline error, and the effect preview when the pane is not drawn.

use std::cell::RefCell;
use std::rc::Rc;

use egui::Pos2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_designer::model::{DesignText, DesignerShape, Shape};
use gcodekit5_ui_egui::panes::designer::DesignerPane;
use gcodekit5_ui_egui::panes::designer::layers::LayersPane;
use gcodekit5_ui_egui::workspace::Pane;

use crate::test_designer_canvas::{click, close, ids, selected};
use crate::test_designer_properties::{
    Both, both, design, field_text, set_field, two_rects, type_text, undo, undo_depth,
};

/// The notice shown when an edit's shapes are gone or locked.
const NOTICE: &str = "The edited shapes were deleted or locked; the change was not applied.";

/// The screen point of world `(x, y)` mm on the canvas.
fn canvas(h: &Harness<'_, Both>, x: f32, y: f32) -> Pos2 {
    h.state()
        .designer
        .borrow()
        .view()
        .world_to_screen(glam::Vec2::new(x, y))
}

/// The widths of every shape, in drawing order.
fn widths(designer: &Rc<RefCell<DesignerPane>>) -> Vec<f64> {
    let d = designer.borrow();
    ids(&d)
        .into_iter()
        .filter_map(|id| d.state().canvas.get_shape(id))
        .map(|o| {
            let (x0, _, x1, _) = o.shape.bounds();
            x1 - x0
        })
        .collect()
}

#[test]
fn test_typed_value_applies_to_original_shape_after_clicking_empty_canvas() {
    let designer = two_rects();
    designer.borrow_mut().state_mut().is_modified = false;
    let mut h = both(Rc::clone(&designer));
    let before = undo_depth(&h);
    type_text(&mut h, "Width:", "33");
    let empty = canvas(&h, 150.0, 150.0);
    click(&mut h, empty);
    assert!(
        selected(&designer.borrow()).is_empty(),
        "the click deselected"
    );
    let w = widths(&designer);
    assert!(close(w[0], 33.0) && close(w[1], 20.0), "{w:?}");
    assert_eq!(undo_depth(&h), before + 1, "one undo step");
    assert!(designer.borrow().is_modified());
    undo(&mut h);
    assert!(close(widths(&designer)[0], 20.0));
}

#[test]
fn test_typed_value_applies_to_original_not_newly_selected_shape() {
    let designer = two_rects();
    let mut h = both(Rc::clone(&designer));
    type_text(&mut h, "Width:", "44");
    let other = canvas(&h, 60.0, 15.0);
    click(&mut h, other);
    let all = ids(&designer.borrow());
    assert_eq!(
        selected(&designer.borrow()),
        vec![all[1]],
        "the other is selected now"
    );
    let w = widths(&designer);
    assert!(close(w[0], 44.0) && close(w[1], 20.0), "{w:?}");
    assert_eq!(
        field_text(&h, "Width:"),
        "20.00",
        "the panel shows the new selection"
    );
}

#[test]
fn test_typed_value_for_a_deleted_shape_applies_nothing_and_shows_a_notice() {
    let designer = two_rects();
    let mut h = both(Rc::clone(&designer));
    type_text(&mut h, "Width:", "55");
    designer.borrow_mut().state_mut().delete_selected();
    let empty = canvas(&h, 150.0, 150.0);
    click(&mut h, empty);
    let w = widths(&designer);
    assert_eq!(w.len(), 1);
    assert!(
        close(w[0], 20.0),
        "nothing applied to the other shape: {w:?}"
    );
    h.get_by_label(NOTICE);
}

#[test]
fn test_locked_shape_cannot_be_edited_through_the_panel() {
    // Locked before: its layers row cannot select it, so the panel has nothing to edit.
    let designer = two_rects();
    let all = ids(&designer.borrow());
    designer.borrow_mut().set_locked(all[0], true);
    let mut h = both(Rc::clone(&designer));
    let mut layers = LayersPane::new(Rc::clone(&designer));
    let tr = gcodekit5_app::i18n::Translator::new("en");
    let mut lh = egui_kittest::Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui(move |ui| {
            let mut requested = Vec::new();
            let mut ctx = gcodekit5_ui_egui::workspace::PaneContext::new(&tr, &mut requested);
            layers.ui(ui, &mut ctx);
        });
    lh.run();
    lh.get_by_label(&format!("Rect #{}", all[0])).click();
    lh.run();
    h.run();
    assert!(selected(&designer.borrow()).is_empty());
    h.get_by_label("Select a shape to edit its properties");
    // Locked while a value is being typed for it: the value is not applied.
    designer.borrow_mut().set_locked(all[0], false);
    designer
        .borrow_mut()
        .state_mut()
        .canvas
        .select_shape(all[0], false);
    h.run();
    type_text(&mut h, "Width:", "66");
    designer.borrow_mut().set_locked(all[0], true);
    h.run();
    assert!(close(widths(&designer)[0], 20.0));
    h.get_by_label(NOTICE);
}

#[test]
fn test_invalid_value_shows_an_inline_error_until_a_valid_commit() {
    let mut h = both(two_rects());
    set_field(&mut h, "Width:", "0");
    h.get_by_label("Enter a value greater than 0");
    set_field(&mut h, "Width:", "25");
    assert!(h.query_by_label("Enter a value greater than 0").is_none());
    set_field(&mut h, "X:", "abc");
    h.get_by_label("Enter a number");
}

#[test]
fn test_start_depth_cannot_be_negative() {
    let designer = two_rects();
    let mut h = both(Rc::clone(&designer));
    set_field(&mut h, "(-Z:)", "-1");
    h.get_by_label("Enter a value of 0 or more");
    let depth = designer
        .borrow()
        .state()
        .canvas
        .shapes()
        .find(|o| o.selected)
        .map(|o| o.start_depth);
    assert_eq!(depth, Some(0.0));
}

#[test]
fn test_blank_text_content_is_rejected() {
    let text = DesignText::new("Hi".to_string(), 50.0, 50.0, 10.0);
    let designer = design(vec![Shape::Text(text)], 1);
    let mut h = both(Rc::clone(&designer));
    set_field(&mut h, "Content:", "   ");
    h.get_by_label("Text cannot be empty");
    assert_eq!(field_text(&h, "Content:"), "Hi");
}

#[test]
fn test_text_size_keeps_its_aspect_as_the_font_scales() {
    let text = DesignText::new("Hi".to_string(), 50.0, 50.0, 10.0);
    let designer = design(vec![Shape::Text(text)], 1);
    // Even with the aspect unlocked, a text's size keeps its aspect (the font scales).
    for obj in designer.borrow_mut().state_mut().canvas.shapes_mut() {
        obj.lock_aspect_ratio = false;
    }
    let mut h = both(Rc::clone(&designer));
    let (w0, h0) = {
        let d = designer.borrow();
        let (x0, y0, x1, y1) = d
            .state()
            .canvas
            .shapes()
            .next()
            .expect("text")
            .shape
            .bounds();
        (x1 - x0, y1 - y0)
    };
    set_field(&mut h, "Width:", &format!("{:.2}", w0 * 2.0));
    let d = designer.borrow();
    let obj = d.state().canvas.shapes().next().expect("text");
    let (x0, y0, x1, y1) = obj.shape.bounds();
    assert!((x1 - x0 - w0 * 2.0).abs() < 0.05, "width {}", x1 - x0);
    assert!(
        (y1 - y0 - h0 * 2.0).abs() < 0.05,
        "height {} (was {h0})",
        y1 - y0
    );
    drop(d);
    assert_eq!(field_text(&h, "Height:"), format!("{:.2}", h0 * 2.0));
}

#[test]
fn test_effect_preview_is_cleared_when_the_pane_is_not_drawn() {
    let designer = two_rects();
    let mut h = both(Rc::clone(&designer));
    type_text(&mut h, "Offset:", "2");
    assert!(!designer.borrow().effect_preview().is_empty());
    // The workspace calls this for every pane it did not draw this frame.
    h.state_mut().props.cancel_hold();
    assert!(designer.borrow().effect_preview().is_empty());
}
