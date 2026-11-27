//! The visualizer's run preview (GTK's play, pause, stop and speed): the preview path, the
//! playback steps, the bar's buttons, File > Run, the preview marker, and that it sends
//! nothing to the machine.

use std::sync::Arc;
use std::time::Duration;

use egui_kittest::kittest::Queryable;
use gcodekit6_app::test_support::Wrote;
use gcodekit6_ui_egui::GcodeKitApp;
use gcodekit6_ui_egui::commands::Command;
use gcodekit6_ui_egui::panes::visualizer::pipeline::PREVIEW_MARKER_COLOR;
use gcodekit6_ui_egui::panes::visualizer::preview::{MAX_POINTS, PathBuilder, TICK};
use gcodekit6_ui_egui::panes::visualizer::{
    self, Display, PreviewSpeed, PreviewState, RunPreview, StdView, VisualizerPane, toolpath,
};
use gcodekit6_ui_egui::workspace::{Page, Pane, PaneContext};
use glam::Vec3;

use crate::test_visualizer::{gpu_harness, load, pane_harness, pixel_near, sample};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn test_preview_path_spacing_as_gtk() {
    let mut path = PathBuilder::default();
    path.line(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), 2);
    assert_eq!(path.clone().finish().len(), 4, "10 mm in 3 mm steps");
    let mut short = PathBuilder::default();
    short.line(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), 2);
    assert_eq!(short.finish(), vec![Vec3::new(0.5, 0.0, 0.0), Vec3::X]);
    let mut long = PathBuilder::default();
    long.line(Vec3::ZERO, Vec3::new(1000.0, 0.0, 0.0), 2);
    assert_eq!(long.finish().len(), 24, "at most 24 per move");
    let mut dwell = PathBuilder::default();
    dwell.dwell(Vec3::ONE, 2.0);
    assert_eq!(dwell.finish(), vec![Vec3::ONE; 16], "8 points a second");
    let mut capped = PathBuilder::default();
    for _ in 0..2000 {
        capped.line(Vec3::ZERO, Vec3::new(100.0, 0.0, 0.0), 2);
    }
    assert_eq!(capped.finish().len(), MAX_POINTS);
}

#[test]
fn test_toolpath_build_makes_the_preview_path() {
    let mesh = toolpath::build(&sample());
    let first = mesh.preview.first().copied();
    let last = mesh.preview.last().copied();
    // The first move is G0 Z5 from the origin; the program ends at (0, 0, 5).
    assert_eq!(first.map(|p| p.x), Some(0.0));
    assert_eq!(last, Some(Vec3::new(0.0, 0.0, 5.0)));
    // The arc's points lie on it (radius 20 around (20, 30)).
    let on_arc = mesh
        .preview
        .iter()
        .filter(|p| p.y > 30.5 && p.x > 20.5)
        .all(|p| ((p.x - 20.0).hypot(p.y - 30.0) - 20.0).abs() < 0.2);
    assert!(on_arc, "the preview follows the arc");
}

/// A preview over the points 0..n along X.
fn preview(n: usize) -> RunPreview {
    let mut preview = RunPreview::default();
    preview.set_points((0..n).map(|i| Vec3::new(i as f32, 0.0, 0.0)).collect());
    preview
}

