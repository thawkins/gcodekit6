//! The Designer page in GTK's three columns: at 1280x800 (1x and 2x) the toolbox, Generate
//! G-Code, the canvas and the inspector are inside the window side by side; the canvas
//! toolbar and the inspector's Hide button work; and a snapshot of the page at 1440x900
//! dark.

use egui_kittest::kittest::Queryable;
use egui_kittest::{Harness, SnapshotOptions};
use gcodekit5_designer::model::{DesignCircle, DesignRectangle, Point, Shape};
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::workspace::PaneId;
use gcodekit5_ui_egui::{AppState, GcodeKitApp};

/// The tool buttons' accessible names, in GTK's grid order.
const TOOLS: [&str; 12] = [
    "Select",
    "Pan",
    "Rectangle",
    "Circle",
    "Line",
    "Ellipse",
    "Triangle",
    "Polygon",
    "Polyline",
    "Text",
    "Gear",
    "Sprocket",
];

/// The app on the Designer page with a rectangle (selected) and a circle, in a harness of
/// `size` points at `ppp` pixels per point, dark, with the theme installed.
fn page_app(size: egui::Vec2, ppp: f32, wgpu: bool) -> Harness<'static, GcodeKitApp> {
    let app = GcodeKitApp::from_state(AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    ));
    let builder = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(ppp)
        .with_theme(egui::Theme::Dark);
    let builder = if wgpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_ui_state(
        |ui, app: &mut GcodeKitApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        app,
    );
    gcodekit5_ui_egui::theme::install(&harness.ctx);
    harness
        .state_mut()
        .workspace_mut()
        .show_pane(PaneId::Designer);
    {
        let designer = std::rc::Rc::clone(harness.state().designer());
        let mut d = designer.borrow_mut();
        let state = d.state_mut();
        let rect = state.add_shape_with_undo(Shape::Rectangle(DesignRectangle::new(
            30.0, 40.0, 80.0, 50.0,
        )));
        state.add_shape_with_undo(Shape::Circle(DesignCircle::new(
            Point::new(170.0, 120.0),
            35.0,
        )));
        if let Some(obj) = state.canvas.get_shape_mut(rect) {
            obj.selected = true;
        }
        state.canvas.selection_manager.set_selected_id(Some(rect));
    }
    harness.run_steps(4);
    harness
}

/// The canvas's screen rect on the last frame.
fn canvas(harness: &Harness<'_, GcodeKitApp>) -> egui::Rect {
    harness.state().designer().borrow().view().rect()
}

#[test]
fn test_designer_primary_controls_visible_at_1280x800() {
    for ppp in [1.0, 2.0] {
        let harness = page_app(egui::vec2(1280.0, 800.0), ppp, false);
        let bottom = harness.get_by_label("eStop").rect().top();
        let page = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1280.0, bottom));
        let mut labels = TOOLS.to_vec();
        labels.extend([
            "Shape Gallery…",
            "Generate G-Code",
            "Inspector",
            "Objects",
            "Zoom In",
            "Zoom Out",
        ]);
        for label in labels {
            let rect = harness.get_by_label(label).rect();
            assert!(
                page.contains_rect(rect),
                "{label} at {rect:?} outside {page:?} at ppp {ppp}"
            );
        }
        let canvas = canvas(&harness);
        assert!(
            canvas.width() > 400.0 && page.contains_rect(canvas),
            "canvas {canvas:?} at ppp {ppp}"
        );
    }
}

#[test]
fn test_designer_page_has_three_columns() {
    let harness = page_app(egui::vec2(1280.0, 800.0), 1.0, false);
    let toolbox = harness.get_by_label("Generate G-Code").rect();
    let canvas = canvas(&harness);
    let inspector = harness.get_by_label("Inspector").rect();
    let objects = harness.get_by_label("Objects").rect();
    assert!(
        toolbox.right() <= canvas.left(),
        "toolbox not left of canvas"
    );
    assert!(canvas.right() <= inspector.left(), "inspector not right");
    assert!(
        inspector.top() < objects.top(),
        "objects not under inspector"
    );
}

