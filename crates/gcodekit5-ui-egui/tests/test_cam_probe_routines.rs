//! The Probe Tools page: GTK's defaults render, each routine shows its own settings, invalid
//! input is shown at the field and disables the actions, Generate makes exactly the program
//! the GTK page made (fixtures from `capture_cam_fixtures`), a run streams exactly the GTK
//! run's lines, and presets in GTK's file format load and save.
//!
//! Running a routine on the machine is covered by `test_cam_probe_run`.
//!
//! The fixtures in `tests/fixtures/cam/` are frozen references: `capture_cam_fixtures`, which
//! wrote them from the GTK pages, was in the GTK crate deleted in `26740e3` (its source is at
//! `6841d84`). Change a fixture only deliberately, by hand, in the commit that changes the
//! generator's output and says why.

use egui_kittest::kittest::Queryable;
use gcodekit5_camtools::comment_processor::strip_comments;
use gcodekit5_core::{Corner, Position, ProbeAxis, ProbeDirection};
use gcodekit5_ui_egui::panes::cam_tools::probe_routines::{ProbeParams, RoutineKind};
use gcodekit5_ui_egui::panes::cam_tools::{CamTool, Category};

use crate::test_cam_drill_press::{click, fixture, open_tool_tall, pane};
use crate::test_cam_tools::cam_app;
use crate::test_designer_files_app::{default_state, frames};

/// The start position the fixtures were generated from.
pub fn start() -> Position {
    Position::new(12.5, -30.25, -4.0)
}

/// The page of each fixture case (`probe_<name>.gcode`).
pub fn cases() -> Vec<(&'static str, ProbeParams)> {
    let routine = |kind| ProbeParams {
        routine: kind,
        ..ProbeParams::default()
    };
    let mut z_wcs = routine(RoutineKind::ZTouch);
    z_wcs.wcs = 55;
    z_wcs.auto_update = true;
    z_wcs.plate_thickness = "1.5".into();
    z_wcs.backoff = "1.25".into();
    let mut edge_wcs = routine(RoutineKind::EdgeFind);
    edge_wcs.wcs = 56;
    edge_wcs.auto_update = true;
    edge_wcs.edge_axis = Some(ProbeAxis::Y);
    edge_wcs.edge_direction = Some(ProbeDirection::Positive);
    edge_wcs.edge_distance = "15".into();
    edge_wcs.tip_radius = "1".into();
    edge_wcs.probe_depth = "4".into();
    let mut corner_wcs = routine(RoutineKind::CornerFind);
    corner_wcs.auto_update = true;
    corner_wcs.corner = Some(Corner::XmaxYmax);
    corner_wcs.tip_radius = "0.5".into();
    vec![
        ("z_touch", routine(RoutineKind::ZTouch)),
        ("edge_find", routine(RoutineKind::EdgeFind)),
        ("corner_find", routine(RoutineKind::CornerFind)),
        ("bore_center", routine(RoutineKind::BoreCenter)),
        ("boss_center", routine(RoutineKind::BossCenter)),
        ("tool_length", routine(RoutineKind::ToolLength)),
        ("z_touch_wcs", z_wcs),
        ("edge_find_wcs", edge_wcs),
        ("corner_find_wcs", corner_wcs),
    ]
}

/// The lines GTK's run streamed for a generated program (`probe_run_program`).
pub fn gtk_run_lines(gcode: &str) -> Vec<String> {
    gcode
        .lines()
        .map(strip_comments)
        .filter(|l| !l.is_empty())
        .collect()
}

#[test]
fn test_cam_probe_default_page_renders() {
    let h = open_tool_tall(CamTool::ProbeRoutines);
    for label in [
        "Routine",
        "Safe Height:",
        "Fast Feed Rate:",
        "Slow Feed Rate:",
        "Backoff Distance:",
        "Max Probe Depth",
        "Target WCS",
        "Auto-Update WCS",
        "Plate Thickness:",
        "Generate",
        "Send to Editor",
        "Preview",
        "Probe",
        "Device connection required to use Probe Tools",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label} not shown");
    }
    // Z-touch shows only the plate thickness of the routine settings.
    for label in ["Probe Axis", "Corner", "Diameter:", "Setter Plate X"] {
        assert!(
            h.query_by_label(label).is_none(),
            "{label} shown for Z-touch"
        );
    }
    let mut cam = pane(&h);
    let p = cam.probe_mut();
    assert_eq!(*p, ProbeParams::default());
    assert_eq!(p.safe_height, "5.000");
    assert_eq!(p.fast_feed, "100.00");
    assert_eq!(p.max_depth, "20.000");
    assert_eq!(p.edge_distance, "10.000");
    assert_eq!(p.setter_z, "-50.000");
    assert_eq!(p.wcs, 54);
    assert!(p.parameters().is_ok());
}

