//! Smoke test: the app renders its docked workspace and themes its widgets.

use craft_window::policy::{Backend, Desktop, DesktopKind, Os, TitleBarPref, decide};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit6_settings::SettingsManager;
use gcodekit6_settings::config::Config;
use gcodekit6_ui_egui::{AppState, GcodeKitApp};

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
    let mut app = GcodeKitApp::from_state(AppState::new(
        SettingsManager::detached(Config::default()),
        native_decision(),
    ));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui(|ui| app.ui(ui));
    harness.run();
    let _ = harness.get_by_role_and_label(Role::Tab, "Machine Control");
}

#[test]
fn test_dark_palette_applied() {
    let visuals = gcodekit6_ui_egui::theme::visuals(egui::Theme::Dark);
    assert_eq!(
        visuals.widgets.hovered.bg_fill,
        gcodekit6_ui_egui::theme::DARK.widgets.hovered
    );
}

#[test]
fn test_theme_override_beats_desktop() {
    use gcodekit6_settings::config::Theme;
    use gcodekit6_ui_egui::theme::resolve;
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

/// The main window opens maximized by default, restoring to 1280x800.
#[test]
fn test_main_window_opens_maximized() {
    let viewport = gcodekit6_ui_egui::main_viewport();
    assert_eq!(viewport.maximized, Some(true));
    assert_eq!(viewport.inner_size, Some(egui::vec2(1280.0, 800.0)));
}

/// GTK's warning when the settings could not be loaded: shown at start with the file and the
/// problem, until OK; none when they loaded cleanly.
#[test]
fn test_settings_problem_is_shown_at_start_until_ok() {
    let clean = GcodeKitApp::from_state(AppState::new(
        SettingsManager::detached(Config::default()),
        native_decision(),
    ));
    let mut h = crate::machine_support::app_harness(clean);
    h.run();
    assert!(h.query_by_label("Settings could not be loaded").is_none());
    let state = AppState::new(
        SettingsManager::detached(Config::default()),
        native_decision(),
    )
    .with_settings_problem(Some(
        "/x/config.toml\n\nconfig.toml was damaged".to_string(),
    ));
    let mut h = crate::machine_support::app_harness(GcodeKitApp::from_state(state));
    h.run();
    let _ = h.get_by_label("Settings could not be loaded");
    assert!(
        h.query_by_label_contains("config.toml was damaged")
            .is_some()
    );
    assert!(
        h.query_by_label_contains("Default settings are in use.")
            .is_some()
    );
    h.get_by_label("OK").click();
    h.run();
    assert!(h.query_by_label("Settings could not be loaded").is_none());
}
