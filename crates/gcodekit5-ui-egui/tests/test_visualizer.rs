//! The Visualizer pane: camera maths, the toolpath built from the streamed program, stream
//! progress, the nav cube and View commands, and wgpu snapshots of a sample program.

use std::sync::Arc;
use std::time::{Duration, Instant};

use egui_kittest::kittest::Queryable;
use egui_kittest::{Harness, SnapshotOptions};
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::MachineEvent;
use gcodekit5_ui_egui::commands::Command;
use gcodekit5_ui_egui::panes::visualizer::toolpath::{self, TRAVEL_COLOR};
use gcodekit5_ui_egui::panes::visualizer::{self, Aabb, Camera, StdView, VisualizerPane};
use gcodekit5_ui_egui::theme;
use gcodekit5_ui_egui::workspace::{Pane, PaneContext};
use glam::{Vec3, Vec4Swizzles};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The sample program, as the machine streams it: a pocket outline with a rapid approach,
/// a plunge, four cuts, a quarter arc and a retract.
const SAMPLE: [&str; 13] = [
    "G21",
    "G90",
    "G0 Z5",
    "G0 X0 Y0",
    "M3 S1000",
    "G1 Z-1 F300",
    "G1 X40",
    "G1 Y30",
    "G3 X20 Y50 I-20 J0",
    "G1 X0",
    "G1 Y0",
    "G0 Z5",
    "M5",
];

fn sample() -> Arc<[String]> {
    SAMPLE.iter().map(|s| s.to_string()).collect()
}

