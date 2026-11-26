//! The Gerber to G-Code page: GTK's defaults render, Browse picks the project folder through
//! the non-blocking dialog, a missing folder or layer and every unsafe value is shown on the
//! page and disables the actions, the program is exactly what the GTK page made from the same
//! small Gerber files (fixtures from `capture_cam_fixtures`), and presets in GTK's file format
//! load and save.
//!
//! The fixtures in `tests/fixtures/cam/` are frozen references: `capture_cam_fixtures`, which
//! wrote them from the GTK pages, was in the GTK crate deleted in `26740e3` (its source is at
//! `6841d84`). Change a fixture only deliberately, by hand, in the commit that changes the
//! generator's output and says why.

use std::path::{Path, PathBuf};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_app::i18n::Translator;
use gcodekit5_camtools::gerber::GerberLayerType;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::panes::cam_tools::CamTool;
use gcodekit5_ui_egui::panes::cam_tools::common::format_len;
use gcodekit5_ui_egui::panes::cam_tools::gerber::GerberParams;

use crate::test_cam_drill_press::{click, fixture, pane};
use crate::test_cam_tools::cam_app;
use crate::test_designer_files::scratch;
use crate::test_designer_files_app::{Shared, answer, default_state, frames, path, shown};

const METRIC: MeasurementSystem = MeasurementSystem::Metric;

/// Makes one field of the page unsafe.
type Breaker = fn(&mut GerberParams);

fn mm(v: f64) -> String {
    format_len(v, METRIC)
}

fn tr() -> Translator {
    Translator::new("en")
}

/// The checked-in Gerber project the fixtures were made from.
fn project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cam/gerber")
}

/// GTK's defaults with the project folder chosen.
fn page() -> GerberParams {
    let mut p = GerberParams::new(METRIC);
    p.set_directory(&project());
    p
}

/// The page of the `gerber_custom` case.
fn custom() -> GerberParams {
    let mut p = page();
    p.feed_rate = "300".into();
    p.spindle_speed = "12000".into();
    p.board_width = "40".into();
    p.board_height = "30".into();
    p.offset_x = "5".into();
    p.offset_y = "5".into();
    p.generate_alignment_holes = true;
    p.cut_depth = "-0.2".into();
    p.safe_z = "3".into();
    p.tool_diameter = "0.4".into();
    p.isolation_width = "0.2".into();
    p.rubout = true;
    p.use_board_outline = true;
    p
}

/// The app with the Gerber page open in a window that shows it whole, its dialogs scripted.
fn open_gerber() -> (Harness<'static, GcodeKitApp>, Shared) {
    let (mut h, script) = cam_app(default_state());
    h.get_by_label("Gerber to G-Code").click();
    frames(&mut h, 1);
    h.get_by_label("Open").click();
    frames(&mut h, 2);
    assert_eq!(
        h.state().cam_tools().borrow().open_tool(),
        Some(CamTool::Gerber)
    );
    h.set_size(egui::vec2(1280.0, 2400.0));
    frames(&mut h, 3);
    (h, script)
}

/// Browse, answered with the checked-in project folder.
fn browse(h: &mut Harness<'static, GcodeKitApp>, script: &Shared) {
    answer(script, [path(&project())]);
    click(h, "Browse...");
}

#[test]
fn test_cam_gerber_default_page_renders_and_waits_for_a_folder() {
    let (h, _script) = open_gerber();
    for label in [
        "Layer Type:",
        "Board Width:",
        "Board Height:",
        "Offset X:",
        "Offset Y:",
        "Feed Rate:",
        "Spindle Speed (RPM):",
        "Cut Depth:",
        "Safe Z:",
        "Tool Diameter:",
        "Isolation Width:",
        "Remove Excess Copper:",
        "Use Board Outline for Rubout:",
        "Generate Alignment Holes:",
        "Hole Diameter:",
        "Margin:",
        "Drill Depth (Z):",
        "Browse...",
        "No directory selected",
        "Generate",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label} not shown");
    }
    assert!(h.query_by_value("Top Copper").is_some());
    assert!(
        h.query_by_label("Please select a Gerber directory.")
            .is_some()
    );
    // Everything but the folder is valid.
    let errors = pane(&h).gerber_mut().parameters().expect_err("no folder");
    assert_eq!(errors.len(), 1);
}

