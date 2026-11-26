//! The rules of the shortcut editor (Review Focus 5): the E-STOP (soft reset) and feed hold
//! keys cannot be changed or taken, motion commands stay away from them, a jog key never jogs
//! while a text field has the keyboard, no key opens a menu of the in-window bar, and two
//! commands never share a key where both could run. Saved shortcuts that break a rule are put
//! back to the defaults when they are loaded.

use craft_keymap::{Active, Binding, Chord, Keymap, Lookup, Overrides, Trigger};
use gcodekit5_app::i18n::Translator;
use gcodekit5_ui_egui::commands::{self, Command};
use gcodekit5_ui_egui::menu;
use gcodekit5_ui_egui::shortcuts::{self, Refusal};

fn trigger(text: &str) -> Trigger {
    Trigger::parse(text).expect("trigger")
}

fn set(keymap: &mut Keymap, command: Command, text: &str) -> Result<(), Refusal> {
    shortcuts::set_key(keymap, command, trigger(text))
}

fn lookup(keymap: &Keymap, text: &str, active: &Active) -> Option<Command> {
    match keymap.lookup(&Chord::parse(text).expect("chord"), false, active) {
        Lookup::Run(command) => Command::from_ref(&command),
        Lookup::None => None,
        other => panic!("{text}: {other:?}"),
    }
}

fn texts(keymap: &Keymap, command: Command) -> Vec<String> {
    keymap
        .bindings(&command.command_ref())
        .iter()
        .map(|b| b.trigger.to_string())
        .collect()
}

#[test]
fn test_safety_commands_cannot_be_changed_or_cleared() {
    let mut keymap = commands::keymap();
    for command in shortcuts::LOCKED {
        assert_eq!(set(&mut keymap, command, "F9"), Err(Refusal::Locked));
        assert_eq!(set(&mut keymap, command, "K"), Err(Refusal::Locked));
        assert_eq!(shortcuts::clear(&mut keymap, command), Err(Refusal::Locked));
    }
    assert_eq!(texts(&keymap, Command::SoftReset), ["Cmd+Shift+K"]);
    assert_eq!(texts(&keymap, Command::FeedHold), ["!", "Cmd+Shift+H"]);
    assert!(shortcuts::LOCKED.contains(&Command::SoftReset));
    assert!(shortcuts::LOCKED.contains(&Command::FeedHold));
}

#[test]
fn test_no_command_takes_a_safety_key_in_any_scope() {
    let mut keymap = commands::keymap();
    for (command, key) in [
        (Command::FileOpen, "Cmd+Shift+K"),
        (Command::ZoomFit, "Cmd+Shift+K"),
        (Command::AlignLeft, "Cmd+Shift+H"),
        (Command::About, "!"),
        (Command::Unlock, "Ctrl+Shift+K"),
    ] {
        let refusal = set(&mut keymap, command, key).expect_err(key);
        assert!(
            matches!(
                refusal,
                Refusal::SafetyKey(Command::SoftReset | Command::FeedHold)
            ),
            "{command:?} {key}: {refusal:?}"
        );
    }
    assert_eq!(keymap.overrides(), &Overrides::new(), "nothing changed");
}

#[test]
fn test_motion_commands_stay_off_the_safety_keys() {
    let mut keymap = commands::keymap();
    for (command, free) in [
        (Command::Home, "F9"),
        (Command::FileRun, "F10"),
        (Command::StreamStart, "F11"),
        (Command::CycleStart, "F12"),
    ] {
        for key in ["Cmd+K", "Shift+H", "Cmd+Alt+K"] {
            let result = set(&mut keymap, command, key);
            assert_eq!(result, Err(Refusal::NearSafetyKey), "{command:?} {key}");
        }
        assert_eq!(
            set(&mut keymap, command, free),
            Ok(()),
            "{command:?} {free}"
        );
    }
    // Commands that move nothing may use those keys.
    assert_eq!(set(&mut keymap, Command::About, "Cmd+K"), Ok(()));
}

