//! Probe programs end by restoring the modal state they changed (audit T22, P2CAM-6b):
//! `G90` and the caller's units (`G20` for imperial, `G21` for metric), since the routines
//! themselves run in `G21 G91` and GRBL has no modal push/pop.

use gcodekit5_camtools::probe_routines::{ProbeOptions, ProbeRoutineEngine};
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_core::{Corner, Position, ProbeAxis, ProbeDirection, ProbeRoutine};

/// One routine of each kind, with valid parameters.
fn routines() -> Vec<ProbeRoutine> {
    vec![
        ProbeRoutine::ZTouch {
            safe_height: 5.0,
            max_depth: 20.0,
            fast_feed: 100.0,
            slow_feed: 25.0,
            backoff: 2.0,
        },
        ProbeRoutine::EdgeFind {
            axis: ProbeAxis::X,
            direction: ProbeDirection::Negative,
            safe_height: 5.0,
            probe_distance: 10.0,
            fast_feed: 100.0,
            slow_feed: 25.0,
            backoff: 2.0,
        },
        ProbeRoutine::CornerFind {
            corner: Corner::XminYmin,
            safe_height: 5.0,
            probe_distance: 10.0,
            fast_feed: 100.0,
            slow_feed: 25.0,
            backoff: 2.0,
        },
        ProbeRoutine::BoreCenter {
            diameter: 20.0,
            safe_height: 5.0,
            fast_feed: 100.0,
            slow_feed: 25.0,
        },
        ProbeRoutine::BossCenter {
            diameter: 20.0,
            safe_height: 5.0,
            fast_feed: 100.0,
            slow_feed: 25.0,
        },
        ProbeRoutine::ToolLength {
            plate_xy: (0.0, 0.0),
            plate_z: -50.0,
            safe_height: 5.0,
            max_depth: 20.0,
            feed_rate: 100.0,
        },
    ]
}

/// The program `routine` makes when the user works in `units`.
fn program(routine: &ProbeRoutine, units: MeasurementSystem) -> String {
    let options = ProbeOptions {
        restore_units: units,
        ..ProbeOptions::default()
    };
    ProbeRoutineEngine::with_options(routine.clone(), options)
        .generate(Position::new(1.0, 2.0, 3.0))
        .expect("valid routine")
        .gcode
}

#[test]
fn test_probe_mode_restore_inches_ends_with_g90_g20() {
    for routine in routines() {
        let gcode = program(&routine, MeasurementSystem::Imperial);
        assert!(
            gcode.ends_with("G20 ; Restore inches\nG90 ; Restore absolute moves\n"),
            "{routine:?}:\n{gcode}"
        );
        // The routine itself still runs in mm.
        assert!(gcode.contains("G21 ; Millimetres\n"), "{routine:?}");
    }
}

#[test]
fn test_probe_mode_restore_metric_differs_only_by_the_units_line() {
    for routine in routines() {
        let mm = program(&routine, MeasurementSystem::Metric);
        let inch = program(&routine, MeasurementSystem::Imperial);
        assert!(
            mm.ends_with("G21 ; Restore millimetres\nG90 ; Restore absolute moves\n"),
            "{routine:?}:\n{mm}"
        );
        // Everything but the restored units is the same.
        let inch_as_mm = inch.replace("G20 ; Restore inches", "G21 ; Restore millimetres");
        assert_eq!(mm, inch_as_mm, "{routine:?}");
    }
}

#[test]
fn test_probe_mode_restore_defaults_to_metric() {
    assert_eq!(
        ProbeOptions::default().restore_units,
        MeasurementSystem::Metric
    );
}
