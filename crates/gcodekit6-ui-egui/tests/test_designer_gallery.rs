//! Designer shape gallery: the toolbox button opens it, a card arms its shape, a click on the
//! canvas places it (one undo step), Escape and Close drop it, and every template makes its
//! shape.

use egui_kittest::kittest::Queryable;
use gcodekit6_designer::model::{DesignerShape, Shape};
use gcodekit6_ui_egui::panes::designer::DesignerPane;
use gcodekit6_ui_egui::panes::designer::shape_gallery::TEMPLATES;

use crate::test_designer_canvas::{click, harness, ids, press, screen};

/// The pane with the gallery opened from the toolbox and "Spur Gear" chosen.
fn armed_gear() -> egui_kittest::Harness<'static, DesignerPane> {
    let mut h = harness(DesignerPane::new());
    h.get_by_label("Shape Gallery…").click();
    h.run();
    assert!(h.state().gallery_open(), "the button opens the gallery");
    assert!(h.query_by_label("Fast Shape Gallery").is_some());
    h.get_by_label("Spur Gear").click();
    h.run();
    assert!(
        !h.state().gallery_open(),
        "choosing a card closes the gallery"
    );
    assert!(h.state().pending_shape().is_some(), "the gear is armed");
    h
}

#[test]
fn test_every_template_makes_its_shape_centred_on_the_point() {
    assert_eq!(TEMPLATES.len(), 6);
    for template in TEMPLATES {
        let shape = (template.make)(gcodekit6_designer::model::Point::new(30.0, -20.0));
        let (x1, y1, x2, y2) = shape.bounds();
        assert!(x2 > x1 && y2 > y1, "{}", template.name);
        let centre = ((x1 + x2) / 2.0, (y1 + y2) / 2.0);
        // Every generator centres on the point it is given (the L-bracket's holes and
        // outline are not symmetric, so allow a few mm).
        assert!(
            (centre.0 - 30.0).abs() < 25.0 && (centre.1 + 20.0).abs() < 25.0,
            "{}: {centre:?}",
            template.name
        );
    }
    assert!(matches!(
        (TEMPLATES[3].make)(gcodekit6_designer::model::Point::new(0.0, 0.0)),
        Shape::Rectangle(r) if r.is_slot
    ));
}

#[test]
fn test_gallery_shape_is_placed_where_the_user_clicks_as_one_undo_step() {
    let mut h = armed_gear();
    assert!(ids(h.state()).is_empty(), "nothing is added before placing");
    assert!(h.query_by_label("Click to position shape").is_some());
    let p = screen(&h, 100.0, 120.0);
    click(&mut h, p);
    assert_eq!(ids(h.state()).len(), 1);
    assert!(h.state().pending_shape().is_none());
    assert!(h.state().is_modified());
    let obj = h
        .state()
        .state()
        .canvas
        .shapes()
        .next()
        .cloned()
        .expect("placed shape");
    assert!(matches!(obj.shape, Shape::Gear(_)));
    let (x1, y1, x2, y2) = obj.shape.bounds();
    assert!(((x1 + x2) / 2.0 - 100.0).abs() < 0.5, "x {x1} {x2}");
    assert!(((y1 + y2) / 2.0 - 120.0).abs() < 0.5, "y {y1} {y2}");
    // A second click places nothing more.
    click(&mut h, p);
    assert_eq!(ids(h.state()).len(), 1);
    h.state_mut().undo();
    h.run();
    assert!(
        ids(h.state()).is_empty(),
        "one undo removes the placed shape"
    );
}

#[test]
fn test_escape_drops_the_armed_shape() {
    let mut h = armed_gear();
    // A move over the canvas gives the ghost a place; Escape drops it.
    let p = screen(&h, 100.0, 100.0);
    h.event(egui::Event::PointerMoved(p));
    h.run();
    press(&mut h, egui::Key::Escape);
    assert!(h.state().pending_shape().is_none());
    click(&mut h, p);
    assert!(ids(h.state()).is_empty());
}

#[test]
fn test_close_button_closes_the_gallery_without_arming() {
    let mut h = harness(DesignerPane::new());
    h.get_by_label("Shape Gallery…").click();
    h.run();
    h.get_by_label("Close").click();
    h.run();
    assert!(!h.state().gallery_open());
    assert!(h.state().pending_shape().is_none());
}
