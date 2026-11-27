//! Field rules that mirror the camtools validators closed by audit ticket T11
//! (UIPANES-2b, P2CAM-6a): the drill press peck limit and the probe safe height.

use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_ui_egui::panes::cam_tools::common::FieldError;
use gcodekit5_ui_egui::panes::cam_tools::drill_press::DrillPressParams;
use gcodekit5_ui_egui::panes::cam_tools::probe_routines::{ProbeParams, RoutineKind};

const PECK_DEPTH: &str = "Peck Depth (0 for none):";
const TOO_MANY_PECKS: &str = "Too small: the hole would take more than 10000 pecks";

fn drill(peck: &str) -> Result<(), Vec<FieldError>> {
    let mut page = DrillPressParams::with_center(MeasurementSystem::Metric, (100.0, 100.0));
    page.peck_depth = peck.into();
    page.parameters().map(|_| ())
}

#[test]
fn test_cam_validation_gaps_peck_past_max_pecks_errors_at_the_field() {
    // The default hole is 10 mm deep: 0.0009 mm pecks would take 11112 pecks.
    let errors = drill("0.0009").expect_err("too many pecks");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].field, Some(PECK_DEPTH));
    assert_eq!(errors[0].message, TOO_MANY_PECKS);
}

#[test]
fn test_cam_validation_gaps_peck_at_max_pecks_is_accepted() {
    // Exactly 10000 pecks is the generator's limit, not past it.
    assert!(drill("0.001").is_ok());
    assert!(drill("0").is_ok());
}

fn probe(routine: RoutineKind, safe_height: &str) -> Result<(), Vec<FieldError>> {
    let p = ProbeParams {
        routine,
        safe_height: safe_height.into(),
        ..ProbeParams::default()
    };
    p.parameters().map(|_| ())
}

#[test]
fn test_cam_validation_gaps_probe_zero_safe_height_only_matters_where_used() {
    for kind in [
        RoutineKind::EdgeFind,
        RoutineKind::CornerFind,
        RoutineKind::BoreCenter,
        RoutineKind::BossCenter,
    ] {
        assert!(probe(kind, "0").is_ok(), "{kind:?}");
    }
    for kind in [RoutineKind::ZTouch, RoutineKind::ToolLength] {
        assert!(probe(kind, "0").is_err(), "{kind:?}");
    }
}
