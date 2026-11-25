//! HiDPI hit-testing: the same click, expressed in egui points, reaches the same nav cube
//! face, editor line and jog button at pixels-per-point 1.0, 1.5 and 2.0.

use std::time::Duration;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::test_support::Wrote;
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::panes::editor::EditorPane;
use gcodekit5_ui_egui::panes::visualizer::{StdView, VisualizerPane};
use gcodekit5_ui_egui::workspace::{Pane, PaneContext};

use crate::machine_support::{connect, loopback_config, loopback_state_with, wait_until};
use crate::test_machine_pane::{pointer_button, report_idle};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The scales every test runs at.
const SCALES: [f32; 3] = [1.0, 1.5, 2.0];

/// Clicks `pos` (in points) with the real pointer events.
fn click_at<S>(harness: &mut Harness<'_, S>, pos: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(pos));
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    harness.run();
}

/// The nav cube faces and the view each must select.
const FACES: [(&str, StdView); 9] = [
    ("Isometric (NW)", StdView::IsoNw),
    ("Top View", StdView::Top),
    ("Isometric (NE)", StdView::IsoNe),
    ("Left View", StdView::Left),
    ("Front View", StdView::Front),
    ("Right View", StdView::Right),
    ("Isometric (SW)", StdView::IsoSw),
    ("Bottom View", StdView::Bottom),
    ("Isometric (SE)", StdView::IsoSe),
];

