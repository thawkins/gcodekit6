//! The Visualizer's 3D stock removal simulation: the shared simulator's mesh, its alignment
//! with the toolpath (stock top at work Z0), the sidebar toggle following the setting, runs
//! off the UI thread that restart on a new program, translated notices, and a wgpu snapshot
//! of a cut sitting in its groove.

use std::sync::Arc;
use std::time::{Duration, Instant};

use egui_kittest::kittest::Queryable;
use egui_kittest::{Harness, SnapshotOptions};
use gcodekit5_app::i18n::Translator;
use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::panes::visualizer::pipeline::size_px;
use gcodekit5_ui_egui::panes::visualizer::stock::{Notice, RESOLUTION, StockSetup};
use gcodekit5_ui_egui::panes::visualizer::{self, StdView, VisualizerPane, toolpath};
use gcodekit5_ui_egui::theme;
use gcodekit5_ui_egui::workspace::{Pane, PaneContext};
use gcodekit5_visualizer::visualizer::stock_removal_3d::MESH_FLOATS_PER_VERTEX;
use gcodekit5_visualizer::visualizer::{StockSimulator3D, StockZeroReference};
use glam::{Vec3, Vec4Swizzles};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A plunge to Z-2 at (10, 10).
const PLUNGE: &str = "G21\nG90\nG0 Z5\nG0 X10 Y10\nM3 S1000\nG1 Z-2 F100\nG0 Z5\nM5";
/// A straight groove 2 mm deep from (10, 10) to (50, 30).
const GROOVE: &str =
    "G21\nG90\nG0 Z5\nG0 X10 Y10\nM3 S1000\nG1 Z-2 F100\nG1 X50 Y30 F300\nG0 Z5\nM5";

fn program(text: &str) -> Arc<[String]> {
    text.lines().map(str::to_owned).collect()
}

/// Settings with the experimental stock removal on or off.
fn config(enabled: bool) -> Config {
    let mut config = Config::default();
    config.ui.enable_stock_removal_3d = enabled;
    config
}

/// The pane, beside its sidebar, in a harness without a GPU renderer.
fn pane_harness(pane: VisualizerPane) -> Harness<'static, VisualizerPane> {
    let tr = Translator::new("en");
    Harness::builder()
        .with_size(egui::vec2(730.0, 600.0))
        .build_ui_state(
            move |ui, pane: &mut VisualizerPane| {
                let mut requested = Vec::new();
                let mut ctx = PaneContext::new(&tr, &mut requested);
                pane.ui(ui, &mut ctx);
            },
            pane,
        )
}

/// Steps `harness` until neither the toolpath nor the stock is being built.
fn settle(harness: &mut Harness<'_, VisualizerPane>) -> TestResult {
    let deadline = Instant::now() + Duration::from_secs(60);
    while harness.state().is_building()
        || harness.state().stock().is_running()
        || harness.state().stock_restart_pending()
    {
        if Instant::now() > deadline {
            return Err("simulation never finished".into());
        }
        harness.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    harness.run();
    Ok(())
}

/// Loads `text` into the harness's pane.
fn load(harness: &mut Harness<'_, VisualizerPane>, text: &str) {
    let ctx = harness.ctx.clone();
    harness.state_mut().load_program(Some(program(text)), &ctx);
}

/// A pane with the setting on and "Show Stock Removal" checked on `setup`.
fn shown(harness: &mut Harness<'_, VisualizerPane>, setup: StockSetup) {
    let ctx = harness.ctx.clone();
    let pane = harness.state_mut();
    pane.configure(&config(true));
    pane.set_stock_setup(setup, &ctx);
    pane.set_stock_shown(true, &ctx);
}

/// The finished stock mesh's floats.
fn mesh_floats(pane: &VisualizerPane) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let mesh = pane.stock().mesh().ok_or("no stock mesh")?;
    Ok(mesh.upload.snapshot().ok_or("mesh already uploaded")?)
}

