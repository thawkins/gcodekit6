//! Smoke test: the app renders its docked workspace and themes its widgets.

use craft_window::policy::{Backend, Desktop, DesktopKind, Os, TitleBarPref, decide};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::{AppState, GcodeKitApp};

/// A decision with the desktop drawing the frame, built without probing the desktop.
pub(crate) fn native_decision() -> craft_window::policy::Decision {
    let desktop = Desktop {
        kind: DesktopKind::Gnome,
        ssd: Some(false),
        gtk_theme: None,
        icon_theme: None,
    };
    decide(
        Os::Linux,
        Backend::Wayland,
        &desktop,
        TitleBarPref::System,
        None,
    )
}

#[test]
fn test_app_renders_workspace() {
    let mut app = GcodeKitApp::from_state(AppState::new(Config::default(), native_decision()));
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