#[test]
fn test_nav_cube_faces_hit_at_every_scale() -> TestResult {
    let _guard = crate::global_env_lock();
    for ppp in SCALES {
        let tr = Translator::new("en");
        let mut harness = Harness::builder()
            .with_size(egui::vec2(480.0, 360.0))
            .with_pixels_per_point(ppp)
            .build_ui_state(
                move |ui, pane: &mut VisualizerPane| {
                    let mut requested = Vec::new();
                    let mut ctx = PaneContext::new(&tr, &mut requested);
                    pane.ui(ui, &mut ctx);
                },
                VisualizerPane::new(),
            );
        harness.run();
        for (name, view) in FACES {
            // Start away from the target so a missed click is visible.
            harness
                .state_mut()
                .apply(gcodekit5_ui_egui::panes::visualizer::NavAction::View(
                    if view == StdView::Top {
                        StdView::Front
                    } else {
                        StdView::Top
                    },
                ));
            harness.run();
            let rect = harness.get_by_label(name).rect();
            // Centre and a point just inside each corner: the edges of the hit area.
            let inset = egui::vec2(1.0, 1.0);
            let spots = [
                rect.center(),
                rect.min + inset,
                rect.max - inset,
                egui::pos2(rect.max.x - 1.0, rect.min.y + 1.0),
                egui::pos2(rect.min.x + 1.0, rect.max.y - 1.0),
            ];
            for spot in spots {
                harness
                    .state_mut()
                    .apply(gcodekit5_ui_egui::panes::visualizer::NavAction::View(
                        if view == StdView::Top {
                            StdView::Front
                        } else {
                            StdView::Top
                        },
                    ));
                harness.run();
                click_at(&mut harness, spot);
                assert_eq!(
                    harness.state().camera().angles(),
                    view.angles(),
                    "{name} at {spot:?}, ppp {ppp}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn test_editor_click_selects_same_line_at_every_scale() -> TestResult {
    let _guard = crate::global_env_lock();
    let text: String = (0..40).map(|i| format!("G1 X{i}\n")).collect();
    for ppp in SCALES {
        let tr = Translator::new("en");
        let mut pane = EditorPane::new();
        pane.buffer_mut().set_text(text.clone());
        let mut harness = Harness::builder()
            .with_size(egui::vec2(480.0, 600.0))
            .with_pixels_per_point(ppp)
            .build_ui_state(
                move |ui, pane: &mut EditorPane| {
                    let mut requested = Vec::new();
                    let mut ctx = PaneContext::new(&tr, &mut requested);
                    pane.ui(ui, &mut ctx);
                },
                pane,
            );
        harness.run();
        let field =
            harness.get_by_role_and_label(egui::accesskit::Role::TextInput, "G-Code Editor");
        let rect = field.rect();
        let lines = harness.state().buffer().line_count();
        let row = rect.height() / lines as f32;
        for target in [0usize, 1, 7, 19, 33] {
            // The line just before, so the click has to move the current line.
            let other = if target == 0 { 5 } else { 0 };
            harness.state_mut().buffer_mut().set_current_line(other);
            let y = rect.top() + (target as f32 + 0.5) * row;
            click_at(&mut harness, egui::pos2(rect.left() + 20.0, y));
            assert_eq!(
                harness.state().buffer().current_line(),
                target,
                "text row {target}, ppp {ppp}"
            );
            // The gutter row of the same line: well inside the gutter (a click within the hit
            // radius of the text field would reach the field instead).
            harness.state_mut().buffer_mut().set_current_line(other);
            click_at(&mut harness, egui::pos2(rect.left() - 20.0, y));
            assert_eq!(
                harness.state().buffer().current_line(),
                target,
                "gutter row {target}, ppp {ppp}"
            );
        }
    }
    Ok(())
}

#[test]
fn test_jog_buttons_send_same_line_at_every_scale() -> TestResult {
    let _guard = crate::global_env_lock();
    let expected = [
        ("Jog X+", "$J=G91 G21 X1.00 F1000"),
        ("Jog X-", "$J=G91 G21 X-1.00 F1000"),
        ("Jog Y+", "$J=G91 G21 Y1.00 F1000"),
        ("Jog Y-", "$J=G91 G21 Y-1.00 F1000"),
    ];
    for ppp in SCALES {
        let (mut state, shared) = loopback_state_with(loopback_config(), Duration::from_millis(1));
        connect(&mut state);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1280.0, 800.0))
            .with_pixels_per_point(ppp)
            .build_ui_state(
                |ui, app: &mut GcodeKitApp| {
                    app.logic(ui.ctx());
                    app.ui(ui);
                },
                GcodeKitApp::from_state(state),
            );
        harness.step();
        harness.step();
        report_idle(&mut harness, &shared);
        for (label, line) in expected {
            let before = shared.log().len();
            let rect = harness.get_by_label(label).rect();
            pointer_button(&harness, rect.center(), true);
            harness.step();
            pointer_button(&harness, rect.center(), false);
            harness.step();
            harness.step();
            let want = Wrote::Line(line.to_string());
            assert!(
                wait_until(|| shared.log().iter().skip(before).any(|w| *w == want)),
                "{label} at ppp {ppp}: {:?}",
                shared.log()
            );
        }
    }
    Ok(())
}

#[test]
fn test_designer_click_selects_same_shape_at_every_scale() -> TestResult {
    use crate::test_designer_canvas::{click, harness_at, pane_with_rects, screen};
    let _guard = crate::global_env_lock();
    let rects = [
        (20.0, 20.0, 30.0, 30.0),
        (70.0, 20.0, 30.0, 30.0),
        (20.0, 70.0, 30.0, 30.0),
    ];
    for ppp in SCALES {
        let mut harness = harness_at(pane_with_rects(&rects), ppp);
        let ids: Vec<u64> = harness
            .state()
            .state()
            .canvas
            .shape_store
            .draw_order_iter()
            .collect();
        for (i, &(x, y, w, h)) in rects.iter().enumerate() {
            // The centre and a point just inside the far corner of each shape.
            for (wx, wy) in [(x + w / 2.0, y + h / 2.0), (x + w - 0.5, y + h - 0.5)] {
                let pos = screen(&harness, wx as f32, wy as f32);
                click(&mut harness, pos);
                let picked: Vec<u64> = harness
                    .state()
                    .state()
                    .canvas
                    .shapes()
                    .filter(|o| o.selected)
                    .map(|o| o.id)
                    .collect();
                assert_eq!(picked, vec![ids[i]], "shape {i} at ({wx}, {wy}), ppp {ppp}");
            }
        }
    }
    Ok(())
}

#[test]
fn test_designer_layers_row_click_selects_same_shape_at_every_scale() -> TestResult {
    use crate::test_designer_layers::{both_at, click_row, order, sel, three};
    let _guard = crate::global_env_lock();
    for ppp in SCALES {
        let mut harness = both_at(three(), ppp);
        let ids = order(&harness);
        for id in [ids[2], ids[0], ids[1]] {
            click_row(&mut harness, id, egui::Modifiers::NONE);
            assert_eq!(sel(&harness), vec![id], "row {id} at ppp {ppp}");
        }
    }
    Ok(())
}

#[test]
fn test_designer_toolbox_button_click_picks_same_tool_at_every_scale() -> TestResult {
    use crate::test_designer_canvas::{click, harness_at};
    use gcodekit5_ui_egui::panes::designer::DesignerPane;
    use gcodekit5_ui_egui::panes::designer::canvas::DesignerTool;
    let _guard = crate::global_env_lock();
    for ppp in SCALES {
        let mut harness = harness_at(DesignerPane::new(), ppp);
        for (label, tool) in [
            ("Gear", DesignerTool::Gear),
            ("Rectangle", DesignerTool::Rectangle),
            ("Polyline", DesignerTool::Polyline),
        ] {
            let pos = harness.get_by_label(label).rect().center();
            click(&mut harness, pos);
            assert_eq!(harness.state().tool(), tool, "{label} at ppp {ppp}");
        }
    }
    Ok(())
}
