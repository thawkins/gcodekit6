//! Smoke test: the app renders its docked workspace and themes its widgets.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::{AppState, GcodeKitApp};

#[test]
fn test_app_renders_workspace() {
    let mut app = GcodeKitApp::from_state(AppState::from_config(Config::default()));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui(|ui| app.ui(ui));
    harness.run();
    let _ = harness.get_by_role_and_label(Role::Tab, "Machine Control");
}

#[test]
fn test_dark_palette_applied() {
    let visuals = gcodekit5_ui_egui::theme::visuals(egui::Theme::Dark);
    assert_eq!(
        visuals.widgets.hovered.bg_fill,
        gcodekit5_ui_egui::theme::DARK.widgets.hovered
    );
}

#[test]
fn test_theme_override_beats_desktop() {
    use gcodekit5_settings::config::Theme;
    use gcodekit5_ui_egui::theme::resolve;
    assert_eq!(
        resolve(Theme::Light, Some(egui::Theme::Dark)),
        egui::Theme::Light
    );
    assert_eq!(
        resolve(Theme::Dark, Some(egui::Theme::Light)),
        egui::Theme::Dark
    );
    assert_eq!(
        resolve(Theme::System, Some(egui::Theme::Light)),
        egui::Theme::Light
    );
}

#[test]
fn test_integrated_frame_draws_caption_buttons() {
    use craft_window::policy::{CaptionFamily, Captions, Frame};
    let mut state = AppState::from_config(Config::default());
    state.frame = Frame::Integrated(Captions {
        family: CaptionFamily::Generic,
        layout: craft_window::Layout::default(),
    });
    let mut app = GcodeKitApp::from_state(state);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui(|ui| app.ui(ui));
    harness.run();
    for label in ["Minimize", "Maximize", "Close"] {
        let _ = harness.get_by_role_and_label(Role::Button, label);
    }
    let _ = harness.get_by_role_and_label(Role::Tab, "Machine Control");
}
