//! The Preferences Shortcuts page: Change records the next key press as a command's key,
//! refused keys say why and change nothing, the E-STOP and feed hold rows cannot be changed,
//! Reset and Reset All give the defaults back, and Apply hands the shortcuts to the app, which
//! uses them at once, in its key dispatch and its menus. While a key is recorded only the
//! E-STOP and feed hold keys run.

use std::path::PathBuf;

use craft_keymap::{Active, Chord, Lookup};
use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use gcodekit5_app::i18n::Translator;
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::migrate::ConfigDirs;
use gcodekit5_ui_egui::commands::{self, Command};
use gcodekit5_ui_egui::windows::settings::{SettingsOutcome, SettingsWindow};
use gcodekit5_ui_egui::{AppState, GcodeKitApp};

/// Fresh settings folders under `target/temp/egui_settings_shortcuts/<test>-<pid>`.
fn scratch(test: &str) -> ConfigDirs {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/egui_settings_shortcuts")
        .join(format!("{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create scratch dir");
    ConfigDirs::new(root.join("legacy"), root.join("config"))
}

struct State {
    window: SettingsWindow,
    settings: SettingsManager,
    tr: Translator,
    outcomes: Vec<SettingsOutcome>,
}

/// The Preferences window over `dirs`, open on the Shortcuts page.
fn harness(dirs: ConfigDirs) -> Harness<'static, State> {
    let (settings, problem) = SettingsManager::open(dirs);
    assert!(problem.is_none(), "{problem:?}");
    let mut window = SettingsWindow::new(false, &settings.config().ui);
    window.open(&settings, &commands::keymap());
    let state = State {
        window,
        settings,
        tr: Translator::new("en"),
        outcomes: Vec::new(),
    };
    let mut h = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, state: &mut State| {
                let outcome = state.window.show(ui.ctx(), &mut state.settings, &state.tr);
                if !matches!(outcome, SettingsOutcome::Open | SettingsOutcome::Closed) {
                    state.outcomes.push(outcome);
                }
            },
            state,
        );
    h.run();
    h.get_by_label("Shortcuts").click();
    h.run();
    h
}

/// Shows only the rows whose label contains `text`.
fn filter<S>(h: &mut Harness<'_, S>, text: &str) {
    let field = h.get_by_label("Filter:");
    field.click();
    field.type_text(text);
    h.run();
}

fn click<S>(h: &mut Harness<'_, S>, label: &str) {
    h.get_by_label(label).click();
    h.run();
}

fn press<S>(h: &mut Harness<'_, S>, modifiers: Modifiers, key: Key) {
    h.key_press_modifiers(modifiers, key);
    h.run();
}

fn ctrl_shift() -> Modifiers {
    Modifiers::COMMAND | Modifiers::SHIFT
}

fn runs(keymap: &craft_keymap::Keymap, text: &str) -> Option<Command> {
    match keymap.lookup(&Chord::parse(text).expect("chord"), false, &Active::NONE) {
        Lookup::Run(command) => Command::from_ref(&command),
        _ => None,
    }
}

#[test]
fn test_change_records_a_key_and_apply_hands_it_to_the_app() {
    let mut h = harness(scratch("test_change_records_a_key"));
    filter(&mut h, "Save As");
    click(&mut h, "Change");
    assert!(h.state().window.is_recording());
    let _ = h.get_by_label("Press a key (Escape cancels)");
    press(&mut h, ctrl_shift(), Key::A);
    assert!(!h.state().window.is_recording());
    let _ = h.get_by_label("Ctrl+Shift+A");
    // The filter field did not get the letter.
    assert!(h.query_by_label("Save AsA").is_none());
    click(&mut h, "Apply");
    assert_eq!(
        h.state().outcomes,
        [SettingsOutcome::Applied { restart: false }]
    );
    let keymap = h
        .state_mut()
        .window
        .take_applied_keymap()
        .expect("applied shortcuts");
    assert_eq!(runs(&keymap, "Cmd+Shift+A"), Some(Command::FileSaveAs));
    assert_eq!(runs(&keymap, "Cmd+Shift+S"), None);
    assert!(h.state_mut().window.take_applied_keymap().is_none(), "once");
}

#[test]
fn test_refused_key_says_why_and_keeps_the_old_key() {
    let mut h = harness(scratch("test_refused_key_says_why"));
    filter(&mut h, "Save As");
    click(&mut h, "Change");
    press(&mut h, Modifiers::COMMAND, Key::S);
    let _ = h.get_by_label("Ctrl+S is already used by Save");
    let _ = h.get_by_label("Ctrl+Shift+S");
    click(&mut h, "Change");
    press(&mut h, ctrl_shift(), Key::K);
    let _ = h.get_by_label("Ctrl+Shift+K is kept for Reset");
    let _ = h.get_by_label("Ctrl+Shift+S");
    click(&mut h, "Apply");
    let keymap = h
        .state_mut()
        .window
        .take_applied_keymap()
        .expect("applied shortcuts");
    assert_eq!(runs(&keymap, "Cmd+Shift+S"), Some(Command::FileSaveAs));
    assert_eq!(runs(&keymap, "Cmd+Shift+K"), Some(Command::SoftReset));
}

#[test]
fn test_escape_stops_recording_without_a_change() {
    let mut h = harness(scratch("test_escape_stops_recording"));
    filter(&mut h, "Save As");
    click(&mut h, "Change");
    press(&mut h, Modifiers::NONE, Key::Escape);
    assert!(!h.state().window.is_recording());
    let _ = h.get_by_label("Ctrl+Shift+S");
    assert!(
        h.state().window.is_open(),
        "Escape did not close the window"
    );
}

