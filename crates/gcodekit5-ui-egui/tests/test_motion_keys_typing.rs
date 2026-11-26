//! A command that moves the machine (jog, Home, Stream Start, Cycle Start) never runs from a
//! key while a text field has the keyboard, whatever the key: an F-key or a Ctrl chord
//! (which craft-keymap lets through while typing) is held back by the app's key dispatch, for
//! the default keys and for keys the user gave it. File > Run only previews and still runs.

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::commands::Command;
use gcodekit5_ui_egui::shortcuts;

/// The app, connected to a loopback machine (so the command entry takes the keyboard).
fn connected_app() -> Harness<'static, GcodeKitApp> {
    let (h, _shared) =
        crate::test_machine_pane::pane_app(std::time::Duration::from_millis(1), true);
    h
}

/// The connected app with `command` given the one key `text`.
fn app_with(command: Command, text: &str) -> Harness<'static, GcodeKitApp> {
    let mut h = connected_app();
    let trigger = craft_keymap::Trigger::parse(text).expect("trigger");
    shortcuts::set_key(h.state_mut().keymap_mut(), command, trigger).expect("allowed key");
    h.run();
    h
}

fn press(h: &mut Harness<'_, GcodeKitApp>, modifiers: Modifiers, key: Key) {
    h.key_press_modifiers(modifiers, key);
    h.step();
    h.step();
}

/// (command, key text, egui modifiers, egui key).
const CASES: [(Command, &str, Modifiers, Key); 6] = [
    (Command::Home, "F9", Modifiers::NONE, Key::F9),
    (
        Command::Home,
        "Cmd+Alt+G",
        Modifiers::COMMAND.plus(Modifiers::ALT),
        Key::G,
    ),
    (Command::StreamStart, "F11", Modifiers::NONE, Key::F11),
    (
        Command::StreamStart,
        "Cmd+Alt+R",
        Modifiers::COMMAND.plus(Modifiers::ALT),
        Key::R,
    ),
    (Command::CycleStart, "F12", Modifiers::NONE, Key::F12),
    (
        Command::CycleStart,
        "Cmd+Alt+C",
        Modifiers::COMMAND.plus(Modifiers::ALT),
        Key::C,
    ),
];

#[test]
fn test_motion_keys_run_when_no_text_field_has_the_keyboard() {
    for (command, text, modifiers, key) in CASES {
        let mut h = app_with(command, text);
        press(&mut h, modifiers, key);
        assert_eq!(h.state().last_dispatched(), Some(command), "{text}");
    }
}

#[test]
fn test_motion_keys_never_run_while_a_text_field_has_the_keyboard() {
    for (command, text, modifiers, key) in CASES {
        let mut h = app_with(command, text);
        crate::test_machine_pane::focus_entry(&mut h);
        press(&mut h, modifiers, key);
        assert_ne!(h.state().last_dispatched(), Some(command), "{text}");
        assert!(crate::test_machine_pane::entry(&h).is_focused(), "{text}");
    }
}

/// Home's default Cmd+Home is held back too (it also moves the text cursor).
#[test]
fn test_default_home_key_never_runs_while_typing() {
    let mut h = connected_app();
    crate::test_machine_pane::focus_entry(&mut h);
    press(&mut h, Modifiers::COMMAND, Key::Home);
    assert_ne!(h.state().last_dispatched(), Some(Command::Home));
}

/// File > Run (Cmd+R, F5) previews in the Visualizer and sends nothing: not held back.
#[test]
fn test_file_run_key_still_runs_while_typing() {
    let mut h = connected_app();
    crate::test_machine_pane::focus_entry(&mut h);
    press(&mut h, Modifiers::NONE, Key::F5);
    assert_eq!(h.state().last_dispatched(), Some(Command::FileRun));
}
