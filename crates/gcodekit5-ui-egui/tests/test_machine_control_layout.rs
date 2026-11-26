//! The Machine Control page in GTK's three columns: at 1280x800 (1x and 2x) the primary
//! controls (Connect, E-STOP, Stop, the jog pad and the override rows) are inside the window
//! without scrolling; in narrow windows and with a widened sidebar or editor column E-STOP
//! and the jog pad stay inside the centre column; and a snapshot of the page at 1440x900
//! dark.

use std::time::Duration;

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use egui_kittest::{Harness, SnapshotOptions};
use gcodekit5_ui_egui::GcodeKitApp;

use crate::machine_support::{loopback_config, loopback_state_with};

/// The app (disconnected, on the loopback host) in a harness of `size` points at `ppp`
/// pixels per point, with the theme installed, after two frames.
fn page_harness(size: egui::Vec2, ppp: f32) -> Harness<'static, GcodeKitApp> {
    let (state, _shared) = loopback_state_with(loopback_config(), Duration::from_millis(1));
    let mut harness = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(ppp)
        .with_theme(egui::Theme::Dark)
        .build_ui_state(
            |ui, app: &mut GcodeKitApp| {
                app.logic(ui.ctx());
                app.ui(ui);
            },
            GcodeKitApp::from_state(state),
        );
    gcodekit5_ui_egui::theme::install(&harness.ctx);
    harness.run_steps(3);
    harness
}

/// The labels of the controls that must be visible without scrolling.
const PRIMARY: [&str; 13] = [
    "E-STOP",
    "Jog X+",
    "Jog X-",
    "Jog Y+",
    "Jog Y-",
    "Jog Z+",
    "Jog Z-",
    "Decrease feed rate by 10%",
    "Increase feed rate by 10%",
    "Decrease spindle speed by 10%",
    "Increase spindle speed by 10%",
    "Stop spindle",
    "Rapid: 25%",
];

#[test]
fn test_primary_controls_visible_at_1280x800() {
    for ppp in [1.0, 2.0] {
        let harness = page_harness(egui::vec2(1280.0, 800.0), ppp);
        // The page ends where the status bar (with its eStop button) begins.
        let bottom = harness.get_by_label("eStop").rect().top();
        let page = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1280.0, bottom));
        let mut rects = vec![
            (
                "Connect",
                harness
                    .get_by_role_and_label(Role::Button, "Connect")
                    .rect(),
            ),
            (
                "Stop",
                harness.get_by_role_and_label(Role::Button, "Stop").rect(),
            ),
        ];
        rects.extend(PRIMARY.map(|label| (label, harness.get_by_label(label).rect())));
        for (label, rect) in rects {
            assert!(
                page.contains_rect(rect),
                "{label} at {rect:?} outside {page:?} at ppp {ppp}"
            );
        }
    }
}

#[test]
fn test_machine_control_page_has_three_columns() {
    let harness = page_harness(egui::vec2(1280.0, 800.0), 1.0);
    let connect = harness
        .get_by_role_and_label(Role::Button, "Connect")
        .rect();
    let jog = harness.get_by_label("Jog X-").rect();
    let estop = harness.get_by_label("E-STOP").rect();
    let editor = harness.get_by_label("G-Code Editor").rect();
    assert!(
        connect.right() < jog.left(),
        "jog pad not right of the sidebar"
    );
    assert!(
        estop.right() < editor.left(),
        "editor not right of the controls"
    );
    // The connection, the readout and the jog pad are side by side, not stacked.
    assert!(jog.top() < 800.0 && connect.top() < jog.top());
}

