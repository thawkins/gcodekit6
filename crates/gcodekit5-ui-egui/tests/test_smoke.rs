//! Smoke test: the placeholder app renders its heading.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_ui_egui::GcodeKitApp;

#[test]
fn test_app_renders_empty_frame() {
    let mut app = GcodeKitApp::default();
    let mut harness = Harness::new_ui(|ui| app.ui(ui));
    harness.run();
    let _ = harness.get_by_label("GCodeKit");
}
