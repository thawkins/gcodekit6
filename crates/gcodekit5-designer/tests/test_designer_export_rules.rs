//! The export rules the designer pane uses from the model (audit T31, UIPANES-3): the Safe Z
//! clearance check limited to the exported shapes, a raster image's engraving parameters, and
//! the tool diameter taken from the tool settings when generating.

use gcodekit5_designer::DesignerState;
use gcodekit5_designer::designer_state::MachineMode;
use gcodekit5_designer::engraving::{HalftoneMethod, ScanDirection};
use gcodekit5_designer::model::{DesignRectangle, Point, RasterImage, Shape};

/// A CNC design with two rectangles starting at the stock top (start depth 0).
fn cnc_with_two_rects() -> (DesignerState, Vec<u64>) {
    let mut state = DesignerState::new();
    state.set_machine_mode(MachineMode::Cnc3D);
    state.add_shape_with_undo(Shape::Rectangle(DesignRectangle::new(0.0, 0.0, 10.0, 10.0)));
    state.add_shape_with_undo(Shape::Rectangle(DesignRectangle::new(
        20.0, 0.0, 10.0, 10.0,
    )));
    let ids: Vec<u64> = state.canvas.shapes().map(|o| o.id).collect();
    for id in &ids {
        if let Some(obj) = state.canvas.get_shape_mut(*id) {
            obj.start_depth = 0.0;
        }
    }
    if let Some(stock) = state.stock_material.as_mut() {
        stock.thickness = 12.0;
        stock.safe_z = 5.0;
    }
    (state, ids)
}

#[test]
fn test_safe_z_summary_checks_only_exported_shapes() {
    let (mut state, ids) = cnc_with_two_rects();
    if let Some(obj) = state.canvas.get_shape_mut(ids[1]) {
        obj.start_depth = -3.0;
    }
    assert_eq!(
        state.safe_z_clearance_violation_summary(|_| true),
        Some((5.0, 2, 15.0))
    );
    assert_eq!(
        state.safe_z_clearance_violation_summary(|o| o.id != ids[1]),
        Some((5.0, 1, 12.0)),
        "a rejected shape is neither counted nor the highest"
    );
    assert_eq!(state.safe_z_clearance_violation_summary(|_| false), None);
    state.set_machine_mode(MachineMode::Laser2D);
    assert_eq!(state.safe_z_clearance_violation_summary(|_| true), None);
}

#[test]
fn test_engraving_params_keep_the_placed_size_and_corner() {
    let mut image = RasterImage::new(1, Point::new(20.0, 15.0), 30.0, 20.0, Vec::new(), None);
    image.scan_direction = "vertical".into();
    image.dithering = "floyd".into();
    image.ppi = 127.0;
    let params = image.engraving_params();
    assert_eq!(params.width_mm, 30.0);
    assert_eq!(
        params.height_mm,
        Some(20.0),
        "the placed height, not the pixel aspect"
    );
    assert_eq!((params.offset_x, params.offset_y), (5.0, 5.0));
    assert_eq!(params.scan_direction, ScanDirection::Vertical);
    assert_eq!(params.halftone, HalftoneMethod::from_name("floyd"));
    assert_eq!((params.feed_rate, params.travel_rate), (2000.0, 3000.0));
    assert_eq!(
        (params.min_power, params.max_power, params.ppi),
        (0.0, 20.0, 127.0)
    );
    assert_eq!((params.power_scale, params.line_spacing), (1000.0, 1.0));
    assert!(params.bidirectional && params.invert && !params.mirror_x && !params.mirror_y);
}

/// The model cuts with the tool settings' diameter even when its toolpath generator was not
/// told (the pane no longer sets the generator before generating).
#[test]
fn test_generate_cuts_with_the_tool_settings_diameter() {
    let (mut state, _) = cnc_with_two_rects();
    state.set_tool_diameter(3.175);
    state.tool_settings.tool_diameter = 6.0;
    let gcode = state.generate_gcode(None);
    assert!(gcode.contains("; Tool diameter: 6.00mm"), "{gcode}");
}
