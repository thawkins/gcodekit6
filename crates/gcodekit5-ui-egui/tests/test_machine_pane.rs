//! The Machine Control pane against a real `MachineController` on the loopback transport:
//! connecting, jogging (step and hold-to-jog), the readout, overrides, stream progress, the
//! safety chords while a text field has the keyboard, and pane keys never taking a key from
//! a focused widget.

use std::sync::atomic::Ordering;
use std::time::Duration;

use craft_keymap::{Binding, Policy};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_app::machine::{MachineEvent, RealtimeCommand};
use gcodekit5_app::test_support::{LoopbackShared, Wrote};
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::commands::Command;
use gcodekit5_ui_egui::machine::coalesce_progress;

use crate::machine_support::{app_harness, connect, step_until, wait_until};

/// The app on a loopback controller acking after `ok_delay`, connected unless `connected`
/// is false, in a harness that has drawn one frame.
pub(crate) fn pane_app(
    ok_delay: Duration,
    connected: bool,
) -> (
    Harness<'static, GcodeKitApp>,
    std::sync::Arc<LoopbackShared>,
) {
    pane_app_with(
        crate::machine_support::loopback_config(),
        ok_delay,
        connected,
    )
}

/// [`pane_app`] with the settings `config`.
pub(crate) fn pane_app_with(
    config: gcodekit5_settings::config::Config,
    ok_delay: Duration,
    connected: bool,
) -> (
    Harness<'static, GcodeKitApp>,
    std::sync::Arc<LoopbackShared>,
) {
    let (mut state, shared) = crate::machine_support::loopback_state_with(config, ok_delay);
    if connected {
        connect(&mut state);
    }
    let mut harness = app_harness(GcodeKitApp::from_state(state));
    harness.step();
    harness.step();
    (harness, shared)
}

/// Has the loopback send the status report `line` and steps until the app shows a report
/// in `state`.
pub(crate) fn report_state(
    harness: &mut Harness<'_, GcodeKitApp>,
    shared: &LoopbackShared,
    line: &str,
    state: &str,
) {
    shared.inject(&format!("{line}\n"));
    assert!(
        step_until(harness, |app| app
            .state()
            .machine
            .status()
            .report
            .as_ref()
            .and_then(|r| r.machine_state.as_deref())
            == Some(state)),
        "no {state} report"
    );
    harness.step();
}

/// Reports the machine idle at the machine origin.
pub(crate) fn report_idle(harness: &mut Harness<'_, GcodeKitApp>, shared: &LoopbackShared) {
    report_state(
        harness,
        shared,
        "<Idle|MPos:0.000,0.000,0.000|FS:0,0>",
        "Idle",
    );
}

/// Whether the loopback saw the real-time byte of `command`.
pub(crate) fn wrote_realtime(shared: &LoopbackShared, command: RealtimeCommand) -> bool {
    shared.log().contains(&Wrote::Realtime(command.byte()))
}

/// The command entry.
pub(crate) fn entry<'h>(harness: &'h Harness<'_, GcodeKitApp>) -> egui_kittest::Node<'h> {
    harness.get_by_role_and_label(Role::TextInput, "Enter G-code command...")
}

/// Scrolls the command entry into view, then hovers and clicks it (a TextEdit takes focus
/// from a click only once the pointer has hovered it on an earlier frame).
pub(crate) fn focus_entry(harness: &mut Harness<'_, GcodeKitApp>) {
    entry(harness).scroll_to_me();
    harness.step();
    harness.step();
    entry(harness).hover();
    harness.step();
    entry(harness).click();
    harness.step();
    harness.step();
    assert!(entry(harness).is_focused(), "entry not focused");
}

/// Presses (or releases) the primary pointer button at `pos`.
pub(crate) fn pointer_button(harness: &Harness<'_, GcodeKitApp>, pos: egui::Pos2, pressed: bool) {
    harness.event(egui::Event::PointerMoved(pos));
    harness.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    });
}