#[test]
fn test_jog_keys_are_plain_character_keys() {
    let mut keymap = commands::keymap();
    for key in [
        "Alt+J", "Cmd+J", "Ctrl+J", "F9", "Escape", "Tab", "Enter", "Space", "Up", "Left", "Home",
    ] {
        assert_eq!(
            set(&mut keymap, Command::JogXPos, key),
            Err(Refusal::JogKeyNotPlain),
            "{key}"
        );
    }
    assert_eq!(set(&mut keymap, Command::JogXPos, "J"), Ok(()));
    assert_eq!(texts(&keymap, Command::JogXPos), ["J"]);
    // The rebound key jogs in Machine Control, and never while a text field has the keyboard.
    let mc = Active::panel(commands::MACHINE_CONTROL_SCOPE);
    assert_eq!(lookup(&keymap, "J", &mc), Some(Command::JogXPos));
    let typing = Active {
        typing: true,
        ..Active::panel(commands::MACHINE_CONTROL_SCOPE)
    };
    assert_eq!(lookup(&keymap, "J", &typing), None);
    assert_eq!(
        lookup(&keymap, "J", &Active::NONE),
        None,
        "Machine Control only"
    );
}

#[test]
fn test_alt_keys_never_open_a_menu_in_any_language() {
    let mut keymap = commands::keymap();
    let letters: Vec<char> = menu::MENU_MNEMONICS
        .iter()
        .flat_map(|(_, letters)| letters.iter().copied())
        .collect();
    for letter in &letters {
        let key = format!("Alt+{}", letter.to_ascii_uppercase());
        for command in [Command::AlignLeft, Command::FileOpen] {
            assert_eq!(
                set(&mut keymap, command, &key),
                Err(Refusal::MenuMnemonic),
                "{command:?} {key}"
            );
        }
    }
    assert_eq!(set(&mut keymap, Command::AlignLeft, "Alt+K"), Ok(()));
    assert_eq!(set(&mut keymap, Command::FileOpen, "Alt+Shift+F"), Ok(()));
}

#[test]
fn test_a_key_in_use_where_both_could_run_is_refused() {
    let mut keymap = commands::keymap();
    // Two app commands.
    assert_eq!(
        set(&mut keymap, Command::FileOpen, "Cmd+S"),
        Err(Refusal::Taken(Command::FileSave))
    );
    // An app command over a panel's key would shadow it there.
    assert_eq!(
        set(&mut keymap, Command::FileOpen, "Cmd+G"),
        Err(Refusal::Taken(Command::Group))
    );
    assert_eq!(
        set(&mut keymap, Command::Group, "Cmd+O"),
        Err(Refusal::Taken(Command::FileOpen))
    );
    // Panels that never have the keyboard together may share a key.
    assert_eq!(set(&mut keymap, Command::ZoomFit, "Cmd+G"), Ok(()));
    assert_eq!(set(&mut keymap, Command::AlignTop, "F"), Ok(()));
    assert_eq!(keymap.conflicts(), Vec::new());
    // Off macOS Ctrl is Cmd: Ctrl+S is Save's key there.
    assert_eq!(
        set(&mut keymap, Command::FileOpen, "Ctrl+S"),
        Err(Refusal::Taken(Command::FileSave))
    );
}

#[test]
fn test_set_key_replaces_and_reset_restores() {
    let mut keymap = commands::keymap();
    assert_eq!(set(&mut keymap, Command::Redo, "Cmd+Shift+Y"), Ok(()));
    assert_eq!(texts(&keymap, Command::Redo), ["Cmd+Shift+Y"]);
    assert_eq!(
        lookup(&keymap, "Cmd+Shift+Y", &Active::NONE),
        Some(Command::Redo)
    );
    assert_eq!(lookup(&keymap, "Cmd+Y", &Active::NONE), None);
    // A key may be set again on the command that has it.
    assert_eq!(set(&mut keymap, Command::Redo, "Cmd+Shift+Y"), Ok(()));
    assert_eq!(shortcuts::clear(&mut keymap, Command::FileOpen), Ok(()));
    assert!(texts(&keymap, Command::FileOpen).is_empty());

    shortcuts::reset(&mut keymap, Command::Redo);
    assert_eq!(texts(&keymap, Command::Redo), ["Cmd+Y", "Cmd+Shift+Z"]);
    assert!(
        texts(&keymap, Command::FileOpen).is_empty(),
        "one command only"
    );
    shortcuts::reset_all(&mut keymap);
    assert_eq!(texts(&keymap, Command::FileOpen), ["Cmd+O"]);
    assert_eq!(keymap.overrides(), &Overrides::new());
}

