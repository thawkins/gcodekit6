//! The command catalog, its keymap and its menus: every GTK accelerator is bound to the same
//! command, the keymap has no conflicts, every command but jogging is in the menus, and a key
//! press runs its command in the app.

use craft_keymap::{Active, Chord, Lookup};
use craft_menu::{LayoutOptions, Target};
use egui_kittest::Harness;
use gcodekit5_app::i18n::Translator;
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::commands::{self, Command};
use gcodekit5_ui_egui::dialogs::{DialogAnswer, FilePurpose};
use gcodekit5_ui_egui::menu;
use gcodekit5_ui_egui::workspace::PaneContext;
use gcodekit5_ui_egui::{AppState, GcodeKitApp};

/// Where a binding works: the whole app, or only while a panel has the keyboard.
#[derive(Clone, Copy, Debug)]
enum Where {
    App,
    Panel(&'static str),
}

/// The accelerators of `crates/gcodekit5-ui/src/ui/gtk/common/accelerators.rs` and the extra
/// ones `gtk_app.rs` registers with `set_accels_for_action`, in craft_keymap's canonical text
/// form, with the GTK string each comes from. `<Ctrl>` is craft's `Cmd` (Ctrl off macOS, the
/// Command key on it).
const GTK_ACCELERATORS: &[(&str, &str, Command, Where)] = &[
    // File
    ("<Ctrl>n", "Cmd+N", Command::FileNew, Where::App),
    ("<Ctrl>o", "Cmd+O", Command::FileOpen, Where::App),
    ("<Ctrl>s", "Cmd+S", Command::FileSave, Where::App),
    (
        "<Ctrl><Shift>s",
        "Cmd+Shift+S",
        Command::FileSaveAs,
        Where::App,
    ),
    ("<Ctrl>q", "Cmd+Q", Command::Quit, Where::App),
    ("<Ctrl>r", "Cmd+R", Command::FileRun, Where::App),
    ("F5", "F5", Command::FileRun, Where::App),
    (
        "<Control><Shift>i",
        "Cmd+Shift+I",
        Command::FileImportImage,
        Where::App,
    ),
    // Edit
    ("<Ctrl>z", "Cmd+Z", Command::Undo, Where::App),
    ("<Ctrl>y", "Cmd+Y", Command::Redo, Where::App),
    (
        "<Control><Shift>z",
        "Cmd+Shift+Z",
        Command::Redo,
        Where::App,
    ),
    ("<Ctrl>x", "Cmd+X", Command::Cut, Where::App),
    ("<Ctrl>c", "Cmd+C", Command::Copy, Where::App),
    ("<Ctrl>v", "Cmd+V", Command::Paste, Where::App),
    // Machine
    ("<Ctrl>Home", "Cmd+Home", Command::Home, Where::App),
    ("<Ctrl>U", "Cmd+U", Command::Unlock, Where::App),
    // Real-time commands
    ("!", "!", Command::FeedHold, Where::App),
    // craft_keymap has no `~` key: on the US layout `~` is Shift+`, which is how the chord
    // is stored and matched. On other layouts Shift+` is a different key from `~`.
    ("~", "Shift+`", Command::CycleStart, Where::App),
    // View: the visualizer's keys.
    ("f", "F", Command::ZoomFit, Where::Panel("Visualizer")),
    ("plus", "+", Command::ZoomIn, Where::Panel("Visualizer")),
    ("minus", "-", Command::ZoomOut, Where::Panel("Visualizer")),
    (
        "<Ctrl>0",
        "Cmd+0",
        Command::ResetView,
        Where::Panel("Visualizer"),
    ),
    // Help
    ("F1", "F1", Command::OpenHelp, Where::App),
];

/// Documented safety deviations: GTK chords (or GTK key handling) that get NO default binding
/// here, with the reason. Each command must have no default chord at all.
const UNBOUND_FOR_SAFETY: &[(&str, Command)] = &[
    // ESTOP "Escape" and MACHINE_RESET "<Ctrl>x" are accelerators.rs constants that GTK's
    // set_keyboard_shortcuts never registers, and its Reset menu item has no accelerator. A
    // bare Escape would reset the controller when closing a dropdown. Menu: Machine > Reset.
    ("Escape / <Ctrl>x", Command::SoftReset),
    // JOG_* "Right"/"Left"/"Up"/"Down"/"Page_Up"/"Page_Down" are unused accelerators.rs
    // constants; arrows move between widgets. GTK jogs from the numeric keypad in
    // machine_control/mod.rs (8 Y+, 2 Y-, 4 X-, 6 X+, 9 Z+, 3 Z-), but craft_keymap folds
    // keypad digits into the top-row digits, which must not jog, so jog has no default keys.
    ("Right / KP_6", Command::JogXPos),
    ("Left / KP_4", Command::JogXNeg),
    ("Up / KP_8", Command::JogYPos),
    ("Down / KP_2", Command::JogYNeg),
    ("Page_Up / KP_9", Command::JogZPos),
    ("Page_Down / KP_3", Command::JogZNeg),
];

#[test]
fn test_every_gtk_accelerator_bound() {
    // "Bound" means `Keymap::lookup` resolves the chord to the command. Panel-scoped keys
    // also need the pane to report keyboard focus (M2) before they fire from the keyboard,
    // and Cut/Copy/Paste reach egui as clipboard events rather than through the dispatcher.
    let keymap = commands::keymap();
    for (gtk, command) in UNBOUND_FOR_SAFETY {
        assert_eq!(
            keymap.default_bindings(&command.command_ref()),
            Vec::new(),
            "GTK {gtk}: {command:?} must have no default key"
        );
    }
    for (gtk, text, command, place) in GTK_ACCELERATORS {
        let chord = Chord::parse(text).expect("canonical chord");
        assert_eq!(chord.to_string(), *text, "{gtk}: {text:?} is not canonical");
        let active = match place {
            Where::App => Active::NONE,
            Where::Panel(panel) => Active::panel(panel),
        };
        assert_eq!(
            keymap.lookup(&chord, false, &active),
            Lookup::Run(command.command_ref()),
            "GTK {gtk} ({text})"
        );
        if let Where::Panel(_) = place {
            assert_eq!(
                keymap.lookup(&chord, false, &Active::NONE),
                Lookup::None,
                "GTK {gtk} ({text}) leaks out of its panel"
            );
        }
    }
}

#[test]
fn test_no_keymap_conflicts() {
    let keymap = commands::keymap();
    assert_eq!(keymap.conflicts(), Vec::new());
    assert!(keymap.problems().is_empty(), "{:?}", keymap.problems());
    assert_eq!(keymap.catalog().len(), Command::ALL.len());
}

#[test]
fn test_command_refs_round_trip() {
    for command in Command::ALL {
        assert_eq!(Command::from_ref(&command.command_ref()), Some(command));
    }
    assert_eq!(Command::from_ref(&"no.such".into()), None);
}

#[test]
fn test_menu_contains_every_command() {
    let tr = Translator::new("en");
    let bar = menu::menu_bar(&tr);
    let options = LayoutOptions::new("GCodeKit");
    let laid_out = [Target::Linux, Target::Windows, Target::Mac]
        .map(|target| (target, craft_menu::layout(&bar, target, &options).bar));
    for command in Command::ALL {
        let wanted = bar.find(&command.command_ref()).is_some();
        assert_eq!(wanted, !command.is_jog(), "{command:?} in the menu bar");
        for (target, bar) in &laid_out {
            assert_eq!(
                bar.find(&command.command_ref()).is_some(),
                !command.is_jog(),
                "{command:?} on {target:?}"
            );
        }
    }
}

#[test]
fn test_menu_shows_keymap_shortcuts() {
    let tr = Translator::new("en");
    let mut bar = menu::menu_bar(&tr);
    craft_menu::apply_keymap(&mut bar, &commands::keymap());
    let open = bar
        .find(&Command::FileOpen.command_ref())
        .expect("File > Open");
    assert_eq!(open.shortcut, Some(Chord::parse("Cmd+O").expect("chord")));
}

#[test]
fn test_menu_labels_translated() {
    let tr = Translator::new("de");
    let bar = menu::menu_bar(&tr);
    let titles: Vec<&str> = bar.menus.iter().map(|m| m.title.as_str()).collect();
    assert!(titles.contains(&"Datei"), "{titles:?}");
    let open = bar
        .find(&Command::FileOpen.command_ref())
        .expect("File > Open");
    assert_eq!(open.label, "\u{d6}ffnen");
}

#[test]
fn test_pane_request_queues_command() {
    let tr = Translator::new("en");
    let mut queue = Vec::new();
    let mut ctx = PaneContext::new(&tr, &mut queue);
    ctx.request(Command::Connect);
    assert_eq!(queue, vec![Command::Connect]);
}

#[test]
fn test_deferred_commands_are_recorded() {
    let mut app = GcodeKitApp::from_state(AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    ));
    for command in [Command::Connect, Command::JogXPos, Command::ViewTop] {
        app.dispatch(command);
        assert_eq!(app.last_dispatched(), Some(command));
    }
    assert_eq!(app.dialogs().waiting(), 0);
}

