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
    // The nav cube's T comes first; the floating Top View button under it is the second.
    assert_eq!(harness.get_all_by_label("Top View").count(), 2);
    for index in 0..2 {
        harness
            .state_mut()
            .apply(visualizer::NavAction::View(StdView::Front));
        harness.run();
        harness
            .get_all_by_label("Top View")
            .nth(index)
            .ok_or("Top View button")?
            .click();
        harness.run();
        assert_eq!(harness.state().camera().angles(), StdView::Top.angles());
    }
    Ok(())
}

#[test]
fn test_program_builds_once_and_fits() -> TestResult {
    let mut harness = pane_harness(VisualizerPane::new());
    harness.run();
    let program = sample();
    load(&mut harness, Arc::clone(&program))?;
    let generation = harness.state().toolpath_generation();
    assert!(generation.is_some(), "toolpath built");
    assert_eq!(
        harness.state().camera().target(),
        Vec3::new(20.0, 25.0, 2.0)
    );
    // The same program again: nothing is rebuilt.
    let ctx = harness.ctx.clone();
    harness.state_mut().load_program(Some(program), &ctx);
    assert!(!harness.state().is_building());
    assert_eq!(harness.state().toolpath_generation(), generation);
    // No program: the view is empty again.
    harness.state_mut().load_program(None, &ctx);
    assert_eq!(harness.state().toolpath_generation(), None);
    Ok(())
}