/// Vertices (position, normal) of a mesh.
fn vertices(mesh: &[f32]) -> impl Iterator<Item = (Vec3, Vec3)> + '_ {
    mesh.as_chunks::<MESH_FLOATS_PER_VERTEX>()
        .0
        .iter()
        .map(|v| (Vec3::new(v[0], v[1], v[2]), Vec3::new(v[3], v[4], v[5])))
}

/// Whether the triangle `a b c` contains `p` (XY, edges included).
fn contains(a: glam::Vec2, b: glam::Vec2, c: glam::Vec2, p: glam::Vec2) -> bool {
    let side = |u: glam::Vec2, v: glam::Vec2| (v - u).perp_dot(p - u);
    let (d1, d2, d3) = (side(a, b), side(b, c), side(c, a));
    let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(negative && positive)
}

/// A small stock: 40 x 40 x 5 mm.
fn small_stock() -> StockSetup {
    StockSetup {
        width: 40.0,
        height: 40.0,
        thickness: 5.0,
        ..StockSetup::default()
    }
}

#[test]
fn test_shared_simulator_cut_height_and_vertex_count() -> TestResult {
    let mut sim = StockSimulator3D::new(20.0, 20.0, 5.0, 1.0, 1.5)?;
    // Uncut: one box, six faces of two triangles.
    assert_eq!(sim.get_mesh().len() / MESH_FLOATS_PER_VERTEX, 36);
    let toolpath = {
        let mut vis = gcodekit5_visualizer::Visualizer::new();
        vis.parse_gcode(PLUNGE);
        gcodekit5_visualizer::visualizer::toolpath_from_commands(vis.commands())
    };
    sim.simulate_toolpath(&toolpath);
    let map = sim.height_map();
    // Cell (10, 10) covers 10..11 mm; its centre is 0.5 mm from the tool axis.
    assert_eq!(map.height_at(10, 10), Some(3.0), "5 mm stock cut 2 mm deep");
    assert_eq!(map.height_at(0, 0), Some(5.0), "far corner uncut");
    assert!(
        sim.get_mesh().len() / MESH_FLOATS_PER_VERTEX > 36,
        "the pocket adds faces"
    );
    Ok(())
}

#[test]
fn test_uncut_stock_top_is_work_zero_or_thickness() -> TestResult {
    for (zero, top) in [
        (StockZeroReference::Top, 0.0),
        (StockZeroReference::Bottom, 5.0),
    ] {
        let sim = StockSimulator3D::new(20.0, 20.0, 5.0, RESOLUTION, 1.5)?
            .with_stock_zero_reference(zero);
        let mesh = sim.get_mesh();
        let max_z = vertices(&mesh).map(|(p, _)| p.z).fold(f32::MIN, f32::max);
        let min_z = vertices(&mesh).map(|(p, _)| p.z).fold(f32::MAX, f32::min);
        assert_eq!(max_z, top, "{zero:?}: top face");
        assert_eq!(min_z, top - 5.0, "{zero:?}: bottom face");
        // Lower-left corner at work X0 Y0, not centred.
        let min_x = vertices(&mesh).map(|(p, _)| p.x).fold(f32::MAX, f32::min);
        let min_y = vertices(&mesh).map(|(p, _)| p.y).fold(f32::MAX, f32::min);
        assert_eq!((min_x, min_y), (0.0, 0.0));
    }
    Ok(())
}