#[test]
fn test_snapshot_machine_control_page_dark() {
    let (state, _shared) = loopback_state_with(loopback_config(), Duration::from_millis(1));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .with_theme(egui::Theme::Dark)
        .wgpu()
        .build_ui_state(
            |ui, app: &mut GcodeKitApp| {
                app.logic(ui.ctx());
                app.ui(ui);
            },
            GcodeKitApp::from_state(state),
        );
    gcodekit5_ui_egui::theme::install(&harness.ctx);
    harness.run_steps(4);
    let options = SnapshotOptions::new()
        .threshold(1.0)
        .max_failed_pixels(40)
        .output_path(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots"));
    if let Err(error) = harness.try_snapshot_options("machine_control_page_dark", &options) {
        panic!("{error}");
    }
}

/// The left edge of the editor column: the resize edge left of its first widget, the
/// Search field.
fn editor_column_left(harness: &mut Harness<'static, GcodeKitApp>) -> f32 {
    let search = harness.get_by_label("Search...").rect().left();
    find_edge(harness, search, search - 40.0)
}

/// Asserts that the primary controls lie inside the centre column at `size`: right of the
/// sidebar (Connect), left of the editor column's edge, and above the status bar.
fn assert_centre_controls_clear(harness: &mut Harness<'static, GcodeKitApp>, size: egui::Vec2) {
    let sidebar_right = harness
        .get_by_role_and_label(Role::Button, "Connect")
        .rect()
        .right();
    let editor_left = editor_column_left(harness);
    // The page ends where the status bar (with its eStop button) begins.
    let bottom = harness.get_by_label("eStop").rect().top();
    harness.hover_at(egui::pos2(0.0, 0.0));
    harness.run_steps(1);
    let zero_actions = ["Zero All Axes", "Go to Work Zero", "Include Z"];
    for label in PRIMARY.into_iter().chain(zero_actions) {
        let rect = harness.get_by_label(label).rect();
        assert!(
            rect.left() > sidebar_right && rect.right() < editor_left,
            "{label} at {rect:?} not between the sidebar ({sidebar_right}) and the editor \
             column ({editor_left}) at {size:?}"
        );
        assert!(
            rect.right() <= size.x,
            "{label} at {rect:?} outside the window at {size:?}"
        );
        // In the narrowest windows the stacked rows scroll, but E-STOP moves to the top.
        if (size.x >= 800.0 || label == "E-STOP") && !zero_actions.contains(&label) {
            assert!(
                rect.bottom() <= bottom,
                "{label} at {rect:?} below the page (bottom {bottom}) at {size:?}"
            );
        }
    }
}

#[test]
fn test_estop_clear_of_the_editor_column_in_narrow_windows() {
    // 640 is the app's minimum window width.
    for width in [640.0, 800.0, 960.0, 1280.0] {
        let size = egui::vec2(width, 800.0);
        let mut harness = page_harness(size, 1.0);
        assert_centre_controls_clear(&mut harness, size);
    }
}

/// Height at which the panel edges are found and dragged: beside the jog pad's row.
const DRAG_Y: f32 = 300.0;

/// The x in `from..to` (scanned by 1 pt) where the pointer shows a horizontal resize cursor:
/// a panel's resize edge.
fn find_edge(harness: &mut Harness<'static, GcodeKitApp>, from: f32, to: f32) -> f32 {
    let step = if to > from { 1.0 } else { -1.0 };
    let mut x = from;
    while (to - x) * step > 0.0 {
        harness.hover_at(egui::pos2(x, DRAG_Y));
        harness.run_steps(1);
        let cursor = harness.output().platform_output.cursor_icon;
        if matches!(
            cursor,
            egui::CursorIcon::ResizeHorizontal
                | egui::CursorIcon::ResizeColumn
                | egui::CursorIcon::ResizeEast
                | egui::CursorIcon::ResizeWest
        ) {
            return x;
        }
        x += step;
    }
    panic!("no resize edge between {from} and {to}");
}

/// Drags the vertical panel edge at `x` by `dx` points.
fn drag_edge(harness: &mut Harness<'static, GcodeKitApp>, x: f32, dx: f32) {
    let press = |pos: egui::Pos2, pressed: bool| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    harness.hover_at(egui::pos2(x, DRAG_Y));
    harness.run_steps(1);
    harness.event(press(egui::pos2(x, DRAG_Y), true));
    harness.run_steps(1);
    for step in 1..=10 {
        harness.hover_at(egui::pos2(x + dx * step as f32 / 10.0, DRAG_Y));
        harness.run_steps(1);
    }
    harness.event(press(egui::pos2(x + dx, DRAG_Y), false));
    harness.run_steps(3);
}

#[test]
fn test_estop_clear_of_the_editor_column_with_a_wide_sidebar_and_editor() {
    let size = egui::vec2(1280.0, 800.0);
    let mut harness = page_harness(size, 1.0);
    // Widen the sidebar as far as it goes, then the editor column.
    let jog_before = harness.get_by_label("Jog X-").rect().left();
    let connect = harness
        .get_by_role_and_label(Role::Button, "Connect")
        .rect()
        .right();
    let edge = find_edge(&mut harness, connect, connect + 30.0);
    drag_edge(&mut harness, edge, 900.0);
    let jog_after = harness.get_by_label("Jog X-").rect().left();
    assert!(
        jog_after > jog_before + 20.0,
        "the sidebar edge did not move ({jog_before} -> {jog_after})"
    );
    assert_centre_controls_clear(&mut harness, size);
    let editor_left = editor_column_left(&mut harness);
    drag_edge(&mut harness, editor_left, -900.0);
    let editor_after = editor_column_left(&mut harness);
    assert!(
        editor_after < editor_left - 50.0,
        "the editor column edge did not move ({editor_left} -> {editor_after})"
    );
    assert_centre_controls_clear(&mut harness, size);
}
