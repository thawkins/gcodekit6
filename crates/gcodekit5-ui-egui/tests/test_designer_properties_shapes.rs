//! Designer properties panel, the per-shape sections: corner radius and slot mode, a path's
//! fillet and closed flag, polygon sides, gear, sprocket and timing pulley parameters, text
//! content/font/size, and the offset and chamfer effects with their live preview. Which
//! sections a shape shows follows GTK.

use egui_kittest::kittest::{NodeT, Queryable};
use gcodekit5_designer::model::{
    DesignCircle, DesignGear, DesignPath, DesignPolygon, DesignRectangle, DesignSprocket,
    DesignText, ParametricPathSource, Point, Shape,
};
use gcodekit5_designer::parametric_shapes::generate_timing_pulley;
use gcodekit5_ui_egui::panes::designer::canvas::text_tool::MM_PER_PT;
use gcodekit5_ui_egui::panes::designer::properties::update::timing_pulley;

use crate::test_designer_canvas::close;
use crate::test_designer_properties::{
    both, cnc, design, field_enabled, field_text, first, set_field, toggle, type_text, undo,
    undo_depth,
};

/// The first selected object's copy.
fn object(
    h: &egui_kittest::Harness<'_, crate::test_designer_properties::Both>,
) -> gcodekit5_designer::canvas::DrawingObject {
    let d = h.state().designer.borrow();
    d.state()
        .canvas
        .shapes()
        .find(|o| o.selected)
        .cloned()
        .expect("a selected shape")
}

/// A closed square path from (10, 10) to (30, 30).
fn square_path() -> Shape {
    let points = [
        Point::new(10.0, 10.0),
        Point::new(30.0, 10.0),
        Point::new(30.0, 30.0),
        Point::new(10.0, 30.0),
    ];
    Shape::Path(DesignPath::from_points(&points, true))
}

#[test]
fn test_rectangle_corner_radius_and_slot_mode() {
    let d = design(
        vec![Shape::Rectangle(DesignRectangle::new(
            10.0, 10.0, 40.0, 20.0,
        ))],
        1,
    );
    let mut h = both(d);
    set_field(&mut h, "Radius:", "4");
    let Shape::Rectangle(r) = first(&h) else {
        panic!("rectangle");
    };
    assert!(close(r.corner_radius, 4.0));
    // The model clamps the radius to half the shorter side.
    set_field(&mut h, "Radius:", "50");
    let Shape::Rectangle(r) = first(&h) else {
        panic!("rectangle");
    };
    assert!(close(r.corner_radius, 10.0));
    toggle(&mut h, "Slot Mode:");
    let Shape::Rectangle(r) = first(&h) else {
        panic!("rectangle");
    };
    assert!(r.is_slot);
    undo(&mut h);
    let Shape::Rectangle(r) = first(&h) else {
        panic!("rectangle");
    };
    assert!(!r.is_slot);
    assert!(h.query_by_label("Sides:").is_none());
}

#[test]
fn test_path_fillet_and_closed() {
    let mut h = both(design(vec![square_path()], 1));
    assert!(
        h.get_by_label("Slot Mode:").accesskit_node().is_disabled(),
        "slot mode is a rectangle's"
    );
    set_field(&mut h, "Radius:", "2");
    assert!(close(object(&h).fillet, 2.0));
    toggle(&mut h, "Closed:");
    let Shape::Path(p) = first(&h) else {
        panic!("path");
    };
    assert!(!p.closed);
    undo(&mut h);
    let Shape::Path(p) = first(&h) else {
        panic!("path");
    };
    assert!(p.closed);
}

#[test]
fn test_polygon_sides_need_three_and_lock_while_rotated() {
    let polygon = DesignPolygon::new(Point::new(50.0, 50.0), 20.0, 6);
    let mut h = both(design(vec![Shape::Polygon(polygon)], 1));
    assert_eq!(field_text(&h, "Sides:"), "6");
    assert!(
        !field_enabled(&h, "Height:"),
        "a polygon's height follows its width"
    );
    set_field(&mut h, "Sides:", "8");
    set_field(&mut h, "Sides:", "2");
    let Shape::Polygon(p) = first(&h) else {
        panic!("polygon");
    };
    assert_eq!(p.sides, 8);
    set_field(&mut h, "Angle:", "10");
    assert!(!field_enabled(&h, "Sides:"));
}

