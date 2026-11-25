//! Designer canvas snapshots through wgpu: one per drawing tool, and a selection with its
//! handles.

use egui_kittest::{Harness, SnapshotOptions};
use gcodekit5_app::i18n::Translator;
use gcodekit5_ui_egui::panes::designer::DesignerPane;
use gcodekit5_ui_egui::panes::designer::canvas::DesignerTool;
use gcodekit5_ui_egui::theme;
use gcodekit5_ui_egui::workspace::{Pane, PaneContext};

use crate::test_designer_canvas::{STEP_DT, click, drag, ids, press, screen};

/// Snapshot options shared with the visualizer snapshots (see `test_visualizer.rs`).
fn snapshot_options() -> SnapshotOptions {
    SnapshotOptions::new()
        .threshold(1.0)
        .max_failed_pixels(40)
        .output_path(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots"))
}

/// The pane in a harness that renders through wgpu, dark theme.
fn gpu_harness() -> Harness<'static, DesignerPane> {
    let tr = Translator::new("en");
    let mut harness = Harness::builder()
        .with_size(egui::vec2(640.0, 400.0))
        .with_step_dt(STEP_DT)
        .with_theme(egui::Theme::Dark)
        .wgpu()
        .build_ui_state(
            move |ui, pane: &mut DesignerPane| {
                theme::install(ui.ctx());
                let mut requested = Vec::new();
                let mut ctx = PaneContext::new(&tr, &mut requested);
                pane.ui(ui, &mut ctx);
            },
            DesignerPane::new(),
        );
    harness.run();
    harness
}

/// One snapshot per drawing tool: the shape drawn by a drag from (40, 40) to (160, 140) mm,
/// then left unselected, on the grid and device bounds.
#[test]
fn test_snapshot_each_drawing_tool() {
    let tools = [
        (DesignerTool::Rectangle, "designer_rectangle"),
        (DesignerTool::Circle, "designer_circle"),
        (DesignerTool::Line, "designer_line"),
        (DesignerTool::Ellipse, "designer_ellipse"),
        (DesignerTool::Triangle, "designer_triangle"),
        (DesignerTool::Polygon, "designer_polygon"),
        (DesignerTool::Gear, "designer_gear"),
        (DesignerTool::Sprocket, "designer_sprocket"),
    ];
    let mut failures = Vec::new();
    for (tool, name) in tools {
        let mut h = gpu_harness();
        h.state_mut().set_tool(tool);
        let (from, to) = (screen(&h, 40.0, 40.0), screen(&h, 160.0, 140.0));
        drag(&mut h, from, to);
        assert_eq!(ids(h.state()).len(), 1, "{tool:?}");
        h.event(egui::Event::PointerGone);
        h.run();
        if let Err(error) = h.try_snapshot_options(name, &snapshot_options()) {
            failures.push(format!("{name}: {error}"));
        }
    }
    // A polyline, and a selection with its handles.
    let mut h = gpu_harness();
    h.state_mut().set_tool(DesignerTool::Polyline);
    for (x, y) in [(40.0, 40.0), (160.0, 60.0), (100.0, 140.0)] {
        let p = screen(&h, x, y);
        click(&mut h, p);
        h.run_steps(30);
    }
    press(&mut h, egui::Key::Enter);
    h.event(egui::Event::PointerGone);
    h.run();
    if let Err(error) = h.try_snapshot_options("designer_polyline", &snapshot_options()) {
        failures.push(format!("designer_polyline: {error}"));
    }
    let mut h = gpu_harness();
    h.state_mut().set_tool(DesignerTool::Rectangle);
    let (from, to) = (screen(&h, 40.0, 40.0), screen(&h, 160.0, 140.0));
    drag(&mut h, from, to);
    h.state_mut().set_tool(DesignerTool::Select);
    let p = screen(&h, 100.0, 90.0);
    click(&mut h, p);
    h.event(egui::Event::PointerGone);
    h.run();
    if let Err(error) = h.try_snapshot_options("designer_selection", &snapshot_options()) {
        failures.push(format!("designer_selection: {error}"));
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The toolbox in CNC mode with the stock settings open, and a tool picked.
#[test]
fn test_snapshot_toolbox_cnc() {
    use egui_kittest::kittest::Queryable;
    let mut h = gpu_harness();
    h.state_mut()
        .state_mut()
        .set_machine_mode(gcodekit5_designer::designer_state::MachineMode::Cnc3D);
    h.state_mut().set_tool(DesignerTool::Gear);
    h.get_by_label("Tool Settings").click();
    h.run();
    h.get_by_label("Stock Settings").click();
    h.run();
    h.event(egui::Event::PointerGone);
    h.run();
    if let Err(error) = h.try_snapshot_options("designer_toolbox_cnc", &snapshot_options()) {
        panic!("designer_toolbox_cnc: {error}");
    }
}

/// The Text tool: text placed by a click and the Add Text dialog, then left unselected.
/// The family is one no system has, so the model's bundled font draws it on every machine.
#[test]
fn test_snapshot_text_tool() {
    use egui_kittest::kittest::Queryable;
    use gcodekit5_designer::model::Shape;
    let mut h = gpu_harness();
    h.state_mut().set_tool(DesignerTool::Text);
    let p = screen(&h, 100.0, 90.0);
    click(&mut h, p);
    h.event(egui::Event::Text("GCode".to_string()));
    h.run();
    let size = h
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Size:")
        .rect()
        .center();
    click(&mut h, size);
    h.key_combination_modifiers(egui::Modifiers::COMMAND, &[egui::Key::A]);
    h.event(egui::Event::Text("90".to_string()));
    h.run();
    h.get_by_label("Add").click();
    h.run();
    {
        let state = h.state_mut().state_mut();
        for obj in state.canvas.shapes_mut() {
            if let Shape::Text(t) = &mut obj.shape {
                t.font_family = "GCodeKit5 Bundled Fallback".to_string();
            }
        }
        state.canvas.deselect_all();
    }
    assert_eq!(ids(h.state()).len(), 1);
    h.event(egui::Event::PointerGone);
    h.run();
    if let Err(error) = h.try_snapshot_options("designer_text", &snapshot_options()) {
        panic!("designer_text: {error}");
    }
}