#[test]
fn test_cut_floor_meets_the_toolpath_in_view_space() -> TestResult {
    let mut harness = pane_harness(VisualizerPane::new());
    shown(&mut harness, small_stock());
    load(&mut harness, PLUNGE);
    settle(&mut harness)?;
    let pane = harness.state();
    let mesh = mesh_floats(pane)?;
    // The surface over (10, 10): the upward triangles that contain that point in XY (the
    // floor is merged into a few large triangles, so it is found by containment).
    let centre = glam::vec2(10.0, 10.0);
    let floor: Vec<Vec3> = vertices(&mesh)
        .collect::<Vec<_>>()
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|t| t.iter().all(|(_, n)| n.z > 0.9))
        .filter(|t| {
            contains(
                t[0].0.truncate(),
                t[1].0.truncate(),
                t[2].0.truncate(),
                centre,
            )
        })
        .flat_map(|t| t.iter().map(|(p, _)| *p))
        .collect();
    assert!(!floor.is_empty(), "a surface over the plunge");
    assert!(floor.iter().all(|p| p.z == -2.0), "floor at Z-2: {floor:?}");
    // The toolpath's plunge ends at the same work point (the render-level check is
    // `test_rendered_cut_lies_in_its_groove`).
    let path = toolpath::build(&program(PLUNGE));
    assert!(
        path.vertices
            .iter()
            .any(|v| Vec3::from_array(v.pos) == Vec3::new(10.0, 10.0, -2.0)),
        "no toolpath vertex at the plunge"
    );
    // The untouched top is at work Z0.
    let top = vertices(&mesh).map(|(p, _)| p.z).fold(f32::MIN, f32::max);
    assert_eq!(top, 0.0);
    Ok(())
}

#[test]
fn test_toggle_follows_the_setting() -> TestResult {
    let mut harness = pane_harness(VisualizerPane::new());
    harness.state_mut().configure(&config(false));
    harness.run();
    assert!(
        harness.query_by_label("Show Stock Removal").is_none(),
        "hidden while the setting is off"
    );
    harness.state_mut().configure(&config(true));
    harness.run();
    harness.get_by_label("Show Stock Removal").click();
    harness.run();
    assert!(harness.state().stock_shown(), "checked by a click");
    assert!(
        harness.query_by_label("Width").is_some(),
        "stock fields shown"
    );
    // Turning the setting off hides and unchecks it, and stops the run.
    harness.state_mut().configure(&config(false));
    harness.run();
    assert!(!harness.state().stock_shown());
    assert!(!harness.state().stock().is_running());
    assert!(harness.query_by_label("Show Stock Removal").is_none());
    Ok(())
}

#[test]
fn test_simulation_restarts_on_program_change() -> TestResult {
    let mut harness = pane_harness(VisualizerPane::new());
    shown(&mut harness, small_stock());
    load(&mut harness, PLUNGE);
    settle(&mut harness)?;
    let first = harness.state().stock().mesh().ok_or("no mesh")?.generation;
    let plunge_vertices = harness.state().stock().mesh().ok_or("no mesh")?.vertices;
    load(&mut harness, GROOVE);
    assert!(
        harness.state().stock().mesh().is_none(),
        "the old program's stock is dropped at once"
    );
    settle(&mut harness)?;
    let mesh = harness
        .state()
        .stock()
        .mesh()
        .ok_or("no mesh after restart")?;
    assert_ne!(mesh.generation, first, "a new run");
    assert_ne!(mesh.vertices, plunge_vertices, "the groove's mesh");
    // Changing the stock restarts it too.
    let ctx = harness.ctx.clone();
    let wider = StockSetup {
        width: 60.0,
        ..small_stock()
    };
    harness.state_mut().set_stock_setup(wider, &ctx);
    assert!(harness.state().stock().mesh().is_none());
    settle(&mut harness)?;
    assert!(harness.state().stock().mesh().is_some());
    Ok(())
}

#[test]
fn test_frame_returns_while_simulating() -> TestResult {
    let mut harness = pane_harness(VisualizerPane::new());
    load(&mut harness, GROOVE);
    settle(&mut harness)?;
    harness.state().stock().pause_worker(true);
    shown(&mut harness, small_stock());
    // Frames run while the worker is held mid-run.
    for _ in 0..5 {
        harness.step();
    }
    assert!(harness.state().stock().is_running());
    assert_eq!(harness.state().stock().progress(), Some(0));
    assert!(
        harness
            .query_by_label("Simulating stock removal\u{2026}")
            .is_some()
    );
    harness.state().stock().pause_worker(false);
    settle(&mut harness)?;
    assert!(harness.state().stock().mesh().is_some());
    assert_eq!(harness.state().stock().progress(), None);
    Ok(())
}

