//! The rules for changing keyboard shortcuts, applied by the Preferences shortcut editor and to
//! the shortcuts saved in the settings.
//!
//! For machine safety:
//! - the E-STOP (soft reset) and feed hold keys ([`LOCKED`]) cannot be changed or removed, and
//!   no other command may take one of their keys, in any scope (a panel's key would win over
//!   them while that panel has the keyboard);
//! - a command that moves the machine ([`Command::starts_motion`]) never runs while a text
//!   field has the keyboard, whatever its key (the app's key dispatch holds it back); it gets
//!   no key on the letter of the feed hold or soft reset chord, so a slipped modifier never
//!   starts what was meant to stop; and outside Machine Control it needs Ctrl, Alt, Cmd or a
//!   function key, since a bare key there would run on every page (GTK's `~` for cycle
//!   start stays as a default);
//! - a jog key is a plain letter, digit or symbol key: craft-keymap's typing rule holds those
//!   back while a text field has the keyboard, so a rebound jog key never jogs while typing,
//!   and Escape, Tab, Enter, Space and the arrows stay with the widgets.
//!
//! And so that every key does one thing:
//! - no two commands share a key where both could run (the whole app, or the same panel; the
//!   Machine Control, Visualizer and Designer panels never have the keyboard together);
//! - no Alt key opens a menu of the in-window bar in any shipped language
//!   ([`crate::menu::MENU_MNEMONICS`]);
//! - no key the operating system takes, no double press, and no scope other than the
//!   command's own.
//!
//! Keys are compared as they work on Linux, Windows and macOS (Ctrl is Cmd off macOS), since
//! one settings file can be used on each.
//!
//! The changes are saved in the settings' `ui.keyboard_shortcuts` ([`saved`]) and loaded at
//! start through [`sanitized`] ([`keymap_from`]), so a hand-edited file cannot take the E-STOP
//! key either.
//!
//! Dependencies: `craft-keymap` (the keymap and chords), `gcodekit5-settings` (the saved
//! form), `gcodekit5-app` (translations).

use craft_keymap::{Binding, Chord, Key, Keymap, Mods, Overrides, Platform, Policy, Trigger};
use gcodekit5_app::i18n::Translator;
use gcodekit5_settings::config::{ShortcutOverride, UiSettings};

use crate::commands::{self, Command};
use crate::menu::MENU_MNEMONICS;

/// The safety commands whose keys never change: soft reset (the E-STOP) and feed hold.
pub const LOCKED: [Command; 2] = [Command::SoftReset, Command::FeedHold];

/// The platforms a settings file can be used on.
const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

/// Why a key cannot be given to a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The command is soft reset or feed hold.
    Locked,
    /// The key is the named safety command's.
    SafetyKey(Command),
    /// A motion command on the letter of a safety chord.
    NearSafetyKey,
    /// A jog key that is not a plain letter, digit or symbol key.
    JogKeyNotPlain,
    /// A command that moves the machine, outside Machine Control, on a bare key.
    MotionKeyNotModified,
    /// Alt and a menu title's mnemonic.
    MenuMnemonic,
    /// The named command has the key where both could run.
    Taken(Command),
    /// The operating system takes the key.
    Reserved,
    /// A double press.
    DoublePress,
    /// A scope other than the command's own.
    WrongScope,
}

impl Refusal {
    /// The reason, in the UI language.
    ///
    /// # Arguments
    /// * `tr` - Translates the message.
    /// * `key` - The key as the user sees it (`Ctrl+S`).
    pub fn message(&self, tr: &Translator, key: &str) -> String {
        let (msgid, command) = match self {
            Self::Locked => (n_!("The E-STOP and feed hold keys cannot be changed"), None),
            Self::SafetyKey(command) => (n_!("{key} is kept for {command}"), Some(*command)),
            Self::NearSafetyKey => (
                n_!(
                    "A command that moves the machine cannot use the letter of the E-STOP or feed hold key"
                ),
                None,
            ),
            Self::JogKeyNotPlain => (
                n_!(
                    "A jog key must be a letter, digit or symbol key without Ctrl or Alt, so it never jogs while you type"
                ),
                None,
            ),
            Self::MotionKeyNotModified => (
                n_!(
                    "Outside Machine Control a command that moves the machine needs Ctrl, Alt or a function key"
                ),
                None,
            ),
            Self::MenuMnemonic => (n_!("{key} opens a menu"), None),
            Self::Taken(command) => (n_!("{key} is already used by {command}"), Some(*command)),
            Self::Reserved => (n_!("{key} is reserved by the system"), None),
            Self::DoublePress => (n_!("Double presses are not supported"), None),
            Self::WrongScope => (n_!("{key} works in the wrong place"), None),
        };
        let text = tr.tr(msgid).replace("{key}", key);
        match command {
            Some(command) => text.replace("{command}", &tr.tr(command.label_msgid())),
            None => text,
        }
    }
}