/// `p` in normalised device coordinates under `view_proj`.
fn ndc(view_proj: glam::Mat4, p: Vec3) -> Vec3 {
    let clip = view_proj * p.extend(1.0);
    clip.xyz() / clip.w
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn test_fit_contains_bounds() {
    let bounds = Aabb::new(Vec3::new(0.0, -20.0, -5.0), Vec3::new(100.0, 50.0, 10.0));
    let views = [StdView::IsoSe, StdView::Top, StdView::Front, StdView::IsoNw];
    for aspect in [0.4, 1.0, 2.5] {
        for view in views {
            let mut camera = Camera::default();
            camera.set_aspect(aspect);
            camera.view(view);
            camera.fit(bounds);
            // Off the standard angles the projection is perspective.
            for orbit in [0.0, 60.0] {
                let mut camera = camera;
                camera.orbit(orbit, orbit / 2.0);
                let m = camera.view_proj(aspect);
                for corner in bounds.corners() {
                    let p = ndc(m, corner);
                    assert!(
                        p.x.abs() <= 1.0 && p.y.abs() <= 1.0 && (0.0..=1.0).contains(&p.z),
                        "{view:?} aspect {aspect} orbit {orbit}: {corner} -> {p}"
                    );
                }
            }
        }
    }
}

#[test]
fn test_fit_centres_bounds() {
    let bounds = Aabb::new(Vec3::new(10.0, 10.0, 0.0), Vec3::new(30.0, 50.0, 4.0));
    let mut camera = Camera::default();
    camera.fit(bounds);
    assert_eq!(camera.target(), Vec3::new(20.0, 30.0, 2.0));
    let centre = ndc(camera.view_proj(1.0), camera.target());
    assert!(centre.x.abs() < 1e-4 && centre.y.abs() < 1e-4, "{centre}");
}

#[test]
fn test_view_top_looks_down_z() {
    let mut camera = Camera::default();
    camera.view(StdView::Top);
    let eye_dir = (camera.eye() - camera.target()).normalize();
    assert!(
        (eye_dir - Vec3::Z).length() < 1e-4,
        "eye direction {eye_dir}"
    );
    let m = camera.view_proj(1.0);
    let o = ndc(m, camera.target());
    let x = ndc(m, camera.target() + Vec3::X);
    let y = ndc(m, camera.target() + Vec3::Y);
    assert!(x.x > o.x && close(x.y, o.y), "+X goes right: {o} -> {x}");
    assert!(y.y > o.y && close(y.x, o.x), "+Y goes up: {o} -> {y}");
}

#[test]
fn test_view_front_looks_along_y() {
    let mut camera = Camera::default();
    camera.view(StdView::Front);
    let eye_dir = (camera.eye() - camera.target()).normalize();
    assert!(
        (eye_dir + Vec3::Y).length() < 1e-4,
        "eye direction {eye_dir}"
    );
    let m = camera.view_proj(1.0);
    let o = ndc(m, camera.target());
    let z = ndc(m, camera.target() + Vec3::Z);
    assert!(z.y > o.y, "+Z goes up: {o} -> {z}");
}

#[test]
fn test_orbit_pan_zoom() {
    let mut camera = Camera::default();
    let (yaw, _) = camera.angles();
    camera.orbit(100.0, 0.0);
    assert!(camera.angles().0 < yaw, "dragging right turns the view");
    camera.orbit(0.0, -100_000.0);
    assert!(camera.angles().1 <= 89.0 + 1e-3, "pitch is clamped");

    let distance = camera.distance();
    camera.zoom(10.0);
    assert!(close(camera.distance(), distance - 10.0));
    camera.zoom(-1e9);
    assert!(camera.distance() <= 10_000.0, "zoom out is clamped");

    let target = camera.target();
    camera.pan(50.0, 0.0);
    assert_ne!(camera.target(), target);
}

#[test]
fn test_toolpath_tags_each_segment_with_its_line() {
    let mesh = toolpath::build(&sample());
    let lines: Vec<u32> = mesh.vertices.iter().map(|v| v.line).collect();
    // Line 2 (G0 Z5) is the first move; M3/M5 and modal lines draw nothing.
    assert_eq!(lines.first(), Some(&2));
    assert!(lines.windows(2).all(|w| w[0] <= w[1]), "program order");
    assert!(!lines.contains(&4) && !lines.contains(&12));
    // The quarter arc (line 8) is split into several segments.
    let arc = mesh.vertices.iter().filter(|v| v.line == 8).count();
    assert!(arc >= 8, "arc split into {} vertices", arc);
    // Rapids are travel, feed moves with the spindle on are cuts.
    let colour = |line: u32| {
        mesh.vertices
            .iter()
            .find(|v| v.line == line)
            .map(|v| v.color)
    };
    assert_eq!(colour(3), Some(TRAVEL_COLOR));
    assert_ne!(colour(6), Some(TRAVEL_COLOR));
    let bounds = mesh.bounds.ok_or("no bounds").map_err(|e| e.to_string());
    assert_eq!(
        bounds,
        Ok(Aabb::new(
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(40.0, 50.0, 5.0)
        ))
    );
}

#[test]
fn test_toolpath_keeps_counting_past_real_dwells() {
    let program: Vec<String> = ["G1 X1 F100", "G4 P2", "G1 X2", "G1 X3"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mesh = toolpath::build(&program);
    let lines: Vec<u32> = mesh.vertices.iter().map(|v| v.line).collect();
    assert_eq!(lines, vec![0, 0, 2, 2, 3, 3]);
}

#[test]
fn test_toolpath_empty_program() {
    let mesh = toolpath::build(&[]);
    assert!(mesh.vertices.is_empty());
    assert_eq!(mesh.bounds, None);
}

#[test]
fn test_progress_follows_stream() {
    let tr = Translator::new("en");
    let mut pane = VisualizerPane::new();
    pane.machine_events(
        &[MachineEvent::StreamProgress {
            sent: 6,
            acked: 4,
            total: 13,
        }],
        &tr,
    );
    assert_eq!(pane.progress(), 4);
    pane.machine_events(&[MachineEvent::StreamFinished { lines: 13 }], &tr);
    assert_eq!(pane.progress(), 13);
    pane.machine_events(
        &[MachineEvent::StreamStopped {
            acked: 2,
            total: 13,
        }],
        &tr,
    );
    assert_eq!(pane.progress(), 2);
}

#[test]
fn test_view_commands_move_the_camera() {
    let tr = Translator::new("en");
    let mut requested = Vec::new();
    let mut ctx = PaneContext::new(&tr, &mut requested);
    let mut pane = VisualizerPane::new();
    assert!(pane.command(Command::ViewTop, &mut ctx));
    assert_eq!(pane.camera().angles(), StdView::Top.angles());
    assert!(pane.command(Command::ViewFront, &mut ctx));
    assert_eq!(pane.camera().angles(), StdView::Front.angles());
    let distance = pane.camera().distance();
    assert!(pane.command(Command::ZoomIn, &mut ctx));
    assert!(close(
        pane.camera().distance(),
        distance - visualizer::ZOOM_STEP
    ));
    assert!(!pane.command(Command::Home, &mut ctx), "not the pane's");
}

/// The pane in a harness without a GPU renderer, drawn with an English translator.
fn pane_harness(pane: VisualizerPane) -> Harness<'static, VisualizerPane> {
    let tr = Translator::new("en");
    Harness::builder()
        .with_size(egui::vec2(480.0, 360.0))
        .build_ui_state(
            move |ui, pane: &mut VisualizerPane| {
                let mut requested = Vec::new();
                let mut ctx = PaneContext::new(&tr, &mut requested);
                pane.ui(ui, &mut ctx);
            },
            pane,
        )
}

/// Loads `program` into the harness's pane and steps until its toolpath is built.
fn load(harness: &mut Harness<'_, VisualizerPane>, program: Arc<[String]>) -> TestResult {
    let ctx = harness.ctx.clone();
    harness.state_mut().load_program(Some(program), &ctx);
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().is_building() {
        if Instant::now() > deadline {
            return Err("toolpath never built".into());
        }
        harness.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    harness.run();
    Ok(())
}

#[test]
fn test_nav_cube_click_front_sets_view() -> TestResult {
    let mut harness = pane_harness(VisualizerPane::new());
    harness.run();
    harness.get_by_label("Front View").click();
    harness.run();
    assert_eq!(harness.state().camera().angles(), StdView::Front.angles());
    harness.get_by_label("Top View").click();
    harness.run();
    assert_eq!(harness.state().camera().angles(), StdView::Top.angles());
    Ok(())
}

#[test]
fn test_program_builds_once_and_fits() -> TestResult {
    let mut harness = pane_harness(VisualizerPane::new());
    harness.run();
    let program = sample();
    load(&mut harness, Arc::clone(&program))?;
    let mesh = harness.state().mesh().map(|m| m as *const _);
    assert!(mesh.is_some(), "toolpath built");
    assert_eq!(
        harness.state().camera().target(),
        Vec3::new(20.0, 25.0, 2.0)
    );
    // The same program again: nothing is rebuilt.
    let ctx = harness.ctx.clone();
    harness.state_mut().load_program(Some(program), &ctx);
    assert!(!harness.state().is_building());
    assert_eq!(harness.state().mesh().map(|m| m as *const _), mesh);
    // No program: the view is empty again.
    harness.state_mut().load_program(None, &ctx);
    assert!(harness.state().mesh().is_none());
    Ok(())
}

/// The pane in a harness that renders through wgpu with the visualizer installed.
fn gpu_harness(pane: VisualizerPane) -> Harness<'static, VisualizerPane> {
    let render_state = egui_kittest::wgpu::create_render_state(
        egui_kittest::wgpu::default_wgpu_setup(),
        egui_wgpu::RendererOptions::PREDICTABLE,
    );
    visualizer::install(&render_state);
    let tr = Translator::new("en");
    Harness::builder()
        .with_size(egui::vec2(480.0, 360.0))
        .with_theme(egui::Theme::Dark)
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(
            render_state,
        ))
        .build_ui_state(
            move |ui, pane: &mut VisualizerPane| {
                theme::install(ui.ctx());
                let mut requested = Vec::new();
                let mut ctx = PaneContext::new(&tr, &mut requested);
                pane.ui(ui, &mut ctx);
            },
            pane,
        )
}

