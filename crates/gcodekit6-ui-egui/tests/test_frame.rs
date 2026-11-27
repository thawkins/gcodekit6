//! The window frame follows craft-window's decision: no caption buttons when the desktop
//! draws the frame, buttons in the decided layout when the app does, and resize edges that
//! yield to the buttons. Every decision is built with `policy::decide` and explicit inputs.

use craft_window::Layout;
use craft_window::policy::{
    Backend, Decision, Desktop, DesktopKind, Frame, Os, TitleBarPref, decide,
};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit6_settings::SettingsManager;
use gcodekit6_settings::config::{Config, UiSettings};
use gcodekit6_ui_egui::frame::title_bar_pref;
use gcodekit6_ui_egui::{AppState, GcodeKitApp};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);
const CAPTIONS: [&str; 3] = ["Minimize", "Maximize", "Close"];

fn desktop(kind: DesktopKind) -> Desktop {
    Desktop {
        kind,
        ssd: Some(false),
        gtk_theme: None,
        icon_theme: None,
    }
}

fn app(decision: Decision) -> GcodeKitApp {
    GcodeKitApp::from_state(AppState::new(
        SettingsManager::detached(Config::default()),
        decision,
    ))
}

/// A harness drawing `app` over the whole window with no margin, as eframe's root `Ui` is
/// (kittest's own root has an 8 pt outer margin).
fn harness(app: &mut GcodeKitApp) -> Harness<'_> {
    let mut harness = Harness::builder().with_size(SIZE).build_ui(move |ui| {
        let window = ui.ctx().content_rect();
        ui.scope_builder(egui::UiBuilder::new().max_rect(window), |ui| {
            ui.set_clip_rect(window);
            app.ui(ui);
        });
    });
    harness.run();
    harness
}

/// The commands the root viewport received in the last frame.
fn commands(harness: &Harness<'_>) -> Vec<egui::ViewportCommand> {
    harness
        .output()
        .viewport_output
        .get(&egui::ViewportId::ROOT)
        .map(|v| v.commands.clone())
        .unwrap_or_default()
}

/// Presses at `from`, moves to `to` and releases, collecting every command sent meanwhile.
fn gesture(
    harness: &mut Harness<'_>,
    from: egui::Pos2,
    to: egui::Pos2,
) -> Vec<egui::ViewportCommand> {
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::default(),
    };
    let mut sent = Vec::new();
    for event in [
        egui::Event::PointerMoved(from),
        button(from, true),
        egui::Event::PointerMoved(to),
        button(to, false),
    ] {
        harness.event(event);
        harness.step();
        sent.extend(commands(harness));
    }
    sent
}

#[test]
fn test_system_title_bar_setting_maps_to_system_pref() {
    let mut ui = UiSettings::default();
    assert_eq!(title_bar_pref(&ui), TitleBarPref::Integrated);
    ui.use_system_title_bar = true;
    assert_eq!(title_bar_pref(&ui), TitleBarPref::System);
}

#[test]
fn test_native_frame_draws_no_caption_buttons() {
    let decision = decide(
        Os::Linux,
        Backend::Wayland,
        &desktop(DesktopKind::Gnome),
        TitleBarPref::System,
        None,
    );
    assert_eq!(decision.frame, Frame::Native);
    let mut app = app(decision);
    let harness = harness(&mut app);
    for label in CAPTIONS {
        assert!(
            harness
                .query_by_role_and_label(Role::Button, label)
                .is_none()
        );
    }
    let _ = harness.get_by_role_and_label(Role::Tab, "Machine Control");
}

#[test]
fn test_left_layout_draws_buttons_left_in_order() {
    let mut decision = decide(
        Os::Linux,
        Backend::Wayland,
        &desktop(DesktopKind::Gnome),
        TitleBarPref::Integrated,
        None,
    );
    // decide() gives each family's default layout; decide_for() would read GNOME's
    // `button-layout`, which is simulated here.
    let Frame::Integrated(captions) = &mut decision.frame else {
        panic!("GNOME on Wayland without SSD should get the app's bar");
    };
    captions.layout = Layout::parse("close,minimize,maximize:");
    let mut app = app(decision);
    let harness = harness(&mut app);
    let x: Vec<f32> = ["Close", "Minimize", "Maximize"]
        .iter()
        .map(|l| {
            harness
                .get_by_role_and_label(Role::Button, l)
                .rect()
                .center()
                .x
        })
        .collect();
    assert!(
        x[0] < x[1] && x[1] < x[2],
        "buttons out of layout order: {x:?}"
    );
    assert!(x[2] < SIZE.x / 2.0, "buttons not on the left: {x:?}");
}

#[test]
fn test_close_caption_at_corner_closes_not_resizes() {
    let decision = decide(
        Os::Windows,
        Backend::X11,
        &desktop(DesktopKind::Other),
        TitleBarPref::Integrated,
        None,
    );
    let mut app = app(decision);
    let mut harness = harness(&mut app);
    let close = harness.get_by_role_and_label(Role::Button, "Close").rect();
    assert!(
        close.max.x >= SIZE.x - 1.0 && close.min.y <= 1.0,
        "close not in the corner: {close:?}"
    );
    // Inside both the close button and the north-east resize corner.
    let corner = egui::pos2(SIZE.x - 2.0, 2.0);
    let sent = gesture(&mut harness, corner, corner);
    assert!(
        sent.contains(&egui::ViewportCommand::Close),
        "close not requested: {sent:?}"
    );
    assert!(
        !sent
            .iter()
            .any(|c| matches!(c, egui::ViewportCommand::BeginResize(_))),
        "resize started: {sent:?}"
    );
}

#[test]
fn test_edge_drag_resizes_window() {
    let decision = decide(
        Os::Windows,
        Backend::X11,
        &desktop(DesktopKind::Other),
        TitleBarPref::Integrated,
        None,
    );
    let mut app = app(decision);
    let mut harness = harness(&mut app);
    let edge = egui::pos2(SIZE.x / 2.0, SIZE.y - 2.0);
    let sent = gesture(&mut harness, edge, edge + egui::vec2(0.0, 20.0));
    assert!(
        sent.contains(&egui::ViewportCommand::BeginResize(
            egui::ResizeDirection::South
        )),
        "no resize from the bottom edge: {sent:?}"
    );
}

/// An environment with exactly the named variable set.
fn env_with(set: &'static str) -> impl Fn(&str) -> Option<std::ffi::OsString> {
    move |name| (name == set).then(|| std::ffi::OsString::from("app"))
}

#[test]
fn titlebar_prefix_reads_gcodekit6_by_default() {
    let prefix = gcodekit6_ui_egui::titlebar_prefix(&|_| None);
    assert_eq!(prefix.var_name(), "GCODEKIT6_TITLEBAR");
    let prefix = gcodekit6_ui_egui::titlebar_prefix(&env_with("GCODEKIT6_TITLEBAR"));
    assert_eq!(prefix.var_name(), "GCODEKIT6_TITLEBAR");
}

#[test]
fn titlebar_prefix_falls_back_to_gcodekit5_variable() {
    let prefix = gcodekit6_ui_egui::titlebar_prefix(&env_with("GCODEKIT5_TITLEBAR"));
    assert_eq!(prefix.var_name(), "GCODEKIT5_TITLEBAR");
}
