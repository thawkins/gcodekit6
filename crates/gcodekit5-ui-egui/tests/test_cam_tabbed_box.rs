//! The Tabbed Box Maker page: GTK's defaults render, invalid input is shown at the field and
//! disables every generating action, the program and the panel outlines are exactly what the
//! GTK page made (fixtures from `capture_cam_fixtures`), and presets in GTK's file format
//! load and save.

use egui_kittest::kittest::Queryable;
use gcodekit5_app::i18n::Translator;
use gcodekit5_camtools::tabbed_box::{BoxType, KeyDividerType};
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_ui_egui::panes::cam_tools::CamTool;
use gcodekit5_ui_egui::panes::cam_tools::common::format_len;
use gcodekit5_ui_egui::panes::cam_tools::requests::DesignerPaths;
use gcodekit5_ui_egui::panes::cam_tools::tabbed_box::TabbedBoxParams;
use gcodekit5_ui_egui::workspace::PaneId;

use crate::test_cam_drill_press::{click, fixture, open_tool, open_tool_tall, pane};
use crate::test_designer_files_app::frames;

const METRIC: MeasurementSystem = MeasurementSystem::Metric;

/// Makes one field of the page unsafe.
type Breaker = fn(&mut TabbedBoxParams);

fn mm(v: f64) -> String {
    format_len(v, METRIC)
}

/// The outlines as the fixtures list them.
fn paths_text(generated: &DesignerPaths) -> String {
    let mut text = String::new();
    for (i, path) in generated.paths.iter().enumerate() {
        text.push_str(&format!(
            "path {i} closed={} points={}\n",
            path.closed,
            path.points.len()
        ));
        for (x, y) in &path.points {
            text.push_str(&format!("{x} {y}\n"));
        }
    }
    text
}

/// The page of the `tabbed_box_custom` case: imperial, an outside-size no-top box.
fn custom() -> TabbedBoxParams {
    let mut p = TabbedBoxParams::new(MeasurementSystem::Imperial);
    p.width = "5".into();
    p.depth = "4".into();
    p.height = "3".into();
    p.outside = true;
    p.thickness = "0.2".into();
    p.burn = "0.004".into();
    p.finger_width = "1.5".into();
    p.space_width = "2.5".into();
    p.surrounding_spaces = "1".into();
    p.play = "0.002".into();
    p.extra_length = "0".into();
    p.box_type = BoxType::NoTop;
    p.dividers_x = "1".into();
    p.dividers_y = "2".into();
    p.divider_keying = KeyDividerType::WallsOnly;
    p.optimize_layout = true;
    p.passes = "2".into();
    p.power = "800".into();
    p.feed_rate = "400".into();
    p.z_step_down = "0.01".into();
    p.offset_x = "0.5".into();
    p.offset_y = "0.5".into();
    p.home_before = true;
    p
}

/// The page of the `preset_tabbed_box.json` fixture, in millimetres.
fn preset_page() -> TabbedBoxParams {
    let mut p = TabbedBoxParams::new(METRIC);
    p.width = mm(120.0);
    p.depth = mm(90.5);
    p.height = mm(60.0);
    p.outside = true;
    p.thickness = mm(4.0);
    p.burn = mm(0.15);
    p.finger_width = "1.5".into();
    p.space_width = "2.5".into();
    p.surrounding_spaces = "1".into();
    p.play = mm(0.05);
    p.extra_length = mm(0.25);
    p.box_type = BoxType::NoBottom;
    p.dividers_x = "1".into();
    p.dividers_y = "2".into();
    p.divider_keying = KeyDividerType::None;
    p.optimize_layout = true;
    p.passes = "2".into();
    p.power = "800".into();
    p.feed_rate = "400".into();
    p.z_step_down = mm(0.2);
    p.offset_x = mm(12.0);
    p.offset_y = mm(8.0);
    p.home_before = true;
    p
}

