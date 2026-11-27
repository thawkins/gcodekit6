//! The Probe Tools page passes its display units to the generator, so a program made while
//! the UI is in inches ends by restoring `G20` and `G90` (audit T22, P2CAM-6b); in mm the
//! frozen `probe_*.gcode` fixtures pin the `G21` restore.

use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_ui_egui::panes::cam_tools::probe_routines::ProbeParams;

use crate::test_cam_probe_routines::{cases, start};

#[test]
fn test_cam_probe_mode_restore_inches_ends_with_g20() {
    for (name, mut p) in cases() {
        p.set_units(MeasurementSystem::Imperial);
        let gcode = p.generate(start()).expect("a program").gcode;
        assert!(
            gcode.ends_with("G20 ; Restore inches\nG90 ; Restore absolute moves\n"),
            "{name}:\n{gcode}"
        );
        let lines = p.run_lines(start()).expect("run lines");
        assert_eq!(
            lines.iter().rev().take(2).collect::<Vec<_>>(),
            ["G90", "G20"],
            "{name}"
        );
    }
}

#[test]
fn test_cam_probe_mode_restore_metric_page_restores_g21() {
    let p = ProbeParams::new(MeasurementSystem::Metric);
    let gcode = p.generate(start()).expect("a program").gcode;
    assert!(
        gcode.ends_with("G21 ; Restore millimetres\nG90 ; Restore absolute moves\n"),
        "{gcode}"
    );
}