#[test]
fn test_preview_steps_by_speed_per_tick() {
    let mut p = preview(100);
    assert_eq!(p.position(), None, "stopped: no marker");
    assert!(p.play());
    assert_eq!(p.position(), Some(Vec3::ZERO));
    p.advance(TICK);
    assert_eq!(
        p.position().map(|v| v.x),
        Some(2.0),
        "Normal: 2 points a tick"
    );
    p.advance(TICK / 2);
    assert_eq!(
        p.position().map(|v| v.x),
        Some(2.0),
        "half a tick carries over"
    );
    p.advance(TICK / 2);
    assert_eq!(p.position().map(|v| v.x), Some(4.0));
    for (speed, step) in [
        (PreviewSpeed::Slow, 1.0),
        (PreviewSpeed::Fast, 5.0),
        (PreviewSpeed::Turbo, 20.0),
    ] {
        p.set_speed(speed);
        let before = p.position().map_or(0.0, |v| v.x);
        p.advance(TICK);
        assert_eq!(p.position().map(|v| v.x), Some(before + step), "{speed:?}");
    }
    p.pause();
    p.advance(TICK * 10);
    assert_eq!(p.state(), PreviewState::Paused);
    assert_eq!(p.position().map(|v| v.x), Some(30.0), "paused holds");
    assert!(p.play(), "resumes");
    assert_eq!(
        p.position().map(|v| v.x),
        Some(30.0),
        "from where it paused"
    );
    p.set_speed(PreviewSpeed::Turbo);
    p.advance(TICK * 10);
    assert_eq!(p.state(), PreviewState::Finished);
    assert_eq!(
        p.position(),
        Some(Vec3::new(99.0, 0.0, 0.0)),
        "ends on the last point"
    );
    assert!(p.play(), "Play after the end starts again");
    assert_eq!(p.position(), Some(Vec3::ZERO));
    p.stop();
    assert_eq!((p.state(), p.position()), (PreviewState::Stopped, None));
    assert_eq!(PreviewSpeed::default(), PreviewSpeed::Normal);
    assert!(!RunPreview::default().play(), "nothing to play");
}

/// The pane beside its sidebar with a 480-point view, room for the whole bar.
fn wide_harness() -> egui_kittest::Harness<'static, VisualizerPane> {
    let tr = gcodekit6_app::i18n::Translator::new("en");
    egui_kittest::Harness::builder()
        .with_size(egui::vec2(730.0, 360.0))
        .build_ui_state(
            move |ui, pane: &mut VisualizerPane| {
                let mut requested = Vec::new();
                let mut ctx = PaneContext::new(&tr, &mut requested);
                pane.ui(ui, &mut ctx);
            },
            VisualizerPane::new(),
        )
}

#[test]
fn test_preview_bar_buttons_drive_the_preview() -> TestResult {
    let mut harness = wide_harness();
    load(&mut harness, sample())?;
    harness.get_by_label("Play preview").click();
    harness.run();
    assert_eq!(harness.state().preview().state(), PreviewState::Playing);
    harness.get_by_label("Pause preview").click();
    harness.run();
    assert_eq!(harness.state().preview().state(), PreviewState::Paused);
    harness
        .get_by_role_and_label(egui::accesskit::Role::ComboBox, "Speed")
        .click();
    harness.run();
    harness.get_by_label("Turbo").click();
    harness.run();
    assert_eq!(harness.state().preview().speed(), PreviewSpeed::Turbo);
    harness.get_by_label("Stop preview").click();
    harness.run();
    assert_eq!(harness.state().preview().state(), PreviewState::Stopped);
    assert_eq!(harness.state().preview().position(), None);
    Ok(())
}

#[test]
fn test_preview_plays_in_frames_and_asks_nothing() -> TestResult {
    let tr = gcodekit6_app::i18n::Translator::new("en");
    let requested = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = Arc::clone(&requested);
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(730.0, 360.0))
        .build_ui_state(
            move |ui, pane: &mut VisualizerPane| {
                let mut asked = Vec::new();
                let mut ctx = PaneContext::new(&tr, &mut asked);
                pane.ui(ui, &mut ctx);
                if let Ok(mut all) = seen.lock() {
                    all.extend(asked);
                }
            },
            VisualizerPane::new(),
        );
    load(&mut harness, sample())?;
    harness.state_mut().preview_mut().play();
    let start = harness.state().preview().position();
    for _ in 0..20 {
        harness.step();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_ne!(
        harness.state().preview().position(),
        start,
        "the marker moved"
    );
    let asked = requested.lock().map(|r| r.len()).unwrap_or(usize::MAX);
    assert_eq!(asked, 0, "the preview asks the app for no command");
    Ok(())
}