#[test]
fn test_connect_button_connects() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), false);
    assert!(!shared.connected.load(Ordering::SeqCst));
    harness.get_by_label("Connect").click();
    assert!(
        step_until(&mut harness, |app| app.state().machine.status().connected),
        "pane never saw the connection"
    );
    assert!(shared.connected.load(Ordering::SeqCst));
    harness.step();
    let _ = harness.get_by_label("Disconnect");
}

#[test]
fn test_jog_x_positive_sends_jog() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    report_idle(&mut harness, &shared);
    harness.get_by_label("Jog X+").click();
    harness.step();
    assert!(
        wait_until(|| shared
            .log()
            .contains(&Wrote::Line("$J=G91 G21 X1.00 F1000".to_string()))),
        "jog not written: {:?}",
        shared.log()
    );
}

#[test]
fn test_dro_shows_two_decimals() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    shared.inject("<Idle|MPos:1.23456,2.00000,-3.10000|FS:0,0>\n");
    assert!(
        step_until(&mut harness, |app| app
            .state()
            .machine
            .status()
            .report
            .is_some()),
        "no status report"
    );
    harness.step();
    assert!(
        !harness
            .query_all_by_label("1.23")
            .collect::<Vec<_>>()
            .is_empty()
    );
    assert!(
        !harness
            .query_all_by_label("-3.10")
            .collect::<Vec<_>>()
            .is_empty()
    );
    assert!(harness.query_by_label("1.23456").is_none());
    assert!(harness.query_by_label("1.235").is_none());
}

#[test]
fn test_feed_override_plus_10() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    harness.get_by_label("Increase feed rate by 10%").click();
    harness.step();
    assert!(
        wait_until(|| wrote_realtime(&shared, RealtimeCommand::FeedOverridePlus10)),
        "feed override not written: {:?}",
        shared.log()
    );
}

/// Review Focus 1: with the Preferences window open over the workspace, the stream keeps
/// reporting progress and the pane keeps showing it.
#[test]
fn test_stream_progress_updates_while_dialog_open() {
    let (mut harness, _shared) = pane_app(Duration::from_millis(20), true);
    let program: String = (0..200).map(|i| format!("G1 X{i} F1000\n")).collect();
    harness
        .state_mut()
        .state_mut()
        .machine
        .set_program(&program)
        .expect("program");
    harness.step();
    harness.get_by_label("Send").click();
    harness.step();
    harness.state_mut().dispatch(Command::OpenSettings);
    assert!(
        step_until(&mut harness, |app| {
            app.state()
                .machine
                .status()
                .progress
                .is_some_and(|p| p.acked > 0)
        }),
        "stream never progressed"
    );
    let _ = harness.get_by_label("Measurement System");
    let acked = |h: &Harness<'_, GcodeKitApp>| {
        h.state()
            .state()
            .machine
            .status()
            .progress
            .map_or(0, |p| p.acked)
    };
    let before = acked(&harness);
    for _ in 0..20 {
        harness.step();
        std::thread::sleep(Duration::from_millis(25));
    }
    harness.step();
    let after = acked(&harness);
    assert!(after > before, "progress stuck at {before} (now {after})");
    let _ = harness.get_by_label("Measurement System");
    let shown = format!("{after} / 200");
    assert!(
        harness.query_by_label_contains(&shown).is_some(),
        "pane does not show {shown}"
    );
}