#[test]
fn test_bad_stock_and_empty_program_show_notices() -> TestResult {
    let tr = Translator::new("de");
    let mut harness = pane_harness(VisualizerPane::new());
    load(&mut harness, GROOVE);
    let bad = StockSetup {
        width: 0.0,
        ..small_stock()
    };
    shown(&mut harness, bad);
    settle(&mut harness)?;
    let notice = harness.state().stock().notice().ok_or("no notice")?;
    assert!(matches!(notice, Notice::Invalid(_)));
    let text = notice.text(&tr);
    assert!(text.starts_with("Materialabtrag kann nicht"), "{text}");
    assert!(text.contains("width"), "the reason: {text}");

    let ctx = harness.ctx.clone();
    harness.state_mut().set_stock_setup(small_stock(), &ctx);
    load(&mut harness, "G21\nG90\nM5");
    settle(&mut harness)?;
    assert_eq!(harness.state().stock().notice(), Some(Notice::NoMoves));
    assert!(
        harness
            .query_by_label("Nothing to simulate: the program has no moves.")
            .is_some()
    );
    harness.state_mut().load_program(None, &ctx);
    assert_eq!(harness.state().stock().notice(), Some(Notice::NoMoves));
    Ok(())
}

#[test]
fn test_stock_fits_buffer_limit() {
    use visualizer::pipeline::stock_fits;
    let floats = 36 * MESH_FLOATS_PER_VERTEX;
    assert!(stock_fits(floats, (floats * 4) as u64));
    assert!(!stock_fits(floats, (floats * 4 - 1) as u64));
}

/// The pane in a harness that renders through wgpu, as the visualizer snapshots do.
fn gpu_harness(pane: VisualizerPane) -> Harness<'static, VisualizerPane> {
    let render_state = egui_kittest::wgpu::create_render_state(
        egui_kittest::wgpu::default_wgpu_setup(),
        egui_wgpu::RendererOptions::PREDICTABLE,
    );
    visualizer::install(&render_state);
    let tr = Translator::new("en");
    Harness::builder()
        .with_size(egui::vec2(730.0, 480.0))
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

/// A 60 x 40 x 5 mm stock that holds the whole groove.
fn groove_stock() -> StockSetup {
    StockSetup {
        width: 60.0,
        ..small_stock()
    }
}

