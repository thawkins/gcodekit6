//! CAM dispatch (`designer_state::cam`): the one owner of the shape-to-toolpath rules that
//! G-code export and the canvas preview share.

use gcodekit5_designer::DesignerState;
use gcodekit5_designer::designer_state::MachineMode;
use gcodekit5_designer::designer_state::cam::CamSettings;
use gcodekit5_designer::shapes::OperationType;
use gcodekit5_designer::toolpath::{ToolpathGenerator, ToolpathSegmentType};

/// A laser-mode design with one rectangle cut with its own feed (777 mm/min).
fn laser_state() -> (DesignerState, u64) {
    let mut state = DesignerState::new();
    state.set_machine_mode(MachineMode::Laser2D);
    state.tool_settings.feed_rate = 1234.0;
    let id = state.canvas.add_rectangle(0.0, 0.0, 20.0, 10.0);
    let obj = state
        .canvas
        .get_shape_mut(id)
        .expect("shape was just added");
    obj.use_global_laser = false;
    obj.laser_params.use_global = false;
    obj.laser_params.feed_rate = 777.0;
    obj.laser_params.power_percent = 40.0;
    if let gcodekit5_designer::model::Shape::Rectangle(r) = &mut obj.shape {
        r.laser_params.use_global = false;
    }
    (state, id)
}

#[test]
fn test_cam_dispatch_laser_shape_uses_its_own_feed() {
    let (state, id) = laser_state();
    let shape = state.canvas.get_shape(id).expect("shape exists").clone();
    let built = CamSettings::of(&state).shape_toolpaths(&mut ToolpathGenerator::new(), &shape);
    let cuts: Vec<f64> = built
        .toolpaths
        .iter()
        .flat_map(|tp| tp.segments.iter())
        .filter(|s| s.segment_type != ToolpathSegmentType::RapidMove)
        .map(|s| s.feed_rate)
        .collect();
    assert!(!cuts.is_empty(), "the rectangle is cut");
    assert!(cuts.iter().all(|&f| (f - 777.0).abs() < 1e-9), "{cuts:?}");
    // The export cuts with the same feed.
    let mut state = state;
    let gcode = state.generate_gcode(None);
    assert!(gcode.contains("F777"), "{gcode}");
    assert!(!gcode.contains("F1234"), "{gcode}");
}

#[test]
fn test_cam_dispatch_cnc_defaults_apply_without_custom_values() {
    let mut state = DesignerState::new();
    state.set_machine_mode(MachineMode::Cnc3D);
    state.tool_settings.step_down = 0.8;
    state.default_properties_shape.operation_type = OperationType::Pocket;
    let id = state.canvas.add_rectangle(0.0, 0.0, 20.0, 10.0);
    let obj = state
        .canvas
        .get_shape_mut(id)
        .expect("shape was just added");
    obj.use_custom_values = false;
    obj.operation_type = OperationType::Profile;
    obj.step_down = 3.0;
    let shape = obj.clone();
    let effective = CamSettings::of(&state).effective_object(&shape);
    assert_eq!(effective.operation_type, OperationType::Pocket);
    assert!((effective.step_down - 0.8).abs() < 1e-6);

    let mut custom = shape;
    custom.use_custom_values = true;
    let effective = CamSettings::of(&state).effective_object(&custom);
    assert_eq!(effective.operation_type, OperationType::Profile);
    assert!((effective.step_down - 3.0).abs() < 1e-6);
}