#[test]
fn test_ctrl_o_dispatches_file_open() {
    let mut app = GcodeKitApp::from_state(AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    ));
    {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1280.0, 800.0))
            .build_ui(|ui| app.ui(ui));
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::O);
        harness.run();
    }
    assert_eq!(app.last_dispatched(), Some(Command::FileOpen));
    assert_eq!(app.dialogs().waiting(), 1, "Open dialog queued");

    // The queued request is shown at the end of a frame (craft-dialogs' override answers it
    // here) and its answer is taken on the next frame.
    let path = std::path::PathBuf::from("/work/part.nc");
    let answer = path.clone();
    app.dialogs_mut().set_override(Some(Box::new(move |_| {
        craft_dialogs::Answer::Paths(vec![answer.clone()])
    })));
    app.dialogs_mut().end_frame(None::<&eframe::Frame>);
    {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1280.0, 800.0))
            .build_ui(|ui| app.ui(ui));
        harness.run();
    }
    assert_eq!(
        app.last_dialog_answer(),
        Some(&DialogAnswer::File {
            purpose: FilePurpose::OpenGcode,
            path,
        })
    );
}

#[test]
fn test_settings_and_about_open_windows() {
    use egui_kittest::kittest::Queryable;
    let mut app = GcodeKitApp::from_state(AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    ));
    app.dispatch(Command::OpenSettings);
    app.dispatch(Command::About);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui(|ui| app.ui(ui));
    harness.run();
    let _ = harness.get_by_label("Measurement System");
    let version = format!("Version {}", env!("CARGO_PKG_VERSION"));
    let _ = harness.get_by_label(&version);
}

