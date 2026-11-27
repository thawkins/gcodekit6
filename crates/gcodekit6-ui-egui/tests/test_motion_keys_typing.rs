//! A command that moves the machine (jog, Home, Stream Start, Cycle Start) never runs from a
//! key while a text field has the keyboard, whatever the key: an F-key or a Ctrl chord
//! (which craft-keymap lets through while typing) is held back by the app's key dispatch, for
//! the default keys and for keys the user gave it. File > Run only previews and still runs.

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use gcodekit6_ui_egui::GcodeKitApp;
use gcodekit6_ui_egui::commands::Command;
use gcodekit6_ui_egui::shortcuts;

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

/// Gives Machine Control the keyboard, as a press in the pane does (Cycle Start's keys work
/// only there).
fn arm_machine_control(h: &mut Harness<'_, GcodeKitApp>) {
    use egui_kittest::kittest::Queryable;
    h.get_by_label("Machine State").click();
    h.step();
    h.step();
}

#[test]
fn test_motion_keys_run_when_no_text_field_has_the_keyboard() {
    for (command, text, modifiers, key) in CASES {
        let mut h = app_with(command, text);
        arm_machine_control(&mut h);
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

/// The one check every route uses (keys, the macOS menu's key equivalents, the in-window
/// bar): only a motion command is held back, and only while a text field has the keyboard.
/// The macOS menu cannot run here, so this is its test: `app.rs` filters its choices with it.
#[test]
fn test_may_run_holds_back_only_motion_while_typing() {
    for command in Command::ALL {
        assert!(command.may_run(false), "{command:?}");
        assert_eq!(
            command.may_run(true),
            !command.starts_motion(),
            "{command:?}"
        );
    }
    for command in [
        Command::Home,
        Command::StreamStart,
        Command::CycleStart,
        Command::JogXPos,
    ] {
        assert!(!command.may_run(true), "{command:?}");
    }
    for command in [Command::FileRun, Command::SoftReset, Command::FeedHold] {
        assert!(command.may_run(true), "{command:?}");
    }
}

/// Opens the in-window Machine menu from the keyboard (Alt and its mnemonic) and presses
/// Home's mnemonic.
fn home_from_menu_keys(h: &mut Harness<'_, GcodeKitApp>) {
    use egui_kittest::kittest::{NodeT, Queryable};
    let machine = h
        .get_by_role_and_label(egui::accesskit::Role::MenuItem, "Machine")
        .accesskit_node()
        .data()
        .access_key()
        .map(str::to_string);
    let machine = machine.expect("Machine mnemonic");
    press(
        h,
        Modifiers::ALT,
        Key::from_name(&machine.to_uppercase()).expect("key"),
    );
    h.run();
    let home = h
        .get_by_role_and_label(egui::accesskit::Role::MenuItem, "Home")
        .accesskit_node()
        .data()
        .access_key()
        .map(str::to_string)
        .expect("Home mnemonic");
    press(
        h,
        Modifiers::NONE,
        Key::from_name(&home.to_uppercase()).expect("key"),
    );
    h.run();
}

#[test]
fn test_in_window_menu_keys_run_home_without_a_text_field() {
    let mut h = connected_app();
    home_from_menu_keys(&mut h);
    assert_eq!(h.state().last_dispatched(), Some(Command::Home));
}

#[test]
fn test_in_window_menu_keys_never_run_home_while_typing() {
    let mut h = connected_app();
    crate::test_machine_pane::focus_entry(&mut h);
    home_from_menu_keys(&mut h);
    assert_ne!(h.state().last_dispatched(), Some(Command::Home));
}

/// A mouse click on Machine > Home still runs it after typing: the click takes the keyboard
/// from the text field.
#[test]
fn test_menu_click_runs_home_after_typing() {
    use egui_kittest::kittest::Queryable;
    let mut h = connected_app();
    crate::test_machine_pane::focus_entry(&mut h);
    h.get_by_role_and_label(egui::accesskit::Role::MenuItem, "Machine")
        .click();
    h.run();
    h.get_by_role_and_label(egui::accesskit::Role::MenuItem, "Home")
        .click();
    h.run();
    assert_eq!(h.state().last_dispatched(), Some(Command::Home));
}

/// GTK's `~` (Shift+`) resumes only while Machine Control has the keyboard, never from a
/// stray press elsewhere.
#[test]
fn test_cycle_start_tilde_runs_only_in_machine_control() {
    let mut h = connected_app();
    press(&mut h, Modifiers::SHIFT, Key::Backtick);
    assert_ne!(h.state().last_dispatched(), Some(Command::CycleStart));
    arm_machine_control(&mut h);
    press(&mut h, Modifiers::SHIFT, Key::Backtick);
    assert_eq!(h.state().last_dispatched(), Some(Command::CycleStart));
}
