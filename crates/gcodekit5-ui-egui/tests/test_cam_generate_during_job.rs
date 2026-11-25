//! The Jigsaw, Tabbed Box and Gerber pages generate on the UI thread, so while a job streams
//! their Generate, Send to Editor, Preview and Generate Shapes are refused with a notice on
//! the page (the UI must stay free for Stop and Feed Hold); nothing is generated or sent.
//! The jigsaw's size also has an upper bound.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_app::test_support::LoopbackShared;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::panes::cam_tools::CamTool;
use gcodekit5_ui_egui::panes::cam_tools::jigsaw::JigsawParams;

use crate::machine_support::{loopback_state, start_held_stream};
use crate::test_cam_drill_press::{click, pane};
use crate::test_cam_tools::cam_app;
use crate::test_designer_files_app::{Shared, answer, frames, path};

/// The page's refusal while a job streams.
const REFUSED: &str = "Stop the running job before generating.";

/// The app with `tool`'s page open, tall enough to show it whole, while a job streams.
fn open_during_job(tool: CamTool) -> (Harness<'static, GcodeKitApp>, Shared, Arc<LoopbackShared>) {
    let (mut state, shared) = loopback_state(Duration::from_millis(1));
    start_held_stream(&mut state, &shared, 100);
    let (mut h, script) = cam_app(state);
    assert!(h.state().state().job_running());
    let title = h
        .state()
        .state()
        .translator
        .tr(tool.title_msgid())
        .into_owned();
    h.get_by_label(&title).click();
    frames(&mut h, 1);
    h.get_by_label("Open").click();
    frames(&mut h, 2);
    assert_eq!(h.state().cam_tools().borrow().open_tool(), Some(tool));
    h.set_size(egui::vec2(1280.0, 2400.0));
    frames(&mut h, 3);
    (h, script, shared)
}

fn shape_count(h: &Harness<'static, GcodeKitApp>) -> usize {
    h.state()
        .designer()
        .borrow()
        .state()
        .canvas
        .shapes()
        .count()
}

fn editor_text(h: &Harness<'static, GcodeKitApp>) -> String {
    h.state().editor().borrow().buffer().text().to_string()
}

/// Clicks `label` and checks it was refused: the notice says so and nothing was made.
fn refused(h: &mut Harness<'static, GcodeKitApp>, label: &str) {
    let (shapes, text) = (shape_count(h), editor_text(h));
    pane(h).set_notice(String::new());
    click(h, label);
    let mut cam = pane(h);
    assert_eq!(cam.notice(), Some(REFUSED), "{label} not refused");
    assert_eq!(cam.output(), None, "{label} generated a program");
    assert_eq!(cam.take_editor_gcode(), None, "{label} sent a program");
    drop(cam);
    assert_eq!(shape_count(h), shapes, "{label} added shapes");
    assert_eq!(
        editor_text(h),
        text,
        "{label} replaced the editor's program"
    );
}

#[test]
fn test_cam_jigsaw_generate_is_refused_while_a_job_runs() {
    let (mut h, _script, _shared) = open_during_job(CamTool::Jigsaw);
    refused(&mut h, "Generate");
}

#[test]
fn test_cam_tabbed_box_actions_are_refused_while_a_job_runs() {
    let (mut h, _script, _shared) = open_during_job(CamTool::TabbedBox);
    for label in ["Generate", "Send to Editor", "Preview", "Generate Shapes"] {
        refused(&mut h, label);
    }
}

#[test]
fn test_cam_gerber_actions_are_refused_while_a_job_runs() {
    let (mut h, script, _shared) = open_during_job(CamTool::Gerber);
    // Browsing is cheap and still allowed.
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cam/gerber");
    answer(&script, [path(&project)]);
    click(&mut h, "Browse...");
    assert!(
        h.query_by_label("board.gtl").is_some(),
        "the folder was read"
    );
    for label in ["Generate", "Send to Editor", "Preview"] {
        refused(&mut h, label);
    }
}

/// The jigsaw's width and height are bounded above, like the piece count (20 pieces of at
/// most 100 mm).
#[test]
fn test_cam_jigsaw_size_has_an_upper_bound() {
    let mut p = JigsawParams::new(MeasurementSystem::Metric);
    p.width = "2000".into();
    p.height = "2000".into();
    assert!(p.parameters().is_ok(), "2000 mm is allowed");
    p.width = "2000.5".into();
    p.height = "5000".into();
    let errors = p.parameters().err().unwrap_or_default();
    let messages: Vec<_> = errors.iter().map(|e| e.message).collect();
    assert_eq!(
        messages,
        ["Must be at most 2000 mm", "Must be at most 2000 mm"],
        "{errors:?}"
    );
    // Imperial: 79 in is about 2007 mm.
    let mut p = JigsawParams::new(MeasurementSystem::Imperial);
    p.width = "79".into();
    assert!(p.parameters().is_err());
}