#[test]
fn test_designer_tools_form_a_three_column_grid() {
    let harness = page_app(egui::vec2(1280.0, 800.0), 1.0, false);
    let rects: Vec<egui::Rect> = TOOLS
        .iter()
        .map(|t| harness.get_by_label(t).rect())
        .collect();
    for row in rects.chunks(3) {
        assert!((row[0].top() - row[2].top()).abs() < 1.0, "row not level");
        assert!(row[0].right() < row[1].left() && row[1].right() < row[2].left());
    }
    for rect in &rects {
        assert!(
            (rect.width() - rect.height()).abs() < 1.0,
            "not square: {rect:?}"
        );
    }
    assert!(rects[3].top() > rects[0].bottom(), "four rows");
}

#[test]
fn test_designer_canvas_toolbar_zooms() {
    let mut harness = page_app(egui::vec2(1280.0, 800.0), 1.0, false);
    let zoom = |h: &Harness<'_, GcodeKitApp>| h.state().designer().borrow().view().zoom();
    let before = zoom(&harness);
    harness.get_by_label("Zoom In").click();
    harness.run();
    let zoomed = zoom(&harness);
    assert!(zoomed > before, "{before} -> {zoomed}");
    harness.get_by_label("Zoom Out").click();
    harness.run();
    assert!(zoom(&harness) < zoomed);
    for label in [
        "Fit to Content",
        "Fit to Viewport",
        "Fit to Device Working Area",
    ] {
        harness.get_by_label(label).click();
        harness.run();
    }
}

#[test]
fn test_designer_status_line_shows_zoom_and_units() {
    let harness = page_app(egui::vec2(1280.0, 800.0), 1.0, false);
    assert!(harness.query_by_label_contains("Zoom: ").is_some());
    assert!(harness.query_by_label_contains("Cursor: X ").is_some());
}

/// GTK's "?" button on the canvas toolbar lists the canvas's keys in a popover.
#[test]
fn test_designer_shortcuts_popover_lists_the_canvas_keys() {
    let mut harness = page_app(egui::vec2(1280.0, 800.0), 1.0, false);
    assert!(harness.query_by_label("Designer shortcuts").is_none());
    harness.get_by_label("?").click();
    harness.run();
    let _ = harness.get_by_label("Designer shortcuts");
    let tr = gcodekit5_app::i18n::Translator::new("en");
    for (keys, msgid) in gcodekit5_ui_egui::panes::designer::canvas::overlay::SHORTCUTS {
        let line =
            gcodekit5_ui_egui::panes::designer::canvas::overlay::shortcut_line(keys, msgid, &tr);
        assert!(harness.query_by_label(&line).is_some(), "{line}");
    }
    let _ = harness.get_by_label("Right click for context menu");
}

/// GTK's status line names the snap and the Shift constraint: "Snap / Constraint".
#[test]
fn test_designer_status_line_names_snap_and_constraint() {
    use gcodekit5_ui_egui::panes::designer::canvas::overlay::{Status, status_text};
    let tr = gcodekit5_app::i18n::Translator::new("en");
    let mut status = Status {
        zoom: 1.0,
        center: (0.0, 0.0),
        cursor: (0.0, 0.0),
        snap: true,
        constraint: true,
        units: gcodekit5_core::units::MeasurementSystem::Metric,
    };
    assert!(status_text(&status, &tr).contains("  Snap / Constraint  "));
    status.snap = false;
    let text = status_text(&status, &tr);
    assert!(
        text.contains("  Constraint  ") && !text.contains("Snap"),
        "{text}"
    );
    status.constraint = false;
    assert!(!status_text(&status, &tr).contains("Constraint"));
}

#[test]
fn test_designer_inspector_hide_and_unhide() {
    let mut harness = page_app(egui::vec2(1280.0, 800.0), 1.0, false);
    let narrow = canvas(&harness).width();
    harness.get_by_label("Hide Properties").click();
    harness.run();
    assert!(harness.query_by_label("Inspector").is_none());
    assert!(canvas(&harness).width() > narrow, "canvas takes the room");
    harness.get_by_label("Unhide Properties").click();
    harness.run();
    assert!(harness.query_by_label("Inspector").is_some());
}

#[test]
fn test_snapshot_designer_page_dark() {
    let mut harness = page_app(egui::vec2(1440.0, 900.0), 1.0, true);
    harness.run_steps(2);
    let options = SnapshotOptions::new()
        .threshold(1.0)
        .max_failed_pixels(40)
        .output_path(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots"));
    if let Err(error) = harness.try_snapshot_options("designer_page_dark", &options) {
        panic!("{error}");
    }
}
