//! The Jigsaw Puzzle Generator page: GTK's defaults render, invalid input is shown at the
//! field and disables Generate, Generate adds the puzzle's outlines to the design exactly as
//! the GTK page made them (fixtures from `capture_cam_fixtures`), and presets in GTK's file
//! format load and save.
//!
//! The fixtures in `tests/fixtures/cam/` are frozen references: `capture_cam_fixtures`, which
//! wrote them from the GTK pages, was in the GTK crate deleted in `26740e3` (its source is at
//! `6841d84`). Change a fixture only deliberately, by hand, in the commit that changes the
//! generator's output and says why.

use egui_kittest::kittest::Queryable;
use gcodekit6_app::i18n::Translator;
use gcodekit6_core::units::MeasurementSystem;
use gcodekit6_ui_egui::panes::cam_tools::CamTool;
use gcodekit6_ui_egui::panes::cam_tools::common::format_len;
use gcodekit6_ui_egui::panes::cam_tools::jigsaw::JigsawParams;
use gcodekit6_ui_egui::panes::cam_tools::requests::DesignerPaths;
use gcodekit6_ui_egui::workspace::PaneId;

use crate::test_cam_drill_press::{click, fixture, open_tool, pane};
use crate::test_designer_files_app::frames;

const METRIC: MeasurementSystem = MeasurementSystem::Metric;

/// Makes one field of the page unsafe.
type Breaker = fn(&mut JigsawParams);

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

/// The page with every field changed from GTK's defaults (the `jigsaw_custom` case).
fn custom() -> JigsawParams {
    let mut p = JigsawParams::new(METRIC);
    p.width = mm(300.0);
    p.height = mm(200.0);
    p.pieces_across = "5".into();
    p.pieces_down = "4".into();
    p.seed = "7".into();
    p.tab_size = "25".into();
    p.jitter = "8".into();
    p.corner_radius = mm(3.0);
    p
}

#[test]
fn test_cam_jigsaw_default_page_renders() {
    let h = open_tool(CamTool::Jigsaw);
    for label in [
        "Width:",
        "Height:",
        "Corner Radius:",
        "Pieces Across:",
        "Pieces Down:",
        "Tab Size (%):",
        "Jitter (%):",
        "Random Seed:",
        "Randomize",
        "Generate",
        "Load",
        "Save",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label} not shown");
    }
    assert!(pane(&h).jigsaw_mut().parameters().is_ok());
}

#[test]
fn test_cam_jigsaw_generate_matches_gtk() {
    let page = JigsawParams::new(METRIC);
    let made = page.generate().expect("generates");
    assert_eq!(paths_text(&made), fixture("jigsaw_default.txt"));
    assert!(made.group, "the puzzle is grouped");
    let made = custom().generate().expect("generates");
    assert_eq!(paths_text(&made), fixture("jigsaw_custom.txt"));
}

/// Generate adds the puzzle to the design as one undo step, in one group with the border
/// closed, and brings the designer to the front.
#[test]
fn test_cam_jigsaw_generate_adds_the_puzzle_to_the_design() {
    let mut h = open_tool(CamTool::Jigsaw);
    let expected = JigsawParams::new(METRIC).generate().expect("generates");
    click(&mut h, "Generate");
    assert!(h.state().workspace().is_drawn(PaneId::Designer));
    let designer = h.state().designer();
    let objects: Vec<_> = designer.borrow().state().canvas.shapes().cloned().collect();
    assert_eq!(objects.len(), expected.paths.len());
    let group = objects[0].group_id;
    assert!(group.is_some(), "the shapes are grouped");
    assert!(objects.iter().all(|o| o.group_id == group));
    designer.borrow_mut().undo();
    assert_eq!(designer.borrow().state().canvas.shapes().count(), 0);
    assert_eq!(pane(&h).notice(), None);
}

#[test]
fn test_cam_jigsaw_invalid_input_shows_error_and_disables_generate() {
    let cases: [(Breaker, &str); 11] = [
        (|p| p.width = "40".into(), "Must be at least 50 mm"),
        (|p| p.height = "wide".into(), "Not a valid length"),
        (
            |p| p.pieces_across = "1".into(),
            "Must be a whole number from 2 to 20",
        ),
        (
            |p| p.pieces_across = "21".into(),
            "Must be a whole number from 2 to 20",
        ),
        (|p| p.pieces_down = "2.5".into(), "Not a whole number"),
        (
            |p| {
                p.width = "100".into();
                p.pieces_across = "10".into();
            },
            "Pieces would be narrower than 15 mm",
        ),
        (
            |p| {
                p.height = "100".into();
                p.pieces_down = "7".into();
            },
            "Pieces would be shorter than 15 mm",
        ),
        (|p| p.tab_size = "5".into(), "Must be between 10 and 30"),
        (|p| p.jitter = "-1".into(), "Must be between 0 and 13"),
        (
            |p| p.corner_radius = "-1".into(),
            "Must be between 0 and 10 mm",
        ),
        (
            |p| p.seed = "-3".into(),
            "Must be a whole number from 0 to 4294967295",
        ),
    ];
    for (break_it, message) in cases {
        let mut h = open_tool(CamTool::Jigsaw);
        break_it(pane(&h).jigsaw_mut());
        frames(&mut h, 2);
        assert!(h.query_by_label(message).is_some(), "{message} not shown");
        click(&mut h, "Generate");
        assert_eq!(
            h.state()
                .designer()
                .borrow()
                .state()
                .canvas
                .shapes()
                .count(),
            0,
            "{message}: Generate made shapes"
        );
    }
}

#[test]
fn test_cam_jigsaw_follows_the_measurement_system() {
    let mut page = JigsawParams::new(METRIC);
    page.set_units(MeasurementSystem::Imperial);
    assert_eq!(page.width, "7.874");
    assert_eq!(page.pieces_across, "4", "counts are not lengths");
    let made = page.generate().expect("generates");
    let metric = JigsawParams::new(METRIC).generate().expect("generates");
    assert_eq!(made.paths.len(), metric.paths.len());
}

#[test]
fn test_cam_jigsaw_randomize_gives_a_valid_seed() {
    let mut h = open_tool(CamTool::Jigsaw);
    click(&mut h, "Randomize");
    let seed = pane(&h).jigsaw_mut().seed.clone();
    assert!(
        seed.parse::<u32>().is_ok_and(|s| s < 100_000),
        "seed {seed:?}"
    );
    assert!(pane(&h).jigsaw_mut().parameters().is_ok());
}

/// The preset GTK wrote loads, and egui writes the same text for the same input.
#[test]
fn test_cam_jigsaw_preset_loads_the_gtk_file_and_round_trips() {
    let tr = Translator::new("en");
    let json = fixture("preset_jigsaw.json");
    let mut page = JigsawParams::new(METRIC);
    page.apply_preset(&json).expect("loads");
    assert_eq!(page, custom());
    assert_eq!(page.preset_json(&tr).as_ref(), Ok(&json));

    let mut broken = JigsawParams::new(METRIC);
    let before = broken.clone();
    assert!(broken.apply_preset("{\"width\": 5").is_err());
    assert!(
        broken
            .apply_preset(&fixture("preset_spoilboard_grid.json"))
            .is_err()
    );
    assert_eq!(broken, before);

    let mut unreadable = JigsawParams::new(METRIC);
    unreadable.width = "wide".into();
    assert_eq!(
        unreadable.preset_json(&tr),
        Err("Width: Not a valid length".to_string())
    );
}
