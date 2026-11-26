//! The visualizer inspector's S-value statistics (GTK's Min S, Max S and Avg S): taken over
//! every move and arc that carries a spindle/laser power (none after M5), one decimal, N/A
//! without any.

use std::sync::Arc;

use egui_kittest::kittest::Queryable;
use gcodekit5_ui_egui::panes::visualizer::VisualizerPane;
use gcodekit5_ui_egui::panes::visualizer::toolpath::{self, Intensity};

use crate::test_visualizer::load;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn program(lines: &[&str]) -> Arc<[String]> {
    lines.iter().map(|s| s.to_string()).collect()
}

/// Three powered moves at S100, S300 and S800 and an arc at S800.
fn powered() -> Arc<[String]> {
    program(&[
        "G21",
        "G90",
        "M3 S100",
        "G1 X10 F500",
        "S300",
        "G1 X20",
        "S800",
        "G1 X30",
        "G2 X40 Y0 I5 J0",
        "M5",
    ])
}

#[test]
fn test_intensity_statistics_as_gtk() {
    let mesh = toolpath::build(&powered());
    let stats = mesh.intensity.unwrap_or(Intensity {
        min: f32::NAN,
        max: f32::NAN,
        avg: f32::NAN,
    });
    assert_eq!((stats.min, stats.max), (100.0, 800.0));
    assert!(
        (stats.avg - 500.0).abs() < 1e-3,
        "one value per command: {}",
        stats.avg
    );
    let unpowered = toolpath::build(&program(&["G21", "M5", "G0 X10", "G1 X20 F100"]));
    assert_eq!(unpowered.intensity, None);
}

/// The pane beside its sidebar with `lines` loaded and the Inspector open.
fn inspector(
    lines: Arc<[String]>,
) -> Result<egui_kittest::Harness<'static, VisualizerPane>, Box<dyn std::error::Error>> {
    let tr = gcodekit5_app::i18n::Translator::new("en");
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(730.0, 700.0))
        .build_ui_state(
            move |ui, pane: &mut VisualizerPane| {
                let mut requested = Vec::new();
                let mut ctx = gcodekit5_ui_egui::workspace::PaneContext::new(&tr, &mut requested);
                gcodekit5_ui_egui::workspace::Pane::ui(pane, ui, &mut ctx);
            },
            VisualizerPane::new(),
        );
    load(&mut harness, lines)?;
    harness.get_by_label("Inspector").click();
    harness.run();
    Ok(harness)
}

#[test]
fn test_inspector_shows_the_s_statistics() -> TestResult {
    let harness = inspector(powered())?;
    for (row, value) in [
        ("Min S:", "100.0"),
        ("Max S:", "800.0"),
        ("Avg S:", "500.0"),
    ] {
        assert!(harness.query_by_label(row).is_some(), "{row}");
        assert!(harness.query_by_label(value).is_some(), "{row} {value}");
    }
    let harness = inspector(program(&["G21", "M5", "G0 X10"]))?;
    assert_eq!(harness.query_all_by_label("N/A").count(), 3);
    Ok(())
}
