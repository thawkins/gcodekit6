//! Keyboard jog in Machine Control: GTK's keypad keys (8/2 Y, 4/6 X, 9/3 Z) jog one step,
//! or in Continuous mode for as long as the key is held, only once the user pressed in the
//! pane and never while a widget has the keyboard. A held key's jog ends with one jog
//! cancel (0x85) on release, and whenever focus moves: into a text field, to another page,
//! to another window, or with a pointer press (Review Focus 1).

use std::sync::Arc;
use std::time::Duration;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_app::machine::RealtimeCommand;
use gcodekit5_app::test_support::{LoopbackShared, Wrote};
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::workspace::PaneId;

use crate::machine_support::wait_until;
use crate::test_machine_pane::{entry, pane_app, report_state};

/// The `$J` lines written so far.
pub(crate) fn jogs(shared: &LoopbackShared) -> Vec<String> {
    shared
        .log()
        .into_iter()
        .filter_map(|w| match w {
            Wrote::Line(line) if line.starts_with("$J=") => Some(line),
            Wrote::Line(_) | Wrote::Realtime(_) => None,
        })
        .collect()
}

/// How many jog cancels were written.
pub(crate) fn cancels(shared: &LoopbackShared) -> usize {
    shared
        .log()
        .iter()
        .filter(|w| **w == Wrote::Realtime(RealtimeCommand::JogCancel.byte()))
        .count()
}

/// Waits until every command the app handed the controller so far was written: a sentinel
/// query (`$I`), queued behind them, has been written. A queued jog line is written before
/// it, and a real-time byte (jog cancel) as soon as the worker takes it, so after this the
/// log holds everything the app sent; a negative assertion then needs no sleep.
pub(crate) fn settle(harness: &Harness<'_, GcodeKitApp>, shared: &LoopbackShared) {
    let sentinels = |shared: &LoopbackShared| {
        shared
            .log()
            .iter()
            .filter(|w| **w == Wrote::Line("$I".to_string()))
            .count()
    };
    let before = sentinels(shared);
    gcodekit5_ui_egui::machine::lock(harness.state().state().machine.controller())
        .send_query("$I")
        .expect("sentinel query");
    assert!(
        wait_until(|| sentinels(shared) > before),
        "sentinel never written: {:?}",
        shared.log()
    );
}

/// The jog lines once everything sent so far was written ([`settle`]).
pub(crate) fn settled_jogs(
    harness: &Harness<'_, GcodeKitApp>,
    shared: &LoopbackShared,
) -> Vec<String> {
    settle(harness, shared);
    jogs(shared)
}

/// The jog cancel count once everything sent so far was written ([`settle`]).
pub(crate) fn settled_cancels(
    harness: &Harness<'_, GcodeKitApp>,
    shared: &LoopbackShared,
) -> usize {
    settle(harness, shared);
    cancels(shared)
}

/// A key event: `key` pressed or released.
pub(crate) fn key(harness: &Harness<'_, GcodeKitApp>, key: egui::Key, pressed: bool) {
    harness.event(egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
}

/// Presses and releases `k`, as a tap.
pub(crate) fn tap(harness: &mut Harness<'_, GcodeKitApp>, k: egui::Key) {
    key(harness, k, true);
    harness.step();
    key(harness, k, false);
    harness.step();
    harness.step();
}

/// Connected, Idle at machine position (-100, -100, -50), with the pane holding the
/// keyboard (the user pressed on its readout heading); Continuous on when `continuous`.
fn keyboard_app(continuous: bool) -> (Harness<'static, GcodeKitApp>, Arc<LoopbackShared>) {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    report_state(
        &mut harness,
        &shared,
        "<Idle|MPos:-100.000,-100.000,-50.000|FS:0,0>",
        "Idle",
    );
    if continuous {
        harness.get_by_label("Continuous").click();
        harness.step();
    }
    harness.get_by_label("Machine State").click();
    harness.step();
    harness.step();
    (harness, shared)
}

/// Starts a continuous keyboard jog with `k` held and waits for its `$J`.
fn hold(harness: &mut Harness<'_, GcodeKitApp>, shared: &LoopbackShared, k: egui::Key) {
    key(harness, k, true);
    harness.step();
    harness.step();
    assert!(
        wait_until(|| jogs(shared).len() == 1),
        "no jog: {:?}",
        shared.log()
    );
}

