//! The Visualizer page as GTK lays it out: the sidebar (Fit to Content, Reset View, Hide,
//! Inspector) left of the view, the nav cube at the view's top right with the Top View and
//! Mouse Controls buttons under it. At 1280x800 (1x and 2x) none of them is clipped; the
//! sidebar's buttons work; and a snapshot of the page at 1440x900 dark.

use std::time::{Duration, Instant};

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use egui_kittest::{Harness, SnapshotOptions};
use gcodekit5_app::i18n::Translator;
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::panes::visualizer::{self, StdView, VisualizerPane};
use gcodekit5_ui_egui::workspace::{Page, Pane, PaneContext};
use gcodekit5_ui_egui::{AppState, GcodeKitApp};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A rectangle pocket and a circle, as in GTK's help screenshot.
const PROGRAM: &str = "G21\nG90\nG0 Z5\nG0 X60 Y80\nM3 S1000\nG1 Z-1 F300\nG1 X100\nG1 Y120\n\
G1 X60\nG1 Y80\nG0 Z5\nG0 X170 Y40\nG1 Z-1\nG2 X170 Y40 I-20 J0\nG0 Z5\nM5\n";

/// The nav cube's and the sidebar's buttons, which must all be on screen.
const CONTROLS: [&str; 17] = [
    "Isometric (NW)",
    "Top View",
    "Isometric (NE)",
    "Left View",
    "Front View",
    "Right View",
    "Isometric (SW)",
    "Bottom View",
    "Isometric (SE)",
    "Zoom Out",
    "Fit to Content",
    "Zoom In",
    "Mouse Controls",
    "Fit to Device Working Area",
    "Reset View",
    "Hide Sidebar",
    "Inspector",
];

/// A visualizer pane already showing `state`'s program, so the page draws it at once.
fn loaded_pane(state: &AppState) -> Result<VisualizerPane, Box<dyn std::error::Error>> {
    let program = state.machine.program().cloned().ok_or("no program")?;
    let mut harness = pane_harness();
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
    Ok(std::mem::take(harness.state_mut()))
}

/// The app on the Visualizer page with [`PROGRAM`] loaded, in a harness of `size` points at
/// `ppp` pixels per point, dark, with the theme installed; drawn through wgpu when `wgpu`.
fn page_app(
    size: egui::Vec2,
    ppp: f32,
    wgpu: bool,
) -> Result<Harness<'static, GcodeKitApp>, Box<dyn std::error::Error>> {
    let mut state = AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    );
    state.machine.set_program(PROGRAM)?;
    let pane = loaded_pane(&state)?;
    let mut app = GcodeKitApp::from_state(state);
    app.workspace_mut().set_pane(Box::new(pane));
    app.show_page(Page::Visualizer);
    let builder = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(ppp)
        .with_theme(egui::Theme::Dark);
    let builder = if wgpu {
        let render_state = egui_kittest::wgpu::create_render_state(
            egui_kittest::wgpu::default_wgpu_setup(),
            egui_wgpu::RendererOptions::PREDICTABLE,
        );
        visualizer::install(&render_state);
        builder.renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(
            render_state,
        ))
    } else {
        builder
    };
    let mut harness = builder.build_ui_state(
        |ui, app: &mut GcodeKitApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        app,
    );
    gcodekit5_ui_egui::theme::install(&harness.ctx);
    harness.run_steps(4);
    Ok(harness)
}

/// The first button named `label` (the nav cube's, where GTK also has a sidebar or floating
/// button of the same name).
///
/// # Panics
/// When no node is named `label`.
fn first(harness: &Harness<'_, GcodeKitApp>, label: &str) -> egui::Rect {
    match harness.get_all_by_label(label).next() {
        Some(node) => node.rect(),
        None => panic!("no node named {label:?}"),
    }
}