#[test]
fn test_cam_tabbed_box_default_page_renders() {
    let h = open_tool(CamTool::TabbedBox);
    for label in [
        "X (Width):",
        "Y (Depth):",
        "H (Height):",
        "Outside Dims:",
        "Box Type:",
        "Dividers X:",
        "Divider Keying:",
        "Optimize Layout:",
        "Thickness:",
        "Burn / Tool Dia:",
        "Finger Width:",
        "Space Width:",
        "Surrounding Spaces:",
        "Play (fit tolerance):",
        "Extra Length:",
        "Passes:",
        "Power (S):",
        "Feed Rate:",
        "Z Step Down:",
        "Offset X:",
        "Home Device Before Start",
        "Boxes",
        "Generate",
        "Send to Editor",
        "Preview",
        "Generate Shapes",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label} not shown");
    }
    assert!(h.query_by_value("Full Box").is_some());
    assert!(h.query_by_value("Walls and Floor").is_some());
    assert!(pane(&h).tabbed_box_mut().parameters().is_ok());
}

#[test]
fn test_cam_tabbed_box_generate_matches_gtk() {
    let page = TabbedBoxParams::new(METRIC);
    assert_eq!(page.generate(), Ok(fixture("tabbed_box_default.gcode")));
    assert_eq!(custom().generate(), Ok(fixture("tabbed_box_custom.gcode")));
    let shapes = page.generate_shapes().expect("generates");
    assert_eq!(
        paths_text(&shapes),
        fixture("tabbed_box_shapes_default.txt")
    );
    assert!(shapes.paths.iter().all(|p| p.closed), "every panel closes");
    assert!(!shapes.group, "the panels are not grouped (GTK's were not)");
}

#[test]
fn test_cam_tabbed_box_generate_and_send_to_editor() {
    let mut h = open_tool_tall(CamTool::TabbedBox);
    click(&mut h, "Generate");
    assert_eq!(
        pane(&h).output(),
        Some(fixture("tabbed_box_default.gcode").as_str())
    );
    click(&mut h, "Send to Editor");
    assert_eq!(
        h.state().editor().borrow().buffer().text(),
        fixture("tabbed_box_default.gcode")
    );
}

#[test]
fn test_cam_tabbed_box_generate_shapes_adds_the_panels_to_the_design() {
    let mut h = open_tool_tall(CamTool::TabbedBox);
    let expected = TabbedBoxParams::new(METRIC)
        .generate_shapes()
        .expect("generates");
    click(&mut h, "Generate Shapes");
    assert!(h.state().workspace().is_drawn(PaneId::Designer));
    let designer = h.state().designer();
    let objects: Vec<_> = designer.borrow().state().canvas.shapes().cloned().collect();
    assert_eq!(objects.len(), expected.paths.len());
    assert!(objects.iter().all(|o| o.group_id.is_none()));
}