#[test]
fn test_menu_click_dispatches() {
    use egui_kittest::kittest::Queryable;
    let mut app = GcodeKitApp::from_state(AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    ));
    {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1280.0, 800.0))
            .build_ui(|ui| app.ui(ui));
        harness.run();
        harness.get_by_label("Machine").click();
        harness.run();
        harness.get_by_label("Connect").click();
        harness.run();
    }
    assert_eq!(app.last_dispatched(), Some(Command::Connect));
}

#[test]
fn test_menu_in_title_strip_when_app_draws_frame() {
    use craft_window::policy::{Backend, Desktop, DesktopKind, Frame, Os, TitleBarPref, decide};
    use egui_kittest::kittest::Queryable;
    let desktop = Desktop {
        kind: DesktopKind::Gnome,
        ssd: Some(false),
        gtk_theme: None,
        icon_theme: None,
    };
    let decision = decide(
        Os::Linux,
        Backend::Wayland,
        &desktop,
        TitleBarPref::Integrated,
        None,
    );
    assert!(matches!(decision.frame, Frame::Integrated(_)));
    let mut app = GcodeKitApp::from_state(AppState::new(
        SettingsManager::detached(Config::default()),
        decision,
    ));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui(|ui| app.ui(ui));
    harness.run();
    let file = harness.get_by_label("File").rect();
    let close = harness.get_by_label("Close").rect();
    assert!(
        file.max.y <= gcodekit5_ui_egui::frame::TITLE_BAR_HEIGHT + 8.0,
        "File title at {file:?} is below the title strip"
    );
    assert!(
        file.max.x <= close.min.x,
        "File title overlaps the captions"
    );
}