#[test]
fn test_safety_rows_cannot_be_changed() {
    let mut h = harness(scratch("test_safety_rows_cannot_be_changed"));
    for label in ["Feed Hold", "Reset"] {
        let field = h.get_by_label("Filter:");
        field.click();
        h.key_press_modifiers(Modifiers::COMMAND, Key::A);
        field.type_text(label);
        h.run();
        let rows: Vec<_> = h.get_all_by_label("Change").collect();
        assert!(!rows.is_empty(), "{label}");
        // The Feed Hold filter shows one row; Reset shows soft reset and Reset View.
        let first = &rows[0];
        assert!(
            first.accesskit_node().is_disabled(),
            "{label} can be changed"
        );
    }
}

#[test]
fn test_jog_rebound_to_a_plain_letter_only() {
    let mut h = harness(scratch("test_jog_rebound_to_a_letter"));
    filter(&mut h, "Jog X+");
    click(&mut h, "Change");
    press(&mut h, Modifiers::COMMAND, Key::J);
    let _ = h.get_by_label(
        "A jog key must be a letter, digit or symbol key without Ctrl or Alt, so it never jogs \
         while you type",
    );
    click(&mut h, "Change");
    press(&mut h, Modifiers::NONE, Key::J);
    let _ = h.get_by_label("J");
    click(&mut h, "Apply");
    let keymap = h
        .state_mut()
        .window
        .take_applied_keymap()
        .expect("applied shortcuts");
    let mc = Active::panel(commands::MACHINE_CONTROL_SCOPE);
    let typing = Active {
        typing: true,
        ..Active::panel(commands::MACHINE_CONTROL_SCOPE)
    };
    let j = Chord::parse("J").expect("chord");
    assert_eq!(
        keymap.lookup(&j, false, &mc),
        Lookup::Run(Command::JogXPos.command_ref())
    );
    assert_eq!(keymap.lookup(&j, false, &typing), Lookup::None);
}

#[test]
fn test_alt_key_of_a_menu_refused() {
    let mut h = harness(scratch("test_alt_key_of_a_menu_refused"));
    filter(&mut h, "Align Left");
    click(&mut h, "Change");
    press(&mut h, Modifiers::ALT, Key::F);
    let _ = h.get_by_label("Alt+F opens a menu");
    let _ = h.get_by_label("Alt+L");
}

#[test]
fn test_reset_and_reset_all_restore_defaults_and_cancel_drops_edits() {
    let mut h = harness(scratch("test_reset_and_reset_all"));
    filter(&mut h, "Save As");
    click(&mut h, "Clear");
    let _ = h.get_by_label("None");
    click(&mut h, "Reset");
    let _ = h.get_by_label("Ctrl+Shift+S");
    click(&mut h, "Clear");
    click(&mut h, "Reset All");
    let _ = h.get_by_label("Ctrl+Shift+S");
    click(&mut h, "Clear");
    click(&mut h, "Cancel");
    assert_eq!(h.state().outcomes, [SettingsOutcome::Cancelled]);
    assert!(h.state_mut().window.take_applied_keymap().is_none());
}

/// Opens Preferences in the app on the Shortcuts page, filtered to `text`.
fn app_on_shortcuts(test: &str, text: &str) -> Harness<'static, GcodeKitApp> {
    let (settings, problem) = SettingsManager::open(scratch(test));
    assert!(problem.is_none(), "{problem:?}");
    let mut app = GcodeKitApp::from_state(AppState::new(
        settings,
        crate::test_smoke::native_decision(),
    ));
    app.dispatch(Command::OpenSettings);
    let mut h = crate::machine_support::app_harness(app);
    h.run();
    click(&mut h, "Shortcuts");
    filter(&mut h, text);
    h
}

#[test]
fn test_applied_shortcut_runs_at_once_and_shows_in_the_menu() {
    let mut h = app_on_shortcuts("test_applied_shortcut_runs_at_once", "Device Info");
    click(&mut h, "Change");
    press(&mut h, Modifiers::NONE, Key::F9);
    click(&mut h, "Apply");
    assert!(!h.state().settings_window().is_open());
    press(&mut h, Modifiers::NONE, Key::F9);
    assert_eq!(h.state().last_dispatched(), Some(Command::OpenDeviceInfo));
    // The Machine menu shows the new key (GTK's menus put Device Info under Machine).
    let item = Command::OpenDeviceInfo.label_msgid();
    click(&mut h, "Machine");
    let node = h.get_by_role_and_label(egui::accesskit::Role::MenuItem, item);
    assert_eq!(node.accesskit_node().data().keyboard_shortcut(), Some("F9"));
}

#[test]
fn test_only_the_safety_keys_run_while_a_key_is_recorded() {
    let mut h = app_on_shortcuts("test_only_the_safety_keys_run", "Save As");
    click(&mut h, "Change");
    // Ctrl+O is recorded (and refused: Open has it), not run.
    press(&mut h, Modifiers::COMMAND, Key::O);
    assert_ne!(h.state().last_dispatched(), Some(Command::FileOpen));
    assert_eq!(h.state().dialogs().waiting(), 0, "no Open dialog");
    let _ = h.get_by_label("Ctrl+O is already used by Open");
    // The E-STOP key still runs, and is not recorded.
    click(&mut h, "Change");
    press(&mut h, ctrl_shift(), Key::K);
    assert_eq!(h.state().last_dispatched(), Some(Command::SoftReset));
    assert!(
        h.state().settings_window().is_recording(),
        "still recording"
    );
}
