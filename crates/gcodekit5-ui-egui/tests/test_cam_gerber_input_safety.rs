//! The Gerber page tells the user about constructs the converter leaves out (audit T10), and
//! the Drill Holes layer makes no program.

use egui_kittest::kittest::Queryable;
use gcodekit5_app::i18n::Translator;
use gcodekit5_camtools::gerber::GerberLayerType;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_ui_egui::panes::cam_tools::CamTool;
use gcodekit5_ui_egui::panes::cam_tools::gerber::GerberParams;

use crate::test_cam_drill_press::pane;
use crate::test_cam_tools::cam_app;
use crate::test_designer_files::scratch;
use crate::test_designer_files_app::{default_state, frames};

const WARNING: &str = "Unsupported Gerber content is left out of the program";

/// A top copper layer with one G36/G37 region (a copper pour).
const REGION_COPPER: &str = "%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.2*%\nD10*\nG36*\nX0Y0D02*\n\
                             X10000000Y0D01*\nX10000000Y10000000D01*\nX0Y0D01*\nG37*\nM02*\n";

#[test]
fn test_cam_gerber_input_safety_region_warning_is_shown() {
    let dir = scratch("gerber_region").expect("scratch folder");
    std::fs::write(dir.join("board.gtl"), REGION_COPPER).expect("write");

    let (mut h, _script) = cam_app(default_state());
    h.get_by_label("Gerber to G-Code").click();
    frames(&mut h, 1);
    h.get_by_label("Open").click();
    frames(&mut h, 2);
    assert_eq!(
        h.state().cam_tools().borrow().open_tool(),
        Some(CamTool::Gerber)
    );
    h.set_size(egui::vec2(1280.0, 2400.0));
    pane(&h).gerber_mut().set_directory(&dir);
    frames(&mut h, 3);
    assert_eq!(
        pane(&h)
            .gerber_mut()
            .unsupported
            .get(&GerberLayerType::TopCopper)
            .map(|c| c.regions),
        Some(1)
    );
    assert!(
        h.query_by_label_contains(WARNING).is_some(),
        "the warning is not shown"
    );
}

#[test]
fn test_cam_gerber_input_safety_drill_layer_is_refused() {
    let dir = scratch("gerber_drill").expect("scratch folder");
    std::fs::write(
        dir.join("board.drl"),
        "M48\nMETRIC\nT1C0.8\n%\nT1\nX1.0Y1.0\nM30\n",
    )
    .expect("write");
    let mut p = GerberParams::new(MeasurementSystem::Metric);
    p.set_directory(&dir);
    p.layer_type = GerberLayerType::DrillHoles;
    assert!(p.parameters().is_ok());
    assert!(p.generate(&Translator::new("en")).is_err());
}