/// Whether two chords are the same key on any platform.
fn same_chord(a: Chord, b: Chord) -> bool {
    PLATFORMS.iter().any(|p| a.effective(*p) == b.effective(*p))
}

/// Whether bindings in scopes `a` and `b` can both run: one is app-wide, or they are the same
/// panel (panels never have the keyboard together).
fn overlap(a: &craft_keymap::Scope, b: &craft_keymap::Scope) -> bool {
    a.is_app() || b.is_app() || a == b
}

/// The command's home scope.
fn home(keymap: &Keymap, command: Command) -> craft_keymap::Scope {
    keymap
        .bindable(&command.command_ref())
        .map_or_else(craft_keymap::Scope::app, |b| b.scope.clone())
}

/// The safety commands' default chords.
fn safety_chords(keymap: &Keymap) -> impl Iterator<Item = (Command, Chord)> + '_ {
    LOCKED.into_iter().flat_map(move |command| {
        keymap
            .default_bindings(&command.command_ref())
            .into_iter()
            .map(move |b| (command, b.trigger.chord))
    })
}

/// Whether `chord` may jog: a plain letter, digit or symbol key, held back while typing on
/// every platform.
fn jog_key_ok(keymap: &Keymap, chord: Chord) -> bool {
    let key: Key = chord.key;
    chord.is_plain()
        && (key.is_letter() || key.is_digit() || key.is_symbol())
        && PLATFORMS
            .iter()
            .all(|p| !keymap.typing_rule().fires(&chord, *p))
}

/// Whether `chord` is Alt and the mnemonic letter of a menu title in a shipped language.
fn is_menu_mnemonic(chord: Chord) -> bool {
    chord.mods == Mods::ALT
        && chord.key.is_letter()
        && MENU_MNEMONICS
            .iter()
            .flat_map(|(_, letters)| letters)
            .any(|c| {
                chord
                    .key
                    .name()
                    .eq_ignore_ascii_case(c.encode_utf8(&mut [0; 4]))
            })
}

/// Checks whether `command` may have `binding`, besides the bindings it has now.
///
/// # Arguments
/// * `keymap` - The keymap with every command's current bindings.
/// * `command` - The command getting the binding.
/// * `binding` - The binding.
///
/// # Errors
/// The first rule the binding breaks.
pub fn check(keymap: &Keymap, command: Command, binding: &Binding) -> Result<(), Refusal> {
    if LOCKED.contains(&command) {
        return Err(Refusal::Locked);
    }
    let home = home(keymap, command);
    if binding.scope_or(&home) != &home {
        return Err(Refusal::WrongScope);
    }
    let trigger = binding.trigger;
    if trigger.double {
        return Err(Refusal::DoublePress);
    }
    let chord = trigger.chord;
    let reserved = keymap.is_reserved(&chord)
        || PLATFORMS.iter().any(|p| {
            craft_keymap::reserved::system(*p)
                .iter()
                .any(|r| r.effective(*p) == chord.effective(*p))
        });
    if reserved {
        return Err(Refusal::Reserved);
    }
    if let Some((safety, _)) = safety_chords(keymap).find(|(_, c)| same_chord(*c, chord)) {
        return Err(Refusal::SafetyKey(safety));
    }
    if command.starts_motion() && safety_chords(keymap).any(|(_, c)| c.key == chord.key) {
        return Err(Refusal::NearSafetyKey);
    }
    if command.is_jog() && !jog_key_ok(keymap, chord) {
        return Err(Refusal::JogKeyNotPlain);
    }
    let own_default = keymap
        .default_bindings(&command.command_ref())
        .iter()
        .any(|b| b.trigger.chord == chord);
    if command.starts_motion()
        && home.as_str() != commands::MACHINE_CONTROL_SCOPE
        && chord.is_plain()
        && !chord.is_function_key()
        && !own_default
    {
        return Err(Refusal::MotionKeyNotModified);
    }
    if is_menu_mnemonic(chord) {
        return Err(Refusal::MenuMnemonic);
    }
    let holder = Command::ALL
        .into_iter()
        .filter(|other| *other != command)
        .find(|other| {
            let other_home = self::home(keymap, *other);
            keymap.bindings(&other.command_ref()).iter().any(|b| {
                overlap(b.scope_or(&other_home), &home)
                    && !b.trigger.double
                    && same_chord(b.trigger.chord, chord)
            })
        });
    match holder {
        Some(holder) => Err(Refusal::Taken(holder)),
        None => Ok(()),
    }
}

/// Gives `command` the one key `trigger` in place of its current keys, when the rules allow.
///
/// # Arguments
/// * `keymap` - The keymap to change.
/// * `command` - The command.
/// * `trigger` - The key, in the command's own scope.
///
/// # Errors
/// The rule the key breaks; the keymap is unchanged then.
pub fn set_key(keymap: &mut Keymap, command: Command, trigger: Trigger) -> Result<(), Refusal> {
    let binding = Binding::new(trigger);
    check(keymap, command, &binding)?;
    keymap
        .set_bindings(&command.command_ref(), vec![binding], Policy::Refuse)
        .map(|_| ())
        .map_err(|error| {
            // `check` already refused every holder and reserved key; this is the overrides cap.
            tracing::warn!(%error, command = command.id(), "shortcut not set");
            Refusal::Reserved
        })
}