/// See `test_visualizer::snapshot_options`: lavapipe references, small rasteriser slack.
fn snapshot_options() -> SnapshotOptions {
    SnapshotOptions::new()
        .threshold(1.0)
        .max_failed_pixels(60)
        .output_path(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots"))
}

#[test]
fn test_snapshot_stock_groove() -> TestResult {
    let mut harness = gpu_harness(VisualizerPane::new());
    shown(&mut harness, groove_stock());
    load(&mut harness, GROOVE);
    settle(&mut harness)?;
    harness
        .state_mut()
        .apply(visualizer::NavAction::View(StdView::IsoSe));
    harness.run();
    harness.render()?;
    assert!(
        harness.state().stock().notice().is_none(),
        "the mesh fits the GPU"
    );
    harness.snapshot_options("visualizer_stock_groove", &snapshot_options());
    Ok(())
}

#[test]
fn test_snapshot_stock_groove_top() -> TestResult {
    let mut harness = gpu_harness(VisualizerPane::new());
    shown(&mut harness, groove_stock());
    load(&mut harness, GROOVE);
    settle(&mut harness)?;
    harness
        .state_mut()
        .apply(visualizer::NavAction::View(StdView::Top));
    harness.run();
    // From above, the cut runs along the middle of its groove.
    harness.snapshot_options("visualizer_stock_groove_top", &snapshot_options());
    Ok(())
}

/// Two grooves 2 mm deep in the 60 x 40 mm stock: along X at Y20 (X10 to X30), and along Y
/// at X45 (Y5 to Y35).
const SLOTS: &str = "G21\nG90\nG0 Z5\nG0 X10 Y20\nM3 S1000\nG1 Z-2 F100\nG1 X30 F300\n\
G0 Z5\nG0 X45 Y5\nG1 Z-2 F100\nG1 Y35 F300\nG0 Z5\nM5";

/// The pixel of `image` at `world`'s projection in the pane's view.
fn pixel_at(image: &image::RgbaImage, pane: &VisualizerPane, world: Vec3) -> Option<[u8; 4]> {
    let rect = pane.view_rect();
    let clip = pane.camera().view_proj(rect.width() / rect.height()) * world.extend(1.0);
    let p = clip.xyz() / clip.w;
    let x = rect.left() + (p.x + 1.0) / 2.0 * rect.width();
    let y = rect.top() + (1.0 - p.y) / 2.0 * rect.height();
    image.get_pixel_checked(x as u32, y as u32).map(|p| p.0)
}

/// Whether some pixel within 1 px of `world`'s projection `matches`.
fn near(
    image: &image::RgbaImage,
    pane: &VisualizerPane,
    world: Vec3,
    matches: impl Fn([u8; 4]) -> bool,
) -> bool {
    let rect = pane.view_rect();
    let clip = pane.camera().view_proj(rect.width() / rect.height()) * world.extend(1.0);
    let p = clip.xyz() / clip.w;
    let x = rect.left() + (p.x + 1.0) / 2.0 * rect.width();
    let y = rect.top() + (1.0 - p.y) / 2.0 * rect.height();
    (-1..=1).any(|dx| {
        (-1..=1).any(|dy| {
            image
                .get_pixel_checked((x as i64 + dx) as u32, (y as i64 + dy) as u32)
                .is_some_and(|p| matches(p.0))
        })
    })
}

/// Whether two colours are within `tolerance` per channel.
fn similar(a: [u8; 4], b: [u8; 4], tolerance: u8) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= tolerance)
}

/// Checks one groove in `image`, seen from 60 degrees up across it. `world(s, across, z)`
/// is the point `s` mm along the groove, `across` mm from its centre line toward the far
/// wall, at height `z`; `top` and `face` are points on the uncut top and on the stock face
/// that faces the camera (lit like the far wall).
fn check_groove(
    image: &image::RgbaImage,
    pane: &VisualizerPane,
    world: impl Fn(f32, f32, f32) -> Vec3,
    along: [f32; 2],
    top: Vec3,
    face: Vec3,
) -> TestResult {
    let top = pixel_at(image, pane, top).ok_or("top off view")?;
    let face = pixel_at(image, pane, face).ok_or("face off view")?;
    assert!(
        !similar(top, face, 12),
        "top {top:?} and face {face:?} tell apart"
    );
    let cut = [255, 255, 0, 255];
    let lit_like = |reference: [u8; 4]| move |p: [u8; 4]| similar(p, reference, 6);
    for s in along {
        // The line at Z-2 shows only while the floor is at Z-2 and the near edge (1.6 mm
        // from the line, at Z0) does not cover it.
        assert!(
            near(image, pane, world(s, 0.0, -2.0), |p| p == cut),
            "the cut at {} shows in its groove",
            world(s, 0.0, -2.0)
        );
        // The floor 1.3 mm toward the far wall (at 1.6 mm) shows only while the floor and
        // the wall are where the cut put them.
        assert!(
            near(image, pane, world(s, 1.3, -2.0), lit_like(top)),
            "the groove floor at {} shows (lit like the top)",
            world(s, 1.3, -2.0)
        );
        assert!(
            near(image, pane, world(s, 1.75, -1.6), lit_like(face)),
            "the far wall behind {} shows (lit like the stock face)",
            world(s, 1.75, -1.6)
        );
    }
    Ok(())
}