#[test]
fn test_cam_gerber_generate_matches_gtk() {
    let tr = tr();
    assert_eq!(page().generate(&tr), Ok(fixture("gerber_default.gcode")));
    assert_eq!(custom().generate(&tr), Ok(fixture("gerber_custom.gcode")));
    let mut outline = page();
    outline.layer_type = GerberLayerType::BoardOutline;
    assert_eq!(outline.generate(&tr), Ok(fixture("gerber_outline.gcode")));
}

/// Browse asks for a folder, finds the layers and enables the actions; the buttons then make
/// GTK's program.
#[test]
fn test_cam_gerber_browse_then_generate_and_send_to_editor() {
    let (mut h, script) = open_gerber();
    browse(&mut h, &script);
    assert_eq!(shown(&script), ["Open Gerber Directory"]);
    {
        let mut pane = pane(&h);
        let gerber = pane.gerber_mut();
        assert_eq!(gerber.directory.as_deref(), Some(project().as_path()));
        assert!(gerber.layer_files.contains_key(&GerberLayerType::TopCopper));
        assert!(
            gerber
                .layer_files
                .contains_key(&GerberLayerType::BoardOutline)
        );
    }
    assert!(h.query_by_label("gerber").is_some(), "the folder's name");
    assert!(h.query_by_label("board.gtl").is_some(), "the layer's file");
    assert!(
        h.query_by_label("Please select a Gerber directory.")
            .is_none()
    );
    click(&mut h, "Generate");
    assert_eq!(
        pane(&h).output(),
        Some(fixture("gerber_default.gcode").as_str())
    );
    click(&mut h, "Send to Editor");
    assert_eq!(
        h.state().editor().borrow().buffer().text(),
        fixture("gerber_default.gcode")
    );
}

/// A cancelled folder dialog changes nothing.
#[test]
fn test_cam_gerber_browse_cancelled_keeps_the_page() {
    let (mut h, _script) = open_gerber();
    click(&mut h, "Browse...");
    assert_eq!(pane(&h).gerber_mut().directory, None);
}

/// A folder without the chosen layer says so and holds the actions back; a folder without
/// any layer file asks for another.
#[test]
fn test_cam_gerber_missing_layer_or_folder_is_shown() {
    let (mut h, script) = open_gerber();
    browse(&mut h, &script);
    pane(&h).gerber_mut().layer_type = GerberLayerType::BottomCopper;
    frames(&mut h, 2);
    assert!(
        h.query_by_label("Selected layer not found in directory.")
            .is_some()
    );
    assert!(h.query_by_label("Layer not found in directory").is_some());
    click(&mut h, "Generate");
    assert_eq!(pane(&h).output(), None);

    let empty = scratch("gerber_empty").expect("scratch folder");
    answer(&script, [path(&empty)]);
    click(&mut h, "Browse...");
    assert!(
        h.query_by_label("Please select a Gerber directory.")
            .is_some()
    );
    assert!(pane(&h).gerber_mut().layer_files.is_empty());
}

#[test]
fn test_cam_gerber_invalid_input_shows_error_and_disables_the_actions() {
    let cases: [(Breaker, &str); 12] = [
        (|p| p.feed_rate = "0".into(), "Must be greater than 0"),
        (|p| p.spindle_speed = "-5".into(), "Must be greater than 0"),
        (|p| p.board_width = "0".into(), "Must be greater than 0"),
        (|p| p.board_height = "tall".into(), "Not a valid length"),
        (|p| p.offset_x = "left".into(), "Not a valid length"),
        (|p| p.tool_diameter = "0".into(), "Must be greater than 0"),
        (|p| p.isolation_width = "-1".into(), "Must not be negative"),
        (
            |p| p.cut_depth = "0.5".into(),
            "Must be below the surface (negative)",
        ),
        (|p| p.safe_z = "0".into(), "Must be greater than 0"),
        (
            |p| {
                p.generate_alignment_holes = true;
                p.alignment_hole_diameter = "0".into();
            },
            "Must be greater than 0",
        ),
        (
            |p| {
                p.generate_alignment_holes = true;
                p.alignment_hole_margin = "-1".into();
            },
            "Must not be negative",
        ),
        (
            |p| {
                p.generate_alignment_holes = true;
                p.alignment_hole_depth = "1".into();
            },
            "Must be below the surface (negative)",
        ),
    ];
    // Control: a valid page generates, so a click that makes nothing below is a refusal.
    let (mut h, script) = open_gerber();
    browse(&mut h, &script);
    click(&mut h, "Generate");
    assert!(pane(&h).output().is_some(), "Generate does nothing");
    for (break_it, message) in cases {
        let (mut h, script) = open_gerber();
        browse(&mut h, &script);
        break_it(pane(&h).gerber_mut());
        frames(&mut h, 2);
        assert!(h.query_by_label(message).is_some(), "{message} not shown");
        for button in ["Generate", "Send to Editor", "Preview"] {
            click(&mut h, button);
            assert_eq!(
                pane(&h).output(),
                None,
                "{message}: {button} made a program"
            );
            assert_eq!(
                h.state().editor().borrow().buffer().text(),
                "",
                "{message}: {button} reached the editor"
            );
        }
    }
}

