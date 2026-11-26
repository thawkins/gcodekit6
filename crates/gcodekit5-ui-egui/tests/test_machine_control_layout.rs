//! The Machine Control page in GTK's three columns: at 1280x800 (1x and 2x) the primary
//! controls (Connect, E-STOP, Stop, the jog pad and the override rows) are inside the window
//! without scrolling, and a snapshot of the page at 1440x900 dark.

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