#[test]
fn test_cam_probe_routine_shows_its_settings() {
    let mut h = open_tool_tall(CamTool::ProbeRoutines);
    let shown: [(RoutineKind, &[&str]); 5] = [
        (
            RoutineKind::EdgeFind,
            &[
                "Probe Axis",
                "Probe Direction",
                "Probe Distance",
                "Probe Depth:",
                "Probe Tip Radius:",
                "Plate Thickness:",
            ],
        ),
        (
            RoutineKind::CornerFind,
            &[
                "Corner",
                "Probe Distance",
                "Probe Depth:",
                "Probe Tip Radius:",
            ],
        ),
        (
            RoutineKind::BoreCenter,
            &["Diameter:", "Probe Depth:", "Probe Tip Radius:"],
        ),
        (
            RoutineKind::BossCenter,
            &["Diameter:", "Probe Depth:", "Probe Tip Radius:"],
        ),
        (
            RoutineKind::ToolLength,
            &["Setter Plate X", "Setter Plate Y", "Setter Plate Z"],
        ),
    ];
    for (kind, labels) in shown {
        pane(&h).probe_mut().routine = kind;
        frames(&mut h, 2);
        for label in labels {
            assert!(h.query_by_label(label).is_some(), "{kind:?}: {label}");
        }
    }
}

#[test]
fn test_cam_probe_is_listed_under_maintenance_last() {
    let (h, _script) = cam_app(default_state());
    let tr = h.state().state().translator;
    let mut cam = pane(&h);
    cam.set_category(Category::All);
    assert_eq!(cam.listed_tools(&tr).last(), Some(&CamTool::ProbeRoutines));
    cam.set_category(Category::Maintenance);
    assert!(cam.listed_tools(&tr).contains(&CamTool::ProbeRoutines));
}

#[test]
fn test_cam_probe_generate_matches_gtk() {
    for (name, p) in cases() {
        let made = p.generate(start());
        assert_eq!(
            made.as_ref().map(|o| o.gcode.clone()),
            Ok(fixture(&format!("probe_{name}.gcode"))),
            "{name}"
        );
    }
}

#[test]
fn test_cam_probe_run_lines_match_gtk() {
    for (name, p) in cases() {
        let gtk = gtk_run_lines(&fixture(&format!("probe_{name}.gcode")));
        let lines = p.run_lines(start()).expect("a program");
        assert_eq!(lines, gtk, "{name}");
    }
}

#[test]
fn test_cam_probe_generate_click_shows_the_program() {
    let mut h = open_tool_tall(CamTool::ProbeRoutines);
    click(&mut h, "Generate");
    let output = pane(&h).output().map(str::to_string).expect("a program");
    // Not connected: the header records the origin.
    assert!(output.contains("; Start (informational): X0.000 Y0.000 Z0.000"));
    assert!(output.contains("G38.2 Z-20.000 F100.0"));
}

#[test]
fn test_cam_probe_invalid_input_shows_error_and_disables_the_actions() {
    type Breaker = fn(&mut ProbeParams);
    let cases: [(Breaker, &str); 11] = [
        (|p| p.safe_height = "0".into(), "Must be greater than 0"),
        (|p| p.fast_feed = "0".into(), "Must be greater than 0"),
        (|p| p.slow_feed = "-1".into(), "Must be greater than 0"),
        (|p| p.backoff = "-1".into(), "Must not be negative"),
        (|p| p.max_depth = "0".into(), "Must be greater than 0"),
        (|p| p.plate_thickness = "-1".into(), "Must not be negative"),
        (|p| p.safe_height = "high".into(), "Not a valid length"),
        (
            |p| {
                p.routine = RoutineKind::EdgeFind;
                p.edge_distance = "0".into();
            },
            "Must be greater than 0",
        ),
        (
            |p| {
                p.routine = RoutineKind::BoreCenter;
                p.diameter = "0".into();
            },
            "Must be greater than 0",
        ),
        (
            |p| {
                p.routine = RoutineKind::CornerFind;
                p.probe_depth = "-1".into();
            },
            "Must not be negative",
        ),
        (
            |p| {
                p.routine = RoutineKind::BossCenter;
                p.tip_radius = "-0.5".into();
            },
            "Must not be negative",
        ),
    ];
    for (break_it, message) in cases {
        let mut h = open_tool_tall(CamTool::ProbeRoutines);
        break_it(pane(&h).probe_mut());
        frames(&mut h, 2);
        assert!(h.query_by_label(message).is_some(), "{message} not shown");
        for button in ["Generate", "Send to Editor", "Preview"] {
            click(&mut h, button);
            assert_eq!(
                pane(&h).output(),
                None,
                "{message}: {button} made a program"
            );
        }
        assert!(
            pane(&h).probe_mut().parameters().is_err(),
            "{message}: still valid"
        );
    }
}

#[test]
fn test_cam_probe_preset_loads_the_gtk_file_and_round_trips() {
    let gtk = fixture("preset_probe.json");
    let mut p = ProbeParams::default();
    assert_eq!(p.apply_preset(&gtk), Ok(()));
    let edge = cases()
        .into_iter()
        .find(|(name, _)| *name == "edge_find_wcs")
        .map(|(_, p)| p)
        .expect("the edge case");
    // GTK writes three decimals when it loads a preset.
    assert_eq!(p.routine, RoutineKind::EdgeFind);
    assert_eq!(p.edge_axis, edge.edge_axis);
    assert_eq!(p.edge_direction, edge.edge_direction);
    assert_eq!(p.edge_distance, "15.000");
    assert_eq!(p.tip_radius, "1.000");
    assert_eq!(p.wcs, 56);
    assert!(p.auto_update);
    let tr = gcodekit5_app::i18n::Translator::new("en");
    assert_eq!(p.preset_json(&tr), Ok(gtk.clone()));
    assert_eq!(edge.preset_json(&tr), Ok(gtk));
    let before = p.clone();
    assert!(p.apply_preset("[1, 2]").is_err());
    assert!(p.apply_preset("not json").is_err());
    assert_eq!(p, before);
}