/// The alignment-hole fields only count while the holes are on, and the Z rules only on a
/// device with a Z axis.
#[test]
fn test_cam_gerber_limits_depend_on_the_holes_and_the_axes() {
    let mut p = page();
    p.alignment_hole_depth = "1".into();
    p.alignment_hole_diameter = "0".into();
    assert!(p.parameters().is_ok(), "holes are off");
    p.generate_alignment_holes = true;
    assert!(p.parameters().is_err(), "holes are on");
    let mut flat = page();
    flat.num_axes = 2;
    flat.cut_depth = "0.5".into();
    flat.safe_z = "0".into();
    assert!(flat.parameters().is_ok(), "no Z axis, no Z rules");
}

#[test]
fn test_cam_gerber_follows_the_measurement_system() {
    let mut p = page();
    p.set_units(MeasurementSystem::Imperial);
    assert_eq!(p.board_width, "3.937");
    assert_eq!(p.feed_rate, "100.0", "a feed stays in mm/min");
    let params = p.parameters().expect("valid");
    assert!((params.board_width - 100.0).abs() < 0.01);
    assert!((params.cut_depth - -0.1).abs() < 0.01);
}

/// The preset GTK wrote loads, and egui writes the same text for the same input.
#[test]
fn test_cam_gerber_preset_loads_the_gtk_file_and_round_trips() {
    let tr = tr();
    let json = fixture("preset_gerber.json");
    let mut expected = GerberParams::new(METRIC);
    expected.layer_type = GerberLayerType::BottomCopper;
    expected.feed_rate = "250".into();
    expected.spindle_speed = "9000".into();
    expected.board_width = mm(80.0);
    expected.board_height = mm(60.0);
    expected.offset_x = mm(2.0);
    expected.offset_y = mm(3.0);
    expected.generate_alignment_holes = true;
    expected.alignment_hole_diameter = mm(3.0);
    expected.alignment_hole_margin = mm(6.0);
    expected.alignment_hole_depth = mm(-2.0);
    expected.cut_depth = mm(-0.15);
    expected.safe_z = mm(4.0);
    expected.tool_diameter = mm(0.2);
    expected.isolation_width = mm(0.1);
    expected.rubout = true;
    expected.use_board_outline = true;
    let mut loaded = GerberParams::new(METRIC);
    loaded.apply_preset(&json).expect("loads");
    assert_eq!(loaded, expected);
    assert_eq!(loaded.preset_json(&tr).as_ref(), Ok(&json));

    let mut broken = GerberParams::new(METRIC);
    let before = broken.clone();
    assert!(broken.apply_preset("[1, 2]").is_err());
    assert_eq!(broken, before);

    let mut unreadable = GerberParams::new(METRIC);
    unreadable.tool_diameter = "fine".into();
    assert_eq!(
        unreadable.preset_json(&tr),
        Err("Tool Diameter: Not a valid length".to_string())
    );
}

/// A preset that names a project folder that still exists takes it and finds its layers; one
/// whose folder is gone leaves the page's folder as it is.
#[test]
fn test_cam_gerber_preset_takes_the_project_folder_when_it_exists() {
    let tr = tr();
    let mut saved = page();
    saved.layer_type = GerberLayerType::BoardOutline;
    let json = saved.preset_json(&tr).expect("saves");
    assert!(json.contains("\"directory\": \""), "{json}");
    let mut loaded = GerberParams::new(METRIC);
    loaded.apply_preset(&json).expect("loads");
    assert_eq!(loaded.directory.as_deref(), Some(project().as_path()));
    assert_eq!(loaded.layer_files, saved.layer_files);
    assert_eq!(loaded.layer_type, GerberLayerType::BoardOutline);

    let gone = json.replace(
        &project().to_string_lossy().to_string(),
        "/no/such/gerber/folder",
    );
    let mut other = GerberParams::new(METRIC);
    other.apply_preset(&gone).expect("loads");
    assert_eq!(other.directory, None);
    assert!(other.layer_files.is_empty());
}