#[test]
fn test_edit_keeps_view_new_file_refits() -> TestResult {
    let mut harness = pane_harness(VisualizerPane::new());
    harness.run();
    load(&mut harness, sample())?;
    harness
        .state_mut()
        .apply(visualizer::NavAction::View(StdView::Top));
    harness.state_mut().camera_mut().pan(40.0, 10.0);
    let before = *harness.state().camera();

    // An edit (one coordinate changed, as after a typing pause): a new Arc, same program.
    let mut edited: Vec<String> = SAMPLE.iter().map(|s| s.to_string()).collect();
    edited[6] = "G1 X45".to_string();
    load(&mut harness, edited.into())?;
    let after = *harness.state().camera();
    assert_eq!(after.angles(), StdView::Top.angles(), "still the top view");
    assert_eq!(after.target(), before.target(), "not refitted");
    assert_eq!(after.distance(), before.distance(), "not refitted");

    // A different program (a newly opened file): isometric and fitted.
    let other: Vec<String> = ["G0 X100 Y100", "G1 X200 Y150 F500 S100 M3", "G1 Y300"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    load(&mut harness, other.into())?;
    let fitted = *harness.state().camera();
    assert_eq!(fitted.angles(), StdView::IsoSe.angles());
    // The rapid starts at the origin, so the bounds are X 0..200, Y 0..300.
    assert_eq!(fitted.target(), Vec3::new(100.0, 150.0, 0.0));
    Ok(())
}

#[test]
fn test_stale_builds_are_dropped() -> TestResult {
    let worker = visualizer::build::BuildWorker::new();
    let ctx = egui::Context::default();
    let big: Arc<[String]> = (0..200_000)
        .map(|i| format!("G1 X{} F500", i % 50))
        .collect();
    worker.request(1, big, None, &ctx);
    worker.request(2, sample(), None, &ctx);
    let deadline = Instant::now() + Duration::from_secs(20);
    let built = loop {
        if let Some(visualizer::build::Outcome::Done(built)) = worker.take() {
            break built;
        }
        if Instant::now() > deadline {
            return Err("no build delivered".into());
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    assert_eq!(built.generation, 2, "only the latest request is delivered");
    assert!(built.replaced, "nothing was shown before");
    worker.cancel(3);
    worker.request(4, sample(), Some(sample()), &ctx);
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(visualizer::build::Outcome::Done(built)) = worker.take() {
            assert_eq!(built.generation, 4);
            assert!(
                !built.replaced,
                "the same program is an edit, not a new file"
            );
            return Ok(());
        }
        if Instant::now() > deadline {
            return Err("no build delivered".into());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn test_toolpath_fits_buffer_limit() {
    use visualizer::pipeline::toolpath_fits;
    let stride = std::mem::size_of::<toolpath::ToolpathVertex>() as u64;
    assert_eq!(stride, 20);
    let limit = 256 << 20;
    assert!(toolpath_fits(0, limit));
    assert!(toolpath_fits((limit / stride) as usize, limit));
    assert!(!toolpath_fits((limit / stride) as usize + 1, limit));
    assert!(!toolpath_fits(usize::MAX, u64::MAX), "byte count overflows");
    assert!(
        !toolpath_fits(u32::MAX as usize + 1, u64::MAX),
        "draw range is u32"
    );
}

/// The pane in a harness that renders through wgpu with the visualizer installed; the view
/// beside the sidebar is 480x360, the size these tests were written for.
fn gpu_harness(pane: VisualizerPane) -> Harness<'static, VisualizerPane> {
    let render_state = egui_kittest::wgpu::create_render_state(
        egui_kittest::wgpu::default_wgpu_setup(),
        egui_wgpu::RendererOptions::PREDICTABLE,
    );
    visualizer::install(&render_state);
    let tr = Translator::new("en");
    // 250 points of sidebar and a 480x360 view.
    Harness::builder()
        .with_size(egui::vec2(730.0, 360.0))
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

/// Snapshot options: the checked-in references (made with Mesa lavapipe on Fedora; CI runs
/// Ubuntu's lavapipe). A pixel fails beyond a YIQ distance of 1.0 (kittest's default is
/// 0.6), and up to 40 failing pixels of the 262,800 pass: enough for a Mesa line-rasteriser
/// difference on a few line ends, while a missing, recoloured or moved line (hundreds of
/// pixels) still fails.
fn snapshot_options() -> SnapshotOptions {
    SnapshotOptions::new()
        .threshold(1.0)
        .max_failed_pixels(40)
        .output_path(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots"))
}

/// Whether `image` has a pixel within 2 px of `world`'s projection that `matches`.
fn pixel_near(
    image: &dyn Fn(u32, u32) -> Option<[u8; 4]>,
    pane: &VisualizerPane,
    world: Vec3,
    matches: impl Fn([u8; 4]) -> bool,
) -> bool {
    let rect = pane.view_rect();
    let p = ndc(pane.camera().view_proj(rect.width() / rect.height()), world);
    let x = rect.left() + (p.x + 1.0) / 2.0 * rect.width();
    let y = rect.top() + (1.0 - p.y) / 2.0 * rect.height();
    (-2..=2).any(|dx| {
        (-2..=2).any(|dy| {
            let (px, py) = (x as i64 + dx, y as i64 + dy);
            u32::try_from(px)
                .ok()
                .zip(u32::try_from(py).ok())
                .and_then(|(px, py)| image(px, py))
                .is_some_and(&matches)
        })
    })
}

#[test]
fn test_progress_colours_acknowledged_lines_only() -> TestResult {
    let mut harness = gpu_harness(VisualizerPane::new());
    load(&mut harness, sample())?;
    harness
        .state_mut()
        .apply(visualizer::NavAction::View(StdView::Top));
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
    let rendered = harness.render()?;
    let image = |x: u32, y: u32| rendered.get_pixel_checked(x, y).map(|p| p.0);
    let accent = theme::DARK.accent.to_array();
    let is_accent = |p: [u8; 4]| p == accent;
    let is_cut = |p: [u8; 4]| p == [255, 255, 0, 255];
    let pane = harness.state();
    // Line 7 (G1 Y30, x = 40) is acknowledged at acked = 8.
    let line7 = Vec3::new(40.0, 15.0, -1.0);
    assert!(pixel_near(&image, pane, line7, is_accent), "line 7 done");
    // Line 8 (the arc) is not.
    let a = std::f32::consts::FRAC_PI_4;
    let line8 = Vec3::new(20.0 + 20.0 * a.cos(), 30.0 + 20.0 * a.sin(), -1.0);
    assert!(
        pixel_near(&image, pane, line8, is_cut),
        "line 8 still a cut"
    );
    assert!(
        !pixel_near(&image, pane, line8, is_accent),
        "line 8 not done"
    );
    Ok(())
}

#[test]
fn test_upload_frees_cpu_vertices() -> TestResult {
    let mut harness = gpu_harness(VisualizerPane::new());
    load(&mut harness, sample())?;
    assert!(harness.state().toolpath_on_cpu(), "held until uploaded");
    harness.render()?;
    assert!(!harness.state().toolpath_on_cpu(), "freed once on the GPU");
    Ok(())
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

#[test]
fn test_panicking_build_leaves_worker_usable() -> TestResult {
    let _guard = crate::global_env_lock();
    let mut harness = pane_harness(VisualizerPane::new());
    harness.run();
    // A good program A is shown first.
    load(&mut harness, sample())?;
    let a_bounds = harness.state().bounds().ok_or("program A has bounds")?;
    assert!(harness.state().toolpath_generation().is_some());
    assert!(!harness.state().build_failed());
    // Program B panics its build: nothing of A may stay on screen, and the OSD says so.
    visualizer::build::PANIC_NEXT_BUILD.store(true, std::sync::atomic::Ordering::Release);
    let mut b: Vec<String> = SAMPLE.iter().map(|s| s.to_string()).collect();
    b.push("G0 X1".to_string());
    let loaded = load(&mut harness, b.into());
    visualizer::build::PANIC_NEXT_BUILD.store(false, std::sync::atomic::Ordering::Release);
    loaded?;
    assert!(!harness.state().is_building(), "building cleared");
    assert_eq!(harness.state().toolpath_generation(), None, "no stale mesh");
    assert_eq!(harness.state().bounds(), None, "no stale bounds");
    assert!(harness.state().build_failed(), "failure notice shown");
    // The next program still builds, and it is the new program that is drawn.
    let c: Vec<String> = ["G21", "G90", "G0 X0 Y0", "G1 X100 Y0 F300"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    // Loading C starts its build; the notice for B is gone before any poll sees the result.
    let ctx = harness.ctx.clone();
    harness
        .state_mut()
        .load_program(Some(c.clone().into()), &ctx);
    assert!(
        !harness.state().build_failed(),
        "notice cleared as soon as the new build starts"
    );
    load(&mut harness, c.into())?;
    assert!(harness.state().toolpath_generation().is_some());
    assert!(
        !harness.state().build_failed(),
        "notice cleared by a good build"
    );
    let c_bounds = harness.state().bounds().ok_or("program C has bounds")?;
    assert!(close(c_bounds.max.x, 100.0), "C's bounds: {c_bounds:?}");
    assert!(
        !close(c_bounds.max.x, a_bounds.max.x),
        "not A's bounds: {c_bounds:?}"
    );
    Ok(())
}
