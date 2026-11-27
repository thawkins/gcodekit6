//! The Machine Control sidebar keeps its width: nothing inside it is wider than the panel, so
//! egui's resizable panel does not grow a little every frame ("drifts sideways").

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit6_settings::SettingsManager;
use gcodekit6_settings::config::Config;
use gcodekit6_ui_egui::{AppState, GcodeKitApp};

use crate::machine_support::app_harness;

/// The app on its first page (Machine Control) with `config`.
fn app(config: Config) -> Harness<'static, GcodeKitApp> {
    let state = AppState::new(
        SettingsManager::detached(config),
        crate::test_smoke::native_decision(),
    );
    let mut h = app_harness(GcodeKitApp::from_state(state));
    h.run_steps(2);
    h
}

/// The right edge of the sidebar's Connect button after `frames` more frames.
fn connect_right(h: &mut Harness<'static, GcodeKitApp>, frames: usize) -> f32 {
    h.run_steps(frames);
    h.get_by_label("Connect").rect().max.x
}

#[test]
fn test_machine_sidebar_width_is_stable() {
    let mut h = app(Config::default());
    let first = connect_right(&mut h, 1);
    let later = connect_right(&mut h, 300);
    assert!(
        (later - first).abs() < 0.5,
        "sidebar drifted from {first} to {later} over 300 frames"
    );
}

/// The same with a network connection (no refresh button; host and port fields).
#[test]
fn test_machine_sidebar_width_is_stable_for_network() {
    let mut h = app(crate::machine_support::loopback_config());
    let first = connect_right(&mut h, 1);
    let later = connect_right(&mut h, 300);
    assert!(
        (later - first).abs() < 0.5,
        "sidebar drifted from {first} to {later} over 300 frames"
    );
}
