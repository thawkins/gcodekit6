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

#[test]
fn test_widget_palette_applied() {
    let mut widgets = egui::Visuals::dark().widgets;
    gcodekit5_ui_egui::app::apply_widget_palette(&mut widgets);
    assert_eq!(
        widgets.hovered.bg_fill,
        gcodekit5_ui_egui::app::WIDGET_PALETTE.hovered
    );
}