#[test]
fn test_new_program_stops_the_preview() -> TestResult {
    let mut harness = pane_harness(VisualizerPane::new());
    load(&mut harness, sample())?;
    harness.state_mut().preview_mut().play();
    let other: Arc<[String]> = ["G0 X5"].iter().map(|s| s.to_string()).collect();
    load(&mut harness, other)?;
    assert_eq!(harness.state().preview().state(), PreviewState::Stopped);
    assert!(
        !harness.state().preview().is_empty(),
        "the new program's path"
    );
    Ok(())
}

#[test]
fn test_file_run_without_motion_says_why() -> TestResult {
    let tr = gcodekit6_app::i18n::Translator::new("en");
    let mut requested = Vec::new();
    let mut pane = VisualizerPane::new();
    {
        let mut ctx = PaneContext::new(&tr, &mut requested);
        assert!(pane.command(Command::FileRun, &mut ctx));
    }
    assert_eq!(pane.preview_notice().map(|n| n.0), Some("No G-code to run"));
    let mut harness = pane_harness(pane);
    let still: Arc<[String]> = ["G21", "G90"].iter().map(|s| s.to_string()).collect();
    load(&mut harness, still)?;
    {
        let mut ctx = PaneContext::new(&tr, &mut requested);
        harness.state_mut().command(Command::FileRun, &mut ctx);
    }
    harness.run();
    assert_eq!(
        harness.state().preview_notice().map(|n| n.0),
        Some("No motion commands found")
    );
    assert!(requested.is_empty());
    Ok(())
}

