//! Locks that the drill press selects the XY plane before any helical (G2/G3) arc.

use gcodekit5_camtools::drill_press::{DrillPressGenerator, DrillPressParameters};

fn program(hole_diameter: f64) -> String {
    DrillPressGenerator::new(DrillPressParameters {
        hole_diameter,
        tool_diameter: 6.0,
        top_z: 0.0,
        bottom_z: -6.0,
        peck_depth: 2.0,
        plunge_rate: 100.0,
        feed_rate: 500.0,
        spindle_speed: 10000.0,
        safe_z: 5.0,
        x: 10.0,
        y: 20.0,
        home_before_job: false,
    })
    .generate()
    .expect("generate drill program")
}

#[test]
fn test_drill_press_plane_g17_before_first_arc() {
    let gcode = program(12.0);
    let g17 = gcode.find("G17").expect("program selects the XY plane");
    let arc = gcode.find("G2 ").expect("helical hole emits an arc");
    assert!(g17 < arc, "G17 must precede the first arc");
}

#[test]
fn test_drill_press_plane_g17_for_plain_drill() {
    assert!(program(6.0).contains("G17 ; XY plane selection\n"));
}