#[test]
fn test_keypad_keys_jog_one_step_each() {
    let (mut harness, shared) = keyboard_app(false);
    for (k, line) in [
        (egui::Key::Num6, "$J=G91 G21 X1.00 F1000"),
        (egui::Key::Num4, "$J=G91 G21 X-1.00 F1000"),
        (egui::Key::Num8, "$J=G91 G21 Y1.00 F1000"),
        (egui::Key::Num2, "$J=G91 G21 Y-1.00 F1000"),
        (egui::Key::Num9, "$J=G91 G21 Z1.00 F1000"),
        (egui::Key::Num3, "$J=G91 G21 Z-1.00 F1000"),
    ] {
        let before = jogs(&shared).len();
        tap(&mut harness, k);
        assert!(
            wait_until(|| jogs(&shared).len() == before + 1),
            "{k:?}: {:?}",
            shared.log()
        );
        assert_eq!(
            jogs(&shared).last().map(String::as_str),
            Some(line),
            "{k:?}"
        );
    }
    // A step jog never sends a cancel.
    assert_eq!(settled_cancels(&harness, &shared), 0);
}

#[test]
fn test_keys_never_jog_while_a_text_field_has_focus() {
    let (mut harness, shared) = keyboard_app(false);
    crate::test_machine_pane::focus_entry(&mut harness);
    tap(&mut harness, egui::Key::Num8);
    harness.event(egui::Event::Text("8".to_string()));
    harness.step();
    assert!(
        settled_jogs(&harness, &shared).is_empty(),
        "{:?}",
        shared.log()
    );
    assert!(entry(&harness).is_focused());
}

#[test]
fn test_keys_never_jog_before_the_pane_has_the_keyboard() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    report_state(
        &mut harness,
        &shared,
        "<Idle|MPos:0.000,0.000,0.000|FS:0,0>",
        "Idle",
    );
    tap(&mut harness, egui::Key::Num6);
    assert!(
        settled_jogs(&harness, &shared).is_empty(),
        "{:?}",
        shared.log()
    );
}

/// A held key jogs until released, then sends exactly one jog cancel.
#[test]
fn test_continuous_key_hold_cancels_once_on_release() {
    let (mut harness, shared) = keyboard_app(true);
    hold(&mut harness, &shared, egui::Key::Num4);
    assert_eq!(jogs(&shared), ["$J=G91 G21 X-100.00 F1000"]);
    for _ in 0..10 {
        harness.step();
    }
    assert_eq!(
        settled_cancels(&harness, &shared),
        0,
        "hold ended while the key was down"
    );
    key(&harness, egui::Key::Num4, false);
    harness.step();
    assert!(wait_until(|| cancels(&shared) == 1), "{:?}", shared.log());
    for _ in 0..4 {
        harness.step();
    }
    assert_eq!(settled_cancels(&harness, &shared), 1);
    assert_eq!(jogs(&shared).len(), 1);
}

/// Review Focus 1: focus moving into a text field ends the hold, though the key is held.
#[test]
fn test_continuous_key_hold_cancelled_when_a_text_field_takes_focus() {
    let (mut harness, shared) = keyboard_app(true);
    hold(&mut harness, &shared, egui::Key::Num6);
    // Focus moved by the keyboard or assistive technology: no pointer press.
    entry(&harness).focus();
    harness.step();
    harness.step();
    assert!(entry(&harness).is_focused(), "entry not focused");
    assert!(wait_until(|| cancels(&shared) == 1), "{:?}", shared.log());
    key(&harness, egui::Key::Num6, false);
    harness.step();
    assert_eq!(settled_cancels(&harness, &shared), 1);
}

/// Review Focus 1: another page shown ends the hold.
#[test]
fn test_continuous_key_hold_cancelled_on_page_switch() {
    let (mut harness, shared) = keyboard_app(true);
    hold(&mut harness, &shared, egui::Key::Num8);
    harness
        .state_mut()
        .workspace_mut()
        .show_pane(PaneId::Visualizer);
    harness.step();
    assert!(wait_until(|| cancels(&shared) == 1), "{:?}", shared.log());
}