#[test]
fn test_visualizer_controls_visible_at_1280x800() -> TestResult {
    for ppp in [1.0, 2.0] {
        let harness = page_app(egui::vec2(1280.0, 800.0), ppp, false)?;
        let bottom = harness.get_by_label("eStop").rect().top();
        let page = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1280.0, bottom));
        for label in CONTROLS {
            let rects: Vec<egui::Rect> = harness
                .get_all_by_label(label)
                .map(|node| node.rect())
                .collect();
            assert!(!rects.is_empty(), "no {label:?} at ppp {ppp}");
            for rect in rects {
                assert!(
                    page.contains_rect(rect),
                    "{label} at {rect:?} outside {page:?} at ppp {ppp}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn test_visualizer_sidebar_is_left_of_the_nav_cube() -> TestResult {
    let harness = page_app(egui::vec2(1280.0, 800.0), 1.0, false)?;
    let reset = harness.get_by_label("Reset View").rect();
    let hide = harness.get_by_label("Hide Sidebar").rect();
    let inspector = harness.get_by_label("Inspector").rect();
    let top = first(&harness, "Top View");
    let zoom_in = harness.get_by_label("Zoom In").rect();
    let help = harness.get_by_label("Mouse Controls").rect();
    assert!((reset.top() - hide.top()).abs() < 1.0, "toolbar is one row");
    assert!(
        inspector.top() > reset.bottom(),
        "inspector under the toolbar"
    );
    assert!(hide.right() < 400.0, "sidebar at the left: {hide:?}");
    assert!(top.left() > 1000.0, "nav cube at the right: {top:?}");
    assert!(zoom_in.top() > top.bottom(), "zoom row under the views");
    assert!(help.top() > zoom_in.bottom(), "help under the nav cube");
    assert!(
        (help.right() - zoom_in.right()).abs() < 1.0,
        "help in the right column"
    );
    Ok(())
}

/// The pane alone in a 730x360 harness, so its view is 480 points wide beside the sidebar.
fn pane_harness() -> Harness<'static, VisualizerPane> {
    let tr = Translator::new("en");
    Harness::builder()
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
fn test_visualizer_reset_view_button_resets_the_camera() {
    let mut harness = pane_harness();
    harness.run();
    harness
        .state_mut()
        .apply(visualizer::NavAction::View(StdView::Front));
    harness.state_mut().camera_mut().zoom(50.0);
    harness.run();
    harness.get_by_label("Reset View").click();
    harness.run();
    let mut reset = visualizer::Camera::default();
    reset.reset();
    assert_eq!(harness.state().camera().angles(), reset.angles());
    assert_eq!(harness.state().camera().target(), reset.target());
}

#[test]
fn test_visualizer_sidebar_hide_and_show() {
    let mut harness = pane_harness();
    harness.run();
    let view = harness.state().view_rect();
    harness.get_by_label("Hide Sidebar").click();
    harness.run();
    assert!(harness.state().sidebar_hidden());
    assert!(harness.query_by_label("Reset View").is_none());
    assert!(
        harness.state().view_rect().width() > view.width(),
        "the view takes the sidebar's room"
    );
    harness
        .get_by_role_and_label(Role::Button, "Show Sidebar")
        .click();
    harness.run();
    assert!(!harness.state().sidebar_hidden());
    assert!(harness.query_by_label("Reset View").is_some());
}

#[test]
fn test_visualizer_inspector_shows_bounds() -> TestResult {
    let harness = page_app(egui::vec2(1280.0, 800.0), 1.0, false)?;
    let mut harness = harness;
    harness.get_by_label("Inspector").click();
    harness.run();
    assert!(harness.query_by_label("Bounds X").is_some());
    assert!(
        harness
            .query_by_label_contains("60.00 to 170.00 mm")
            .is_some(),
        "X bounds of the cuts"
    );
    assert!(
        harness
            .query_by_label_contains("20.00 to 120.00 mm")
            .is_some()
    );
    Ok(())
}

#[test]
fn test_snapshot_visualizer_page_dark() -> TestResult {
    let mut harness = page_app(egui::vec2(1440.0, 900.0), 1.0, true)?;
    harness.run_steps(2);
    let options = SnapshotOptions::new()
        .threshold(1.0)
        .max_failed_pixels(40)
        .output_path(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots"));
    if let Err(error) = harness.try_snapshot_options("visualizer_page_dark", &options) {
        return Err(error.to_string().into());
    }
    Ok(())
}