/// Plan Review Focus 1: with the command entry focused, Cmd+Shift+H holds the feed and
/// Cmd+Shift+K soft-resets; a typed `!` goes into the entry and holds nothing.
#[test]
fn test_safety_chords_work_in_text_field() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    focus_entry(&mut harness);

    // A typed `!` is text.
    harness.event(egui::Event::Key {
        key: egui::Key::Exclamationmark,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::SHIFT,
    });
    harness.event(egui::Event::Text("!".to_string()));
    harness.step();
    harness.step();
    assert_eq!(entry(&harness).value().as_deref(), Some("!"));
    assert_ne!(harness.state().last_dispatched(), Some(Command::FeedHold));
    std::thread::sleep(Duration::from_millis(20));
    assert!(!wrote_realtime(&shared, RealtimeCommand::FeedHold));

    let cmd_shift = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
    harness.key_press_modifiers(cmd_shift, egui::Key::H);
    harness.step();
    assert_eq!(harness.state().last_dispatched(), Some(Command::FeedHold));
    assert!(
        wait_until(|| wrote_realtime(&shared, RealtimeCommand::FeedHold)),
        "feed hold not written: {:?}",
        shared.log()
    );
    assert!(entry(&harness).is_focused());

    harness.key_press_modifiers(cmd_shift, egui::Key::K);
    harness.step();
    assert_eq!(harness.state().last_dispatched(), Some(Command::SoftReset));
    assert!(
        wait_until(|| wrote_realtime(&shared, RealtimeCommand::SoftReset)),
        "soft reset not written: {:?}",
        shared.log()
    );
    assert_eq!(
        entry(&harness).value().as_deref(),
        Some("!"),
        "the chords typed nothing"
    );
}

/// Bare Escape runs nothing anywhere, also with the pane holding the keyboard.
#[test]
fn test_escape_is_unbound() {
    let keymap = gcodekit5_ui_egui::commands::keymap();
    let escape = craft_keymap::Chord::parse("Escape").expect("chord");
    for active in [
        craft_keymap::Active::NONE,
        craft_keymap::Active::panel(gcodekit5_ui_egui::commands::MACHINE_CONTROL_SCOPE),
    ] {
        assert_eq!(
            keymap.lookup(&escape, false, &active),
            craft_keymap::Lookup::None
        );
    }
}

/// Pane keys work once the user pressed in the pane, but never while a widget inside it (a
/// dropdown, the command entry) has the keyboard.
#[test]
fn test_pane_keys_never_steal_from_focused_widget() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    report_idle(&mut harness, &shared);
    let binding = Binding::parse("Alt+J").expect("binding");
    harness
        .state_mut()
        .keymap_mut()
        .assign(&Command::JogXPos.command_ref(), binding, Policy::Keep)
        .expect("assign");
    let jogs = |shared: &LoopbackShared| {
        shared
            .log()
            .iter()
            .filter(|w| matches!(w, Wrote::Line(line) if line.starts_with("$J=")))
            .count()
    };
    let press_alt_j = |harness: &mut Harness<'_, GcodeKitApp>| {
        harness.key_press_modifiers(egui::Modifiers::ALT, egui::Key::J);
        harness.step();
        harness.step();
        std::thread::sleep(Duration::from_millis(20));
    };

    // Press on the pane's readout heading (no widget takes focus): the pane has the keys.
    harness.get_by_label("Machine State").click();
    harness.step();
    press_alt_j(&mut harness);
    assert!(wait_until(|| jogs(&shared) == 1), "{:?}", shared.log());

    // The step dropdown open: the key stays with it.
    harness.get_by_label("Step").click();
    harness.step();
    harness.step();
    press_alt_j(&mut harness);
    assert_eq!(jogs(&shared), 1, "jogged with the dropdown open");
    harness.key_press(egui::Key::Escape);
    harness.step();
    harness.step();

    // The command entry focused: the key stays with it.
    focus_entry(&mut harness);
    press_alt_j(&mut harness);
    assert!(entry(&harness).is_focused());
    assert_eq!(jogs(&shared), 1, "jogged with the text field focused");
}

/// One frame keeps only the latest stream progress, in place; other events stay in order.
#[test]
fn test_coalesce_progress_keeps_latest() {
    let progress = |acked| MachineEvent::StreamProgress {
        sent: 10,
        acked,
        total: 10,
    };
    let events = vec![
        progress(1),
        MachineEvent::Received("[MSG:x]".to_string()),
        progress(2),
        progress(3),
        MachineEvent::StreamFinished { lines: 10 },
    ];
    assert_eq!(
        coalesce_progress(events),
        vec![
            MachineEvent::Received("[MSG:x]".to_string()),
            progress(3),
            MachineEvent::StreamFinished { lines: 10 },
        ]
    );
}
