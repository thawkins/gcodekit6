//! Designer properties panel, the machining sections: CAM properties in CNC mode (use
//! global values, operation, step down/in, ramp angle, strategy, raster fill), the laser
//! override in laser mode (one shape and several, with mixed values), and a raster image's
//! engraving settings.

use std::cell::RefCell;
use std::rc::Rc;

use egui_kittest::kittest::{NodeT, Queryable};
use gcodekit6_designer::model::{DesignRectangle, RasterImage, Shape};
use gcodekit6_designer::pocket_operations::PocketStrategy;
use gcodekit6_designer::shapes::OperationType;
use gcodekit6_ui_egui::panes::designer::DesignerPane;

use crate::test_designer_canvas::close;
use crate::test_designer_properties::{
    Both, both, cnc, design, field_enabled, field_text, first, pick, set_field, shapes, toggle,
    two_rects, undo, undo_depth,
};

/// The selected objects' copies.
fn objects(h: &egui_kittest::Harness<'_, Both>) -> Vec<gcodekit6_designer::canvas::DrawingObject> {
    let d = h.state().designer.borrow();
    d.state()
        .canvas
        .shapes()
        .filter(|o| o.selected)
        .cloned()
        .collect()
}

/// Two rectangles, both selected, in laser mode (the model's default).
fn two_laser_rects() -> Rc<RefCell<DesignerPane>> {
    design(
        vec![
            Shape::Rectangle(DesignRectangle::new(10.0, 10.0, 20.0, 10.0)),
            Shape::Rectangle(DesignRectangle::new(50.0, 10.0, 20.0, 10.0)),
        ],
        2,
    )
}

/// The laser parameters embedded in `shape`.
fn laser(shape: &Shape) -> gcodekit6_designer::model::LaserParams {
    match shape {
        Shape::Rectangle(r) => r.laser_params,
        other => panic!("not a rectangle: {other:?}"),
    }
}

#[test]
fn test_cam_fields_follow_use_global_and_pocket() {
    let mut h = both(two_rects());
    assert!(
        h.query_by_label("Feed rate:").is_none(),
        "no laser override in CNC"
    );
    if !objects(&h)[0].use_custom_values {
        assert!(!field_enabled(&h, "Step Down:"));
        toggle(&mut h, "Use global values");
    }
    assert!(objects(&h)[0].use_custom_values);
    assert!(field_enabled(&h, "Step Down:"));
    assert!(!field_enabled(&h, "Step In:"), "step in is a pocket's");
    pick(&mut h, 0, "Pocket");
    assert_eq!(objects(&h)[0].operation_type, OperationType::Pocket);
    assert!(field_enabled(&h, "Step In:"));
    set_field(&mut h, "Step Down:", "0.5");
    set_field(&mut h, "Step In:", "1.25");
    set_field(&mut h, "Ramp Angle:", "3");
    set_field(&mut h, "Raster Fill (%):", "150");
    pick(&mut h, 1, "Adaptive");
    let o = &objects(&h)[0];
    assert!(close(f64::from(o.step_down), 0.5));
    assert!(close(f64::from(o.step_in), 1.25));
    assert!(close(o.ramp_angle, 3.0));
    assert!(close(o.raster_fill_ratio, 1.0), "clamped to 100 %");
    assert_eq!(o.pocket_strategy, PocketStrategy::Adaptive);
    assert_eq!(field_text(&h, "Raster Fill (%):"), "100");
    undo(&mut h);
    assert_ne!(objects(&h)[0].pocket_strategy, PocketStrategy::Adaptive);
}

#[test]
fn test_cam_change_applies_to_every_selected_shape() {
    let d = two_laser_rects();
    cnc(&d);
    let mut h = both(d);
    if !objects(&h)[0].use_custom_values {
        toggle(&mut h, "Use global values");
    }
    let before = undo_depth(&h);
    set_field(&mut h, "Step Down:", "0.75");
    assert_eq!(undo_depth(&h), before + 1);
    assert!(
        objects(&h)
            .iter()
            .all(|o| close(f64::from(o.step_down), 0.75))
    );
}

#[test]
fn test_laser_override_for_one_shape() {
    let d = design(
        vec![Shape::Rectangle(DesignRectangle::new(
            10.0, 10.0, 20.0, 10.0,
        ))],
        1,
    );
    let mut h = both(d);
    assert!(
        h.query_by_label("Step Down:").is_none(),
        "no CAM in laser mode"
    );
    let uses_global = laser(&first(&h)).use_global;
    assert_eq!(field_enabled(&h, "Power:"), !uses_global);
    if uses_global {
        toggle(&mut h, "Use global values");
    }
    set_field(&mut h, "Power:", "40");
    set_field(&mut h, "Passes:", "3");
    let p = laser(&first(&h));
    assert!(!p.use_global && close(p.power_percent, 40.0) && p.passes == 3);
    let o = &objects(&h)[0];
    assert!(!o.use_global_laser && close(o.laser_params.power_percent, 40.0));
    toggle(&mut h, "Use global values");
    assert!(laser(&first(&h)).use_global);
    assert!(!field_enabled(&h, "Power:"));
}