#[test]
fn test_saved_shortcuts_that_break_a_rule_load_as_defaults() {
    let mut saved = Overrides::new();
    let entries: [(&str, &[&str]); 8] = [
        // Kept: a valid change.
        ("file.open", &["Cmd+Shift+O"]),
        // The E-STOP key moved or removed.
        ("machine.soft_reset", &["F2"]),
        ("machine.feed_hold", &[]),
        // Takes the E-STOP key.
        ("help.about", &["Cmd+Shift+K"]),
        // A jog key that fires while typing, and one moved out of Machine Control.
        ("jog.x_pos", &["Cmd+J"]),
        ("jog.x_neg", &["Application: J"]),
        // Shares Save's key.
        ("edit.undo", &["Cmd+S"]),
        // A menu mnemonic.
        ("designer.align_left", &["Alt+E"]),
    ];
    for (key, texts) in entries {
        saved.insert(key, texts.iter().map(|t| (*t).to_string()).collect());
    }
    let (keymap, dropped) = shortcuts::sanitized(saved);
    assert_eq!(texts(&keymap, Command::FileOpen), ["Cmd+Shift+O"]);
    for command in [
        Command::SoftReset,
        Command::FeedHold,
        Command::About,
        Command::JogXPos,
        Command::JogXNeg,
        Command::Undo,
        Command::AlignLeft,
    ] {
        assert_eq!(
            keymap.bindings(&command.command_ref()),
            keymap.default_bindings(&command.command_ref()),
            "{command:?} is back to its defaults"
        );
        assert!(dropped.contains(&command), "{command:?} not reported");
    }
    assert!(!dropped.contains(&Command::FileOpen));
    assert_eq!(keymap.conflicts(), Vec::new());
    // Soft reset still works anywhere, also while typing.
    let typing = Active {
        typing: true,
        ..Active::NONE
    };
    assert_eq!(
        lookup(&keymap, "Cmd+Shift+K", &typing),
        Some(Command::SoftReset)
    );
}

/// A saved change that shares a key with another saved change cannot be told apart from it:
/// both go back to their defaults.
#[test]
fn test_saved_shortcuts_sharing_a_key_both_load_as_defaults() {
    let mut saved = Overrides::new();
    saved.insert("help.about", vec!["F9".to_string()]);
    saved.insert("tools.device_info", vec!["F9".to_string()]);
    let (keymap, dropped) = shortcuts::sanitized(saved);
    assert!(dropped.contains(&Command::About));
    assert!(dropped.contains(&Command::OpenDeviceInfo));
    assert_eq!(lookup(&keymap, "F9", &Active::NONE), None);
    assert_eq!(keymap.conflicts(), Vec::new());
}

#[test]
fn test_every_default_binding_passes_the_rules() {
    let keymap = commands::keymap();
    for command in Command::ALL {
        if shortcuts::LOCKED.contains(&command) {
            continue;
        }
        for binding in keymap.default_bindings(&command.command_ref()) {
            assert_eq!(
                shortcuts::check(&keymap, command, &binding),
                Ok(()),
                "{command:?} {binding}"
            );
        }
    }
    let (_, dropped) = shortcuts::sanitized(Overrides::new());
    assert!(dropped.is_empty(), "{dropped:?}");
}

#[test]
fn test_refusals_name_the_key_and_the_command_in_the_ui_language() {
    let tr = Translator::new("en");
    assert_eq!(
        Refusal::Taken(Command::FileSave).message(&tr, "Ctrl+S"),
        "Ctrl+S is already used by Save"
    );
    assert_eq!(
        Refusal::SafetyKey(Command::SoftReset).message(&tr, "Ctrl+Shift+K"),
        "Ctrl+Shift+K is kept for Reset"
    );
    let de = Translator::new("de");
    let german = Refusal::Taken(Command::FileSave).message(&de, "Strg+S");
    assert!(german.contains("Strg+S"), "{german}");
    assert_ne!(german, "Strg+S is already used by Save");
    // A binding written with a scope other than the command's own is refused too.
    let keymap = commands::keymap();
    let binding = Binding::parse("Application: J").expect("binding");
    assert_eq!(
        shortcuts::check(&keymap, Command::JogXPos, &binding),
        Err(Refusal::WrongScope)
    );
}