#[test]
fn test_rendered_cut_lies_in_its_groove() -> TestResult {
    let mut harness = gpu_harness(VisualizerPane::new());
    shown(&mut harness, groove_stock());
    load(&mut harness, SLOTS);
    settle(&mut harness)?;
    let stock = visualizer::Aabb::new(Vec3::new(0.0, 0.0, -5.0), Vec3::new(60.0, 40.0, 0.0));
    harness.state_mut().camera_mut().fit(stock);
    // From the front (-Y), across the groove along X at Y20.
    harness.state_mut().camera_mut().look(-90.0, 60.0);
    harness.run();
    let image = harness.render()?;
    check_groove(
        &image,
        harness.state(),
        |s, across, z| Vec3::new(s, 20.0 + across, z),
        [15.0, 25.0],
        Vec3::new(20.0, 32.0, 0.0),
        Vec3::new(20.0, 0.0, -4.0),
    )?;
    // From the right (+X), across the groove along Y at X45.
    harness.state_mut().camera_mut().look(0.0, 60.0);
    harness.run();
    let image = harness.render()?;
    check_groove(
        &image,
        harness.state(),
        |s, across, z| Vec3::new(45.0 - across, s, z),
        [12.0, 28.0],
        Vec3::new(35.0, 30.0, 0.0),
        Vec3::new(60.0, 20.0, -4.0),
    )
}

#[test]
fn test_offscreen_size_follows_the_viewport() {
    let screen = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [800, 600],
        pixels_per_point: 2.0,
    };
    let rect = egui::Rect::from_min_max(egui::pos2(100.25, 10.0), egui::pos2(300.0, 400.0));
    // Rounded per edge, clamped to the screen as egui clamps the callback's viewport.
    assert_eq!(size_px(rect, &screen, 8192), [399, 580]);
    // Never past the device's texture limit, never zero.
    assert_eq!(size_px(rect, &screen, 256), [256, 256]);
    let empty = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::Vec2::ZERO);
    assert_eq!(size_px(empty, &screen, 8192), [1, 1]);
}

#[test]
fn test_dragging_a_field_runs_one_simulation() -> TestResult {
    let mut harness = pane_harness(VisualizerPane::new());
    shown(&mut harness, small_stock());
    load(&mut harness, GROOVE);
    settle(&mut harness)?;
    let ctx = harness.ctx.clone();
    let mut last = small_stock();
    // 50 edits a frame apart, as a drag makes them.
    for i in 0..50 {
        last.width = 40.0 + i as f32 * 0.5;
        harness.state_mut().edit_stock_setup(last, &ctx);
        harness.step();
    }
    assert!(
        harness.state().stock_restart_pending(),
        "still waiting for a pause"
    );
    settle(&mut harness)?;
    let stock = harness.state().stock();
    assert!(
        stock.peak_workers() <= 2,
        "{} workers at once",
        stock.peak_workers()
    );
    let mesh = stock.mesh().ok_or("no mesh")?;
    assert_eq!(mesh.setup, last, "simulated with the last values");
    Ok(())
}