/// Snapshot options: the checked-in references, with lavapipe's tolerance.
fn snapshot_options() -> SnapshotOptions {
    SnapshotOptions::new()
        .output_path(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots"))
}

#[test]
fn test_snapshot_iso_sample() -> TestResult {
    let mut harness = gpu_harness(VisualizerPane::new());
    load(&mut harness, sample())?;
    // Eight lines are acknowledged: the approach, the plunge and the first two cuts show
    // as done.
    let tr = Translator::new("en");
    harness.state_mut().machine_events(
        &[MachineEvent::StreamProgress {
            sent: 10,
            acked: 8,
            total: 13,
        }],
        &tr,
    );
    harness.run();
    harness.snapshot_options("visualizer_iso_sample", &snapshot_options());
    Ok(())
}

#[test]
fn test_snapshot_top_sample() -> TestResult {
    let mut harness = gpu_harness(VisualizerPane::new());
    load(&mut harness, sample())?;
    harness
        .state_mut()
        .apply(visualizer::NavAction::View(StdView::Top));
    harness.run();
    harness.snapshot_options("visualizer_top_sample", &snapshot_options());
    Ok(())
}

/// Stands in for the Visualizer and records the commands it is offered.
struct Probe(std::rc::Rc<std::cell::RefCell<Vec<Command>>>);

impl Pane for Probe {
    fn id(&self) -> gcodekit5_ui_egui::workspace::PaneId {
        gcodekit5_ui_egui::workspace::PaneId::Visualizer
    }

    fn title(&self, _tr: &Translator) -> String {
        String::new()
    }

    fn ui(&mut self, _ui: &mut egui::Ui, _ctx: &mut PaneContext) {}

    fn command(&mut self, command: Command, _ctx: &mut PaneContext) -> bool {
        self.0.borrow_mut().push(command);
        true
    }
}

#[test]
fn test_app_routes_view_commands_to_the_visualizer() {
    use gcodekit5_settings::SettingsManager;
    use gcodekit5_settings::config::Config;
    use gcodekit5_ui_egui::{AppState, GcodeKitApp};

    let mut app = GcodeKitApp::from_state(AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    ));
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    app.workspace_mut()
        .set_pane(Box::new(Probe(std::rc::Rc::clone(&seen))));
    let view = [
        Command::ZoomFit,
        Command::ZoomIn,
        Command::ZoomOut,
        Command::ResetView,
        Command::ViewTop,
        Command::ViewFront,
        Command::ViewIso,
    ];
    for command in view {
        app.dispatch(command);
    }
    assert_eq!(*seen.borrow(), view.to_vec());
}
