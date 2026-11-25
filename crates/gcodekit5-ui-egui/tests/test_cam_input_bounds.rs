//! Input bounds of the CAM pages: the shared rules compare a value as the generators get it
//! (`f32`), so a value that rounds to zero or overflows to infinity is refused, and the
//! Tabbed Box Maker refuses more dividers than fit their side and an unbounded pass count.

use egui_kittest::kittest::Queryable;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_ui_egui::panes::cam_tools::CamTool;
use gcodekit5_ui_egui::panes::cam_tools::common::{
    MUST_BE_POSITIVE, OUT_OF_RANGE, at_least, between, not_negative, positive,
};
use gcodekit5_ui_egui::panes::cam_tools::spoilboard_grid::SpoilboardGridParams;
use gcodekit5_ui_egui::panes::cam_tools::tabbed_box::TabbedBoxParams;

use crate::test_cam_drill_press::{click, open_tool_tall, pane};
use crate::test_designer_files_app::frames;

const METRIC: MeasurementSystem = MeasurementSystem::Metric;
const TOO_MANY_DIVIDERS: &str = "Too many dividers: they would not fit the side";
const TOO_MANY_PASSES: &str = "Must be at most 100";

/// The messages `params` reports for the field labelled `field`.
fn box_errors(params: &TabbedBoxParams, field: &str) -> Vec<&'static str> {
    params
        .parameters()
        .err()
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.field == Some(field))
        .map(|e| e.message)
        .collect()
}

#[test]
fn test_cam_rules_compare_as_f32() {
    // 1e-50 is 0.0 as f32; 1e40 is infinity.
    assert_eq!(positive(1e-50), Some(MUST_BE_POSITIVE));
    assert_eq!(positive(1e40), Some(OUT_OF_RANGE));
    assert_eq!(positive(-1e40), Some(MUST_BE_POSITIVE));
    assert_eq!(positive(0.5), None);
    assert_eq!(not_negative(1e40), Some(OUT_OF_RANGE));
    assert_eq!(not_negative(-1e-50), None);
    assert_eq!(not_negative(0.0), None);
    assert_eq!(at_least(1.0, "low")(1e40), Some(OUT_OF_RANGE));
    assert_eq!(at_least(1.0, "low")(2.0), None);
    assert!(between(0.0, 10.0, "range")(1e40).is_some());
}

#[test]
fn test_cam_page_refuses_values_lost_as_f32() {
    let mut p = SpoilboardGridParams::new(METRIC);
    p.feed_rate = "1e-50".into();
    p.laser_power = "1e40".into();
    let errors = p.parameters().expect_err("refused");
    assert!(
        errors
            .iter()
            .any(|e| e.field == Some("Feed Rate (mm/min):") && e.message == MUST_BE_POSITIVE),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|e| e.field == Some("Laser Power (S):") && e.message == OUT_OF_RANGE),
        "{errors:?}"
    );
}

#[test]
fn test_cam_tabbed_box_dividers_must_fit_their_side() {
    // 100 mm sides, 3 mm thick: 32 dividers leave 100 / 33 = 3.03 mm each; 33 do not fit.
    let mut p = TabbedBoxParams::new(METRIC);
    p.dividers_x = "32".into();
    p.dividers_y = "32".into();
    assert!(p.parameters().is_ok(), "{:?}", p.parameters().err());
    for text in ["33", "100", "1000000000"] {
        p.dividers_x = text.into();
        assert_eq!(box_errors(&p, "Dividers X:"), [TOO_MANY_DIVIDERS], "{text}");
        assert!(box_errors(&p, "Dividers Y:").is_empty());
    }
    p.dividers_x = "0".into();
    p.dividers_y = "33".into();
    assert_eq!(box_errors(&p, "Dividers Y:"), [TOO_MANY_DIVIDERS]);
    // Outside sizes: the walls take 2 x 3 mm, leaving 94 mm: 30 fit, 31 do not.
    p.dividers_y = "0".into();
    p.outside = true;
    p.dividers_x = "30".into();
    assert!(p.parameters().is_ok(), "{:?}", p.parameters().err());
    p.dividers_x = "31".into();
    assert_eq!(box_errors(&p, "Dividers X:"), [TOO_MANY_DIVIDERS]);
}

#[test]
fn test_cam_tabbed_box_passes_are_bounded() {
    let mut p = TabbedBoxParams::new(METRIC);
    p.passes = "100".into();
    assert!(p.parameters().is_ok());
    for text in ["101", "1000000000"] {
        p.passes = text.into();
        assert_eq!(box_errors(&p, "Passes:"), [TOO_MANY_PASSES], "{text}");
    }
}

#[test]
fn test_cam_tabbed_box_bounds_show_inline_and_disable_generate() {
    for (field, message) in [("dividers", TOO_MANY_DIVIDERS), ("passes", TOO_MANY_PASSES)] {
        let mut h = open_tool_tall(CamTool::TabbedBox);
        {
            let mut cam = pane(&h);
            let p = cam.tabbed_box_mut();
            match field {
                "dividers" => p.dividers_x = "100".into(),
                _ => p.passes = "1000000000".into(),
            }
        }
        frames(&mut h, 2);
        assert!(h.query_by_label(message).is_some(), "{message} not shown");
        click(&mut h, "Generate");
        assert_eq!(
            pane(&h).output(),
            None,
            "{message}: Generate made a program"
        );
    }
}