#[test]
fn test_gear_parameters() {
    let gear = DesignGear::new(Point::new(50.0, 50.0), 2.0, 20);
    let mut h = both(design(vec![Shape::Gear(gear)], 1));
    assert!(h.query_by_label("Width:").is_none(), "no size section");
    assert!(h.query_by_label("Angle:").is_none(), "no rotation section");
    assert_eq!(field_text(&h, "Module:"), "2.00");
    set_field(&mut h, "Module:", "3");
    set_field(&mut h, "Teeth:", "24");
    set_field(&mut h, "Pressure Angle:", "14.5");
    set_field(&mut h, "Pressure Angle:", "50");
    let Shape::Gear(g) = first(&h) else {
        panic!("gear");
    };
    assert!(close(g.module, 3.0) && g.teeth == 24 && close(g.pressure_angle_deg, 14.5));
}

#[test]
fn test_sprocket_pitch_sets_the_standard_roller() {
    let sprocket = DesignSprocket::new(Point::new(50.0, 50.0), 12.7, 15);
    let mut h = both(design(vec![Shape::Sprocket(sprocket)], 1));
    let before = undo_depth(&h);
    set_field(&mut h, "Pitch:", "15.875");
    let Shape::Sprocket(s) = first(&h) else {
        panic!("sprocket");
    };
    assert!(close(s.pitch, 15.875));
    assert!(close(
        s.roller_diameter,
        DesignSprocket::standard_roller_diameter(15.875)
    ));
    assert_eq!(undo_depth(&h), before + 1, "pitch and roller in one step");
    set_field(&mut h, "Teeth:", "18");
    set_field(&mut h, "Roller Dia:", "9");
    let Shape::Sprocket(s) = first(&h) else {
        panic!("sprocket");
    };
    assert!(s.teeth == 18 && close(s.roller_diameter, 9.0));
}

#[test]
fn test_timing_pulley_parameters() {
    let path = generate_timing_pulley(Point::new(50.0, 50.0), 5.08, 20, 9.4, 5.0);
    let mut pulley = DesignPath::from_lyon_path(&path);
    pulley.parametric_source = Some(ParametricPathSource::TimingPulley {
        pitch: 5.08,
        teeth: 20,
        belt_width: 9.4,
        hole_radius: 5.0,
    });
    let mut h = both(design(vec![Shape::Path(pulley)], 1));
    assert!(h.query_by_label("Closed:").is_none(), "not a plain path");
    assert!(h.query_by_label("Radius:").is_none());
    set_field(&mut h, "Teeth:", "5");
    set_field(&mut h, "Teeth:", "30");
    set_field(&mut h, "Hole Radius:", "4");
    let params = timing_pulley(&first(&h)).expect("still a timing pulley");
    assert!(close(params.0, 5.08) && params.1 == 30);
    assert!(close(params.2, 9.4) && close(params.3, 4.0));
}

#[test]
fn test_text_content_size_and_style() {
    let text = DesignText::new("Hi".to_string(), 50.0, 50.0, 10.0);
    let mut h = both(design(vec![Shape::Text(text)], 1));
    assert!(h.query_by_label("Offset:").is_none(), "no effects for text");
    set_field(&mut h, "Content:", "Hello");
    set_field(&mut h, "Size:", "36");
    toggle(&mut h, "Bold");
    let Shape::Text(t) = first(&h) else {
        panic!("text");
    };
    assert_eq!(t.text, "Hello");
    assert!(close(t.font_size, 36.0 * MM_PER_PT));
    assert!(t.bold && !t.italic);
    assert_eq!(field_text(&h, "Size:"), "36.00");
    set_field(&mut h, "Size:", "0");
    let Shape::Text(t) = first(&h) else {
        panic!("text");
    };
    assert!(
        close(t.font_size, 36.0 * MM_PER_PT),
        "a size must be positive"
    );
}

#[test]
fn test_offset_previews_while_typing_and_applies_on_commit() {
    let d = design(
        vec![Shape::Circle(DesignCircle::new(
            Point::new(50.0, 50.0),
            10.0,
        ))],
        1,
    );
    cnc(&d);
    let mut h = both(std::rc::Rc::clone(&d));
    assert!(
        h.query_by_label("Angle:").is_none(),
        "a circle has no rotation"
    );
    let before = undo_depth(&h);
    type_text(&mut h, "Offset:", "2");
    assert_eq!(d.borrow().effect_preview().len(), 1, "previewed");
    assert!(close(object(&h).offset, 0.0), "not applied yet");
    crate::test_designer_canvas::press(&mut h, egui::Key::Enter);
    assert!(d.borrow().effect_preview().is_empty());
    assert!(close(object(&h).offset, 2.0));
    set_field(&mut h, "Chamfer:", "1");
    assert!(close(object(&h).chamfer, 1.0));
    assert_eq!(undo_depth(&h), before + 2);
    // Committing the shown value again records nothing.
    set_field(&mut h, "Offset:", "2.001");
    assert_eq!(undo_depth(&h), before + 2);
}