/// A CNC design in the app: a 30 x 20 mm rectangle cut 1 mm deep with a 2 mm tool in 6 mm
/// stock.
fn cnc_design(h: &mut Harness<'static, gcodekit5_ui_egui::GcodeKitApp>) {
    use gcodekit5_designer::designer_state::MachineMode;
    use gcodekit5_designer::model::{DesignRectangle, Shape};
    use gcodekit5_designer::stock_removal::StockMaterial;
    {
        let designer = h.state().designer();
        let mut designer = designer.borrow_mut();
        let state = designer.state_mut();
        state.set_machine_mode(MachineMode::Cnc3D);
        state.stock_material = Some(StockMaterial::new(80.0, 60.0, 6.0, (0.0, 0.0, 0.0)));
        state.set_tool_diameter(2.0);
        state.add_shape_with_undo_and_depth(
            Shape::Rectangle(DesignRectangle::new(20.0, 20.0, 40.0, 30.0)),
            // A start depth of 1 mm: the profile is cut 1 mm into the 6 mm stock.
            1.0,
            1.0,
        );
    }
    crate::test_designer_files_app::frames(h, 1);
}

#[test]
fn test_designer_generate_seeds_the_stock() -> TestResult {
    let (mut h, _script) =
        crate::test_designer_files_app::app(crate::test_designer_files_app::default_state());
    cnc_design(&mut h);
    h.state()
        .designer()
        .borrow_mut()
        .request_to_editor(gcodekit5_ui_egui::panes::designer::ToEditor::Generate);
    crate::test_designer_files_app::frames(&mut h, 3);
    let seed = h.state().state().stock_seed.ok_or("no seed")?;
    assert_eq!(
        (seed.width, seed.height, seed.thickness, seed.tool_diameter),
        (80.0, 60.0, 6.0, 2.0)
    );
    let gcode = h.state().editor().borrow().buffer().text().to_string();

    // The visualizer takes the seed with the program; the header says Stock Z0: bottom.
    let tr = Translator::new("en");
    let mut pane = VisualizerPane::new();
    pane.configure(&config(true));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(730.0, 600.0))
        .build_ui_state(
            move |ui, pane: &mut VisualizerPane| {
                let mut requested = Vec::new();
                let mut ctx = PaneContext::new(&tr, &mut requested).with_stock_seed(Some(seed));
                pane.ui(ui, &mut ctx);
            },
            pane,
        );
    let ctx = harness.ctx.clone();
    harness.state_mut().set_stock_shown(true, &ctx);
    harness.run();
    load(&mut harness, &gcode);
    settle(&mut harness)?;
    let pane = harness.state();
    let setup = pane.stock_setup();
    assert_eq!(
        (
            setup.width,
            setup.height,
            setup.thickness,
            setup.tool_diameter
        ),
        (80.0, 60.0, 6.0, 2.0)
    );
    assert_eq!(setup.zero, StockZeroReference::Bottom);
    let mesh = mesh_floats(pane)?;
    assert_eq!(pane.stock().mesh().ok_or("no mesh")?.setup, setup);
    // Safe-Z moves do not cut: the top stays at the stock thickness everywhere but the
    // rectangle's outline (X20..60, Y20..50), and the cut is 1 mm deep.
    let heights: Vec<f32> = vertices(&mesh)
        .filter(|(_, n)| n.z > 0.9)
        .map(|(p, _)| p.z)
        .collect();
    let top = heights.iter().copied().fold(f32::MIN, f32::max);
    let floor = heights.iter().copied().fold(f32::MAX, f32::min);
    assert_eq!(top, 6.0, "uncut top at the stock thickness");
    assert!(
        (floor - 5.0).abs() < 1e-4,
        "the profile is 1 mm deep: {floor}"
    );
    let centre = glam::vec2(5.0, 5.0);
    let tris: Vec<_> = vertices(&mesh).collect();
    let over_corner: Vec<f32> = tris
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|t| t.iter().all(|(_, n)| n.z > 0.9))
        .filter(|t| {
            contains(
                t[0].0.truncate(),
                t[1].0.truncate(),
                t[2].0.truncate(),
                centre,
            )
        })
        .flat_map(|t| t.iter().map(|(p, _)| p.z))
        .collect();
    assert!(
        !over_corner.is_empty() && over_corner.iter().all(|z| *z == 6.0),
        "the travel from the origin did not cut: {over_corner:?}"
    );
    Ok(())
}