/// Review Focus 1: the window losing focus (another window) ends the hold.
#[test]
fn test_continuous_key_hold_cancelled_on_window_blur() {
    let (mut harness, shared) = keyboard_app(true);
    hold(&mut harness, &shared, egui::Key::Num9);
    harness.input_mut().focused = false;
    harness.event(egui::Event::WindowFocused(false));
    harness.step();
    assert!(wait_until(|| cancels(&shared) == 1), "{:?}", shared.log());
    harness.input_mut().focused = true;
}

/// A pointer press while a key is held (focus moving elsewhere) ends the hold.
#[test]
fn test_continuous_key_hold_cancelled_by_pointer_press() {
    let (mut harness, shared) = keyboard_app(true);
    hold(&mut harness, &shared, egui::Key::Num3);
    harness.get_by_label("G-Code Editor").click();
    harness.step();
    assert!(wait_until(|| cancels(&shared) == 1), "{:?}", shared.log());
}

/// The key's own auto-repeat does not start a second jog while the hold lasts.
#[test]
fn test_continuous_key_repeat_sends_no_second_jog() {
    let (mut harness, shared) = keyboard_app(true);
    hold(&mut harness, &shared, egui::Key::Num2);
    for _ in 0..3 {
        harness.event(egui::Event::Key {
            key: egui::Key::Num2,
            physical_key: Some(egui::Key::Num2),
            pressed: true,
            repeat: true,
            modifiers: egui::Modifiers::NONE,
        });
        harness.step();
    }
    assert_eq!(
        settled_jogs(&harness, &shared).len(),
        1,
        "{:?}",
        shared.log()
    );
    assert_eq!(settled_cancels(&harness, &shared), 0);
}

/// Review Focus 1, a lost key-up: with NumLock on, Shift turns a held keypad 8 into
/// ArrowUp, so its release arrives as ArrowUp and egui keeps 8 down. The modifier change
/// ends the hold.
#[test]
fn test_continuous_key_hold_cancelled_when_shift_changes_the_key() {
    let (mut harness, shared) = keyboard_app(true);
    hold(&mut harness, &shared, egui::Key::Num8);
    // Shift pressed while 8 is held: the hold ends at once.
    harness.event(egui::Event::ModifiersChanged(egui::Modifiers::SHIFT));
    harness.step();
    assert!(wait_until(|| cancels(&shared) == 1), "{:?}", shared.log());
    // The release then arrives as ArrowUp; egui keeps 8 down, but nothing more is sent.
    harness.event(egui::Event::Key {
        key: egui::Key::ArrowUp,
        physical_key: Some(egui::Key::Num8),
        pressed: false,
        repeat: false,
        modifiers: egui::Modifiers::SHIFT,
    });
    harness.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    for _ in 0..3 {
        harness.step();
    }
    assert_eq!(settled_cancels(&harness, &shared), 1);
    assert_eq!(jogs(&shared).len(), 1);
}

/// A release reported as another key, with no modifier change (a NumLock change between
/// press and release): any other key event ends the hold, though egui keeps 8 down.
#[test]
fn test_continuous_key_hold_cancelled_by_another_key_event() {
    let (mut harness, shared) = keyboard_app(true);
    hold(&mut harness, &shared, egui::Key::Num8);
    harness.event(egui::Event::Key {
        key: egui::Key::ArrowUp,
        physical_key: Some(egui::Key::Num8),
        pressed: false,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    assert!(wait_until(|| cancels(&shared) == 1), "{:?}", shared.log());
}

/// The dead man's switch: a hold no frame confirmed for longer than 250 ms (the app stalled)
/// is cancelled on the next frame, though the key is still down.
#[test]
fn test_continuous_key_hold_cancelled_when_frames_stop() {
    let (mut harness, shared) = keyboard_app(true);
    hold(&mut harness, &shared, egui::Key::Num6);
    std::thread::sleep(Duration::from_millis(300));
    harness.step();
    assert!(wait_until(|| cancels(&shared) == 1), "{:?}", shared.log());
    assert!(
        harness.state().state().machine.status().connected,
        "still connected"
    );
}