#[test]
fn test_cam_tabbed_box_invalid_input_shows_error_and_disables_the_actions() {
    let cases: [(Breaker, &str); 17] = [
        (|p| p.width = "10".into(), "Must be at least 20 mm"),
        (|p| p.depth = "19".into(), "Must be at least 20 mm"),
        (|p| p.height = "tall".into(), "Not a valid length"),
        (|p| p.thickness = "0".into(), "Must be between 1 and 20 mm"),
        (|p| p.thickness = "25".into(), "Must be between 1 and 20 mm"),
        (
            |p| {
                p.outside = true;
                p.width = "20".into();
                p.thickness = "10".into();
            },
            "Must be more than twice the thickness",
        ),
        (|p| p.burn = "-0.1".into(), "Must not be negative"),
        (|p| p.finger_width = "0".into(), "Must be greater than 0"),
        (
            |p| {
                p.finger_width = "0.04".into();
                p.space_width = "0.04".into();
            },
            "Finger plus space must be at least 0.1",
        ),
        (
            |p| p.surrounding_spaces = "-1".into(),
            "Must not be negative",
        ),
        (|p| p.play = "loose".into(), "Not a valid length"),
        (|p| p.dividers_x = "-1".into(), "Must not be negative"),
        (|p| p.passes = "0".into(), "Must be at least 1"),
        (|p| p.passes = "1.5".into(), "Not a whole number"),
        (|p| p.power = "-5".into(), "Must not be negative"),
        (|p| p.feed_rate = "0".into(), "Must be greater than 0"),
        (|p| p.z_step_down = "-1".into(), "Must not be negative"),
    ];
    // Control: with a valid page every button has an effect, so a click that has none below
    // is the page refusing it.
    let mut h = open_tool_tall(CamTool::TabbedBox);
    click(&mut h, "Generate");
    assert!(pane(&h).output().is_some(), "Generate does nothing");
    click(&mut h, "Send to Editor");
    assert!(!editor_text(&h).is_empty(), "Send to Editor does nothing");
    click(&mut h, "Generate Shapes");
    assert!(shape_count(&h) > 0, "Generate Shapes does nothing");
    for (break_it, message) in cases {
        let mut h = open_tool_tall(CamTool::TabbedBox);
        break_it(pane(&h).tabbed_box_mut());
        frames(&mut h, 2);
        assert!(h.query_by_label(message).is_some(), "{message} not shown");
        for button in ["Generate", "Send to Editor", "Preview", "Generate Shapes"] {
            click(&mut h, button);
            assert_eq!(
                pane(&h).output(),
                None,
                "{message}: {button} made a program"
            );
            assert_eq!(
                editor_text(&h),
                "",
                "{message}: {button} reached the editor"
            );
            assert_eq!(shape_count(&h), 0, "{message}: {button} made shapes");
        }
    }
}

/// The editor's program.
fn editor_text(h: &egui_kittest::Harness<'static, gcodekit5_ui_egui::GcodeKitApp>) -> String {
    h.state().editor().borrow().buffer().text().to_string()
}

/// The shapes in the design.
fn shape_count(h: &egui_kittest::Harness<'static, gcodekit5_ui_egui::GcodeKitApp>) -> usize {
    h.state()
        .designer()
        .borrow()
        .state()
        .canvas
        .shapes()
        .count()
}

#[test]
fn test_cam_tabbed_box_follows_the_measurement_system() {
    let mut page = TabbedBoxParams::new(METRIC);
    page.set_units(MeasurementSystem::Imperial);
    assert_eq!(page.width, "3.937");
    assert_eq!(page.finger_width, "2", "multiples of the thickness stay");
    assert_eq!(page.passes, "3");
    // Inches are shown to 3 places, so the box is the same to a few microns.
    let shown = page.parameters().expect("valid");
    let kept = TabbedBoxParams::new(METRIC).parameters().expect("valid");
    assert!((shown.x - kept.x).abs() < 0.01 && (shown.thickness - kept.thickness).abs() < 0.01);
}

/// The preset GTK wrote loads, and egui writes the same text for the same input.
#[test]
fn test_cam_tabbed_box_preset_loads_the_gtk_file_and_round_trips() {
    let tr = Translator::new("en");
    let json = fixture("preset_tabbed_box.json");
    let mut page = TabbedBoxParams::new(METRIC);
    page.apply_preset(&json).expect("loads");
    assert_eq!(page, preset_page());
    assert_eq!(page.preset_json(&tr).as_ref(), Ok(&json));

    let mut broken = TabbedBoxParams::new(METRIC);
    let before = broken.clone();
    assert!(broken.apply_preset("not json").is_err());
    assert!(broken.apply_preset(&fixture("preset_jigsaw.json")).is_err());
    assert_eq!(broken, before);

    let mut unreadable = TabbedBoxParams::new(METRIC);
    unreadable.thickness = "thick".into();
    assert_eq!(
        unreadable.preset_json(&tr),
        Err("Thickness: Not a valid length".to_string())
    );
}