#[test]
fn test_file_run_waits_for_the_build() {
    let tr = gcodekit6_app::i18n::Translator::new("en");
    let mut harness = pane_harness(VisualizerPane::new());
    let ctx = harness.ctx.clone();
    harness.state_mut().load_program(Some(sample()), &ctx);
    let mut requested = Vec::new();
    {
        let mut pane_ctx = PaneContext::new(&tr, &mut requested);
        harness.state_mut().command(Command::FileRun, &mut pane_ctx);
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while harness.state().is_building() && std::time::Instant::now() < deadline {
        harness.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    harness.run();
    assert_eq!(harness.state().preview().state(), PreviewState::Playing);
    assert_eq!(harness.state().preview_notice(), None);
}

#[test]
fn test_file_run_in_the_app_shows_the_visualizer_and_sends_nothing() {
    let (mut state, shared) = crate::machine_support::loopback_state(Duration::ZERO);
    crate::machine_support::connect(&mut state);
    state
        .machine
        .set_program(&sample().join("\n"))
        .expect("program");
    let mut harness = crate::machine_support::app_harness(GcodeKitApp::from_state(state));
    harness.run_steps(2);
    let lines = || {
        shared
            .log
            .lock()
            .map(|log| log.iter().filter(|w| matches!(w, Wrote::Line(_))).count())
            .unwrap_or(usize::MAX)
    };
    let before = lines();
    harness.state_mut().dispatch(Command::FileRun);
    for _ in 0..30 {
        harness.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(harness.state().page(), Page::Visualizer);
    assert!(harness.query_by_label("Pause preview").is_some());
    assert_eq!(lines(), before, "no line written to the machine");
    assert!(!harness.state().state().job_running(), "no job");
}

/// Cyan of the preview marker over the dark view, within blending.
fn is_preview(p: [u8; 4]) -> bool {
    p[0] < 60 && p[1] > 120 && p[2] > 150
}

/// Whether the preview marker is drawn at its position, with `display`.
fn marker_drawn(display: Display) -> Result<bool, Box<dyn std::error::Error>> {
    let mut pane = VisualizerPane::new();
    pane.set_display(display);
    let mut harness = gpu_harness(pane);
    load(&mut harness, sample())?;
    harness
        .state_mut()
        .apply(visualizer::NavAction::View(StdView::Top));
    harness.state_mut().preview_mut().play();
    harness.state_mut().preview_mut().pause();
    harness.run();
    let at = harness.state().preview().position().ok_or("no position")?;
    let rendered = harness.render()?;
    let image = |x: u32, y: u32| rendered.get_pixel_checked(x, y).map(|p| p.0);
    Ok(pixel_near(&image, harness.state(), at, is_preview))
}

#[test]
fn test_preview_marker_drawn_unless_hidden() -> TestResult {
    assert_eq!(PREVIEW_MARKER_COLOR, [0, 217, 255, 140]);
    assert!(
        marker_drawn(Display::default())?,
        "cyan marker at the start"
    );
    let hidden = Display {
        marker: false,
        ..Display::default()
    };
    assert!(
        !marker_drawn(hidden)?,
        "Show Laser/Spindle off hides it, as in GTK"
    );
    Ok(())
}

/// The pane drawn with the app's machine, as the app draws it.
struct WithMachine {
    pane: VisualizerPane,
    machine: gcodekit6_ui_egui::machine::Machine,
}

#[test]
fn test_file_run_right_after_an_edit_previews_the_edited_program() -> TestResult {
    let state = gcodekit6_ui_egui::AppState::new(
        gcodekit6_settings::SettingsManager::detached(gcodekit6_settings::config::Config::default()),
        crate::test_smoke::native_decision(),
    );
    let tr = gcodekit6_app::i18n::Translator::new("en");
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(730.0, 360.0))
        .build_ui_state(
            move |ui, s: &mut WithMachine| {
                let mut requested = Vec::new();
                let mut ctx = PaneContext::new(&tr, &mut requested).with_machine(&s.machine);
                s.pane.ui(ui, &mut ctx);
            },
            WithMachine {
                pane: VisualizerPane::new(),
                machine: state.machine,
            },
        );
    harness.state_mut().machine.set_program("G21\nG0 X5\n")?;
    harness.run();
    // The edited program reaches the machine and Run is pressed in the same frame, before
    // the pane has seen it (the app syncs the editor, then routes Run).
    harness
        .state_mut()
        .machine
        .set_program("G21\nG0 X40 Y30\n")?;
    {
        let tr = gcodekit6_app::i18n::Translator::new("en");
        let mut requested = Vec::new();
        let s = harness.state_mut();
        let mut ctx = PaneContext::new(&tr, &mut requested).with_machine(&s.machine);
        assert!(s.pane.command(Command::FileRun, &mut ctx));
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while harness.state().pane.preview().state() != PreviewState::Playing
        && std::time::Instant::now() < deadline
    {
        harness.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    harness.state_mut().pane.preview_mut().pause();
    let preview = harness.state().pane.preview();
    assert_eq!(
        preview.state(),
        PreviewState::Paused,
        "the Run was not lost"
    );
    // The path ends where the edited program ends.
    let mut end = preview.clone();
    end.set_speed(PreviewSpeed::Turbo);
    end.play();
    end.advance(TICK * 10_000);
    assert_eq!(end.position(), Some(Vec3::new(40.0, 30.0, 0.0)));
    Ok(())
}

#[test]
fn test_app_file_run_hands_the_edit_to_the_machine_at_once() {
    let mut harness = crate::machine_support::app_harness(GcodeKitApp::from_state(
        gcodekit6_ui_egui::AppState::new(
            gcodekit6_settings::SettingsManager::detached(
                gcodekit6_settings::config::Config::default(),
            ),
            crate::test_smoke::native_decision(),
        ),
    ));
    harness.state_mut().show_page(Page::MachineControl);
    harness.run_steps(2);
    harness.get_by_label("G-Code Editor").focus();
    harness.step();
    harness.get_by_label("G-Code Editor").type_text("G0 X12\n");
    harness.step();
    // F5 within the editor's typing pause.
    harness.state_mut().dispatch(Command::FileRun);
    let program = harness.state().state().machine.program().cloned();
    assert_eq!(
        program.as_deref().map(|p| p.join("\n")),
        Some("G0 X12".to_string()),
        "Run hands the editor's text to the machine first, as GTK ran the editor's text"
    );
    assert_eq!(harness.state().page(), Page::Visualizer);
}