/// Leaves `command` without keys.
///
/// # Errors
/// [`Refusal::Locked`] for soft reset and feed hold.
pub fn clear(keymap: &mut Keymap, command: Command) -> Result<(), Refusal> {
    if LOCKED.contains(&command) {
        return Err(Refusal::Locked);
    }
    keymap.clear(&command.command_ref());
    Ok(())
}

/// Gives `command` its default keys back, when the rules allow them now: another command
/// may have been given one of them meanwhile.
///
/// # Errors
/// The first default key the rules refuse, and why; the keymap is unchanged then.
pub fn reset(keymap: &mut Keymap, command: Command) -> Result<(), (Trigger, Refusal)> {
    let command_ref = command.command_ref();
    if !LOCKED.contains(&command) {
        for binding in keymap.default_bindings(&command_ref) {
            check(keymap, command, &binding).map_err(|refusal| (binding.trigger, refusal))?;
        }
    }
    keymap.reset(&command_ref);
    Ok(())
}

/// Gives every command its default keys back.
pub fn reset_all(keymap: &mut Keymap) {
    keymap.reset_all();
}

/// Whether the user changed `command`'s keys.
fn changed(keymap: &Keymap, command: Command) -> bool {
    keymap.overrides().get(command.id()).is_some()
}

/// The app's keymap with the saved `overrides`, after putting back to their defaults the
/// commands whose saved keys break a rule (a hand-edited or damaged settings file).
///
/// # Arguments
/// * `overrides` - The saved changes to the default keys.
///
/// # Returns
/// The keymap, and the commands put back to their defaults.
pub fn sanitized(overrides: Overrides) -> (Keymap, Vec<Command>) {
    let mut keymap = commands::keymap().with_overrides(overrides);
    for problem in keymap.problems() {
        tracing::warn!(?problem, "saved shortcut not used");
    }
    let mut dropped: Vec<Command> = LOCKED
        .into_iter()
        .filter(|c| changed(&keymap, *c))
        .collect();
    for command in &dropped {
        keymap.reset(&command.command_ref());
    }
    // Each pass resets at least one changed command, so this ends. A command put back may
    // collide with another change, which the next pass catches.
    loop {
        let bad: Vec<Command> = Command::ALL
            .into_iter()
            .filter(|c| changed(&keymap, *c))
            .filter(|c| {
                keymap
                    .bindings(&c.command_ref())
                    .iter()
                    .any(|b| check(&keymap, *c, b).is_err())
            })
            .collect();
        if bad.is_empty() {
            break;
        }
        for command in bad {
            tracing::warn!(
                command = command.id(),
                "saved shortcut breaks a rule; default used"
            );
            keymap.reset(&command.command_ref());
            dropped.push(command);
        }
    }
    (keymap, dropped)
}

/// The keymap's changes to the default keys, as the settings save them.
///
/// # Arguments
/// * `keymap` - The keymap as edited.
pub fn saved(keymap: &Keymap) -> Vec<ShortcutOverride> {
    keymap
        .overrides()
        .iter()
        .map(|(command, keys)| ShortcutOverride {
            command: command.to_string(),
            keys: keys.to_vec(),
        })
        .collect()
}

/// The app's keymap with the shortcuts saved in `ui`, each command whose saved keys break a
/// rule put back to its defaults ([`sanitized`]).
///
/// # Arguments
/// * `ui` - The UI settings holding the saved shortcuts.
pub fn keymap_from(ui: &UiSettings) -> Keymap {
    let mut overrides = Overrides::new();
    for entry in &ui.keyboard_shortcuts {
        if !overrides.insert(entry.command.clone(), entry.keys.clone()) {
            tracing::warn!(command = %entry.command, "saved shortcut too long; ignored");
        }
    }
    sanitized(overrides).0
}

/// The English msgids of the refusal messages, for the translation test.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("The E-STOP and feed hold keys cannot be changed"),
        n_!("{key} is kept for {command}"),
        n_!(
            "A command that moves the machine cannot use the letter of the E-STOP or feed hold key"
        ),
        n_!(
            "A jog key must be a letter, digit or symbol key without Ctrl or Alt, so it never jogs while you type"
        ),
        n_!(
            "Outside Machine Control a command that moves the machine needs Ctrl, Alt or a function key"
        ),
        n_!("{key} opens a menu"),
        n_!("{key} is already used by {command}"),
        n_!("{key} is reserved by the system"),
        n_!("Double presses are not supported"),
        n_!("{key} works in the wrong place"),
    ]
}