#[test]
fn test_laser_override_for_several_shapes_shows_mixed_values() {
    let mut h = both(two_laser_rects());
    if laser(&shapes(&h)[0]).use_global {
        toggle(&mut h, "Use global values");
    }
    // Give the shapes different powers, then edit only the passes.
    {
        let d = &h.state().designer;
        let mut d = d.borrow_mut();
        let ids: Vec<u64> = d.state().canvas.shapes().map(|o| o.id).collect();
        if let Some(o) = d.state_mut().canvas.get_shape_mut(ids[1])
            && let Shape::Rectangle(r) = &mut o.shape
        {
            r.laser_params.power_percent = 55.0;
        }
    }
    h.run();
    assert_eq!(field_text(&h, "Power:"), "---");
    set_field(&mut h, "Passes:", "2");
    let powers: Vec<f64> = shapes(&h).iter().map(|s| laser(s).power_percent).collect();
    assert!(close(powers[1], 55.0), "a mixed value is kept: {powers:?}");
    assert!(shapes(&h).iter().all(|s| laser(s).passes == 2));
}

#[test]
fn test_raster_image_engraving_settings() {
    let image = RasterImage::default();
    let d = design(vec![Shape::RasterImage(image)], 1);
    let mut h = both(Rc::clone(&d));
    assert!(
        h.query_by_label("Feed rate:").is_none(),
        "no laser override for images"
    );
    assert!(
        h.query_by_label("Angle:").is_none(),
        "no rotation for images"
    );
    assert!(h.query_by_label("Offset:").is_none());
    assert!(
        h.get_by_label("Lock Aspect:")
            .accesskit_node()
            .is_disabled()
    );
    set_field(&mut h, "Feed Rate:", "80");
    set_field(&mut h, "Max Power:", "250");
    set_field(&mut h, "Resolution:", "0");
    set_field(&mut h, "Halftone:", "300");
    set_field(&mut h, "Halftone:", "64");
    let inverted = matches!(first(&h), Shape::RasterImage(ref r) if r.invert);
    toggle(&mut h, "Invert:");
    pick(&mut h, 1, "Floyd-Ste.");
    pick(&mut h, 0, "Vertical");
    let Shape::RasterImage(r) = first(&h) else {
        panic!("image");
    };
    assert!(close(r.feed_rate, 80.0));
    assert!(close(r.max_power, 100.0), "clamped");
    assert!(close(r.ppi, 1.0), "clamped");
    assert_eq!(r.halftone_threshold, 64);
    assert_eq!(r.invert, !inverted);
    assert_eq!(r.dithering, "floyd");
    assert_eq!(r.scan_direction, "vertical");
}

#[test]
fn test_cam_rejects_unsafe_step_and_ramp_values() {
    let mut h = both(two_rects());
    if !objects(&h)[0].use_custom_values {
        toggle(&mut h, "Use global values");
    }
    pick(&mut h, 0, "Pocket");
    let before = (
        objects(&h)[0].step_down,
        objects(&h)[0].step_in,
        objects(&h)[0].ramp_angle,
    );
    set_field(&mut h, "Step Down:", "0");
    h.get_by_label("Enter a value greater than 0");
    set_field(&mut h, "Step In:", "-1");
    set_field(&mut h, "Ramp Angle:", "95");
    h.get_by_label("Enter an angle from 0 to 90");
    let o = &objects(&h)[0];
    assert_eq!((o.step_down, o.step_in, o.ramp_angle), before);
    set_field(&mut h, "Ramp Angle:", "90");
    assert!(close(objects(&h)[0].ramp_angle, 90.0));
}

#[test]
fn test_laser_override_rejects_zero_feed_and_passes() {
    let d = design(
        vec![Shape::Rectangle(DesignRectangle::new(
            10.0, 10.0, 20.0, 10.0,
        ))],
        1,
    );
    let mut h = both(d);
    if laser(&first(&h)).use_global {
        toggle(&mut h, "Use global values");
    }
    let before = laser(&first(&h));
    set_field(&mut h, "Feed rate:", "0");
    h.get_by_label("Enter a value greater than 0");
    set_field(&mut h, "Passes:", "0");
    h.get_by_label("Enter a whole number of 1 or more");
    let p = laser(&first(&h));
    assert!(close(p.feed_rate, before.feed_rate) && p.passes == before.passes);
}
