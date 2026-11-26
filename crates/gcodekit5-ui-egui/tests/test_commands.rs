//! The command catalog, its keymap and its menus: every GTK accelerator is bound to the same
//! command, the keymap has no conflicts, every command is reachable from a menu, a key, a page
//! tab or a page control, and a key press runs its command in the app.

use craft_keymap::{Active, Chord, Lookup};
use craft_menu::{LayoutOptions, Target};
use egui_kittest::Harness;
use gcodekit5_app::i18n::Translator;
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::commands::{self, Command};
use gcodekit5_ui_egui::dialogs::{DialogAnswer, FilePurpose};
use gcodekit5_ui_egui::menu;
use gcodekit5_ui_egui::workspace::{Page, PaneContext};
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
    // GTK's ~ was app-wide; here a bare motion key works only in Machine Control.
    (
        "~",
        "Shift+`",
        Command::CycleStart,
        Where::Panel(commands::MACHINE_CONTROL_SCOPE),
    ),
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
    // Designer: `designer/ui_builders.rs`'s canvas keys, on the canvas only.
    (
        "<Alt>l",
        "Alt+L",
        Command::AlignLeft,
        Where::Panel("Designer"),
    ),
    (
        "<Alt>r",
        "Alt+R",
        Command::AlignRight,
        Where::Panel("Designer"),
    ),
    (
        "<Alt>t",
        "Alt+T",
        Command::AlignTop,
        Where::Panel("Designer"),
    ),
    // GTK's <Alt>b and <Alt>h are menu-title mnemonics of the in-window bar (German Edit,
    // Help), so they moved to Alt+O and Alt+C
    // (`test_designer_alt_keys_leave_the_menu_mnemonics`).
    (
        "<Alt>b",
        "Alt+O",
        Command::AlignBottom,
        Where::Panel("Designer"),
    ),
    (
        "<Alt>h",
        "Alt+C",
        Command::AlignCenterX,
        Where::Panel("Designer"),
    ),
    (
        "<Alt>v",
        "Alt+V",
        Command::AlignCenterY,
        Where::Panel("Designer"),
    ),
    // The designer canvas's other keys (`designer/ui_builders.rs`): Ctrl+D duplicates, the
    // view keys, and Ctrl+I imports an image.
    (
        "<Ctrl>d",
        "Cmd+D",
        Command::Duplicate,
        Where::Panel("Designer"),
    ),
    ("plus", "+", Command::ZoomIn, Where::Panel("Designer")),
    ("equal", "=", Command::ZoomIn, Where::Panel("Designer")),
    ("minus", "-", Command::ZoomOut, Where::Panel("Designer")),
    ("f", "F", Command::ZoomFit, Where::Panel("Designer")),
    ("r", "R", Command::ResetView, Where::Panel("Designer")),
    ("d", "D", Command::FitDevice, Where::Panel("Designer")),
    (
        "<Ctrl>i",
        "Cmd+I",
        Command::FileImportImage,
        Where::Panel("Designer"),
    ),
    // Jog: machine_control/mod.rs's numeric keypad keys (`key.to_unicode()`, so the top-row
    // digits jogged too; egui reports both as the digit), in Machine Control only.
    (
        "KP_6",
        "6",
        Command::JogXPos,
        Where::Panel("MachineControl"),
    ),
    (
        "KP_4",
        "4",
        Command::JogXNeg,
        Where::Panel("MachineControl"),
    ),
    (
        "KP_8",
        "8",
        Command::JogYPos,
        Where::Panel("MachineControl"),
    ),
    (
        "KP_2",
        "2",
        Command::JogYNeg,
        Where::Panel("MachineControl"),
    ),
    (
        "KP_9",
        "9",
        Command::JogZPos,
        Where::Panel("MachineControl"),
    ),
    (
        "KP_3",
        "3",
        Command::JogZNeg,
        Where::Panel("MachineControl"),
    ),
];

/// Safety chords with no GTK accelerator: feed hold and soft reset each get a modifier chord
/// that works anywhere, also while a text field has the keyboard. GTK's ESTOP "Escape" and
/// MACHINE_RESET "<Ctrl>x" were accelerators.rs constants it never registered; bare Escape
/// stays unbound (it would reset the controller when closing a dropdown) and Ctrl+X is Cut.
const SAFETY_CHORDS: &[(&str, Command)] = &[
    ("Cmd+Shift+H", Command::FeedHold),
    ("Cmd+Shift+K", Command::SoftReset),
];

/// Documented safety deviation: keys that run nothing, anywhere. GTK's JOG_* "Right",
/// "Left", "Up", "Down", "Page_Up" and "Page_Down" were unused accelerators.rs constants;
/// arrows move between widgets, so they jog nothing, anywhere.
const UNBOUND_FOR_SAFETY: &[&str] = &["Right", "Left", "Up", "Down", "PageUp", "PageDown"];

#[test]
fn test_every_gtk_accelerator_bound() {
    // "Bound" means `Keymap::lookup` resolves the chord to the command. Panel-scoped keys
    // also need the pane to report keyboard focus (M2) before they fire from the keyboard,
    // and Cut/Copy/Paste reach egui as clipboard events rather than through the dispatcher.
    let keymap = commands::keymap();
    for text in UNBOUND_FOR_SAFETY {
        let chord = Chord::parse(text).expect("chord");
        for active in [Active::NONE, Active::panel(commands::MACHINE_CONTROL_SCOPE)] {
            assert_eq!(
                keymap.lookup(&chord, false, &active),
                Lookup::None,
                "{text} must run nothing"
            );
        }
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
fn test_safety_chords_bound_everywhere() {
    let keymap = commands::keymap();
    for (text, command) in SAFETY_CHORDS {
        let chord = Chord::parse(text).expect("canonical chord");
        assert_eq!(chord.to_string(), *text, "{text:?} is not canonical");
        assert!(!keymap.is_reserved(&chord), "{text} is reserved");
        for platform in [
            craft_keymap::Platform::Linux,
            craft_keymap::Platform::Windows,
            craft_keymap::Platform::Mac,
        ] {
            assert!(
                !craft_keymap::reserved::system(platform).contains(&chord),
                "{text} is reserved by the OS on {platform:?}"
            );
            assert!(
                !craft_keymap::text_editing_chords(platform).contains(&chord.effective(platform)),
                "{text} edits text on {platform:?}"
            );
        }
        for active in [
            Active::NONE,
            Active {
                typing: true,
                ..Active::NONE
            },
            Active {
                typing: true,
                ..Active::panel(commands::MACHINE_CONTROL_SCOPE)
            },
        ] {
            assert_eq!(
                keymap.lookup(&chord, false, &active),
                Lookup::Run(command.command_ref()),
                "{text} with {active:?}"
            );
        }
    }
}

/// The safety chords are not one slip from starting motion: each chord without Shift, and
/// without Cmd (Ctrl off macOS), runs no start, run, jog, cycle-start or motion command.
#[test]
fn test_safety_chords_not_near_motion() {
    let keymap = commands::keymap();
    let forbidden = |command: Command| {
        command.is_jog()
            || matches!(
                command,
                Command::FileRun | Command::StreamStart | Command::CycleStart | Command::Home
            )
    };
    let chords: Vec<String> = [Command::FeedHold, Command::SoftReset]
        .into_iter()
        .flat_map(|c| keymap.default_bindings(&c.command_ref()))
        .map(|b| b.text(&craft_keymap::Scope::app()))
        .filter(|text| text.contains("Shift+"))
        .collect();
    assert_eq!(chords.len(), 2, "{chords:?}");
    for text in &chords {
        let key = text.rsplit('+').next().expect("key");
        for near in [
            format!("Cmd+{key}"),
            format!("Shift+{key}"),
            key.to_string(),
        ] {
            let chord = Chord::parse(&near).expect("chord");
            for active in [
                Active::NONE,
                Active::panel(commands::MACHINE_CONTROL_SCOPE),
                Active::panel(commands::VISUALIZER_SCOPE),
            ] {
                match keymap.lookup(&chord, false, &active) {
                    Lookup::Run(command) => {
                        let command = Command::from_ref(&command).expect("our command");
                        assert!(!forbidden(command), "{near} (near {text}) runs {command:?}");
                    }
                    Lookup::Cycle(commands) => panic!("{near} cycles {commands:?}"),
                    Lookup::None => {}
                    other => panic!("{near}: {other:?}"),
                }
            }
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

/// The commands the menus leave out, each with the page control that runs it instead (its
/// page and accessible label). GTK's menus have no View or Tools menu: its views are the
/// page tabs and the visualizer's own buttons, and jogging is in Machine Control.
const PAGE_CONTROLS: &[(Command, Page, &str)] = &[
    (Command::JogXPos, Page::MachineControl, "Jog X+"),
    (Command::JogXNeg, Page::MachineControl, "Jog X-"),
    (Command::JogYPos, Page::MachineControl, "Jog Y+"),
    (Command::JogYNeg, Page::MachineControl, "Jog Y-"),
    (Command::JogZPos, Page::MachineControl, "Jog Z+"),
    (Command::JogZNeg, Page::MachineControl, "Jog Z-"),
    (Command::ZoomFit, Page::Visualizer, "Fit to Content"),
    (Command::ZoomIn, Page::Visualizer, "Zoom In"),
    (Command::ZoomOut, Page::Visualizer, "Zoom Out"),
    (Command::ViewTop, Page::Visualizer, "Top View"),
    (Command::ViewFront, Page::Visualizer, "Front View"),
    (Command::ViewIso, Page::Visualizer, "Isometric (SE)"),
    (Command::ResetView, Page::Visualizer, "Reset View"),
    (
        Command::FitDevice,
        Page::Designer,
        "Fit to Device Working Area",
    ),
];

/// The designer shape commands, with the label of their entry in the canvas context menu
/// (GTK's menus had none of them): the Align submenu's entries, Group and Ungroup.
const DESIGNER_MENU: &[(Command, &str)] = &[
    (Command::AlignLeft, "Align Left"),
    (Command::AlignRight, "Align Right"),
    (Command::AlignTop, "Align Top"),
    (Command::AlignBottom, "Align Bottom"),
    (Command::AlignCenterX, "Align Center X"),
    (Command::AlignCenterY, "Align Center Y"),
    (Command::Group, "Group"),
    (Command::Ungroup, "Ungroup"),
    (Command::Duplicate, "Duplicate"),
];

/// The menus hold every command but those with a page control ([`PAGE_CONTROLS`]), those a
/// page tab opens ([`Page::for_command`]) and the designer's ([`DESIGNER_MENU`]), on every
/// platform layout. No command is
/// key-only since Reset View got its sidebar button (it keeps `Cmd+0`).
#[test]
fn test_menu_contains_every_command() {
    let tr = Translator::new("en");
    let bar = menu::menu_bar(&tr);
    let options = LayoutOptions::new("GCodeKit");
    let laid_out = [Target::Linux, Target::Windows, Target::Mac]
        .map(|target| (target, craft_menu::layout(&bar, target, &options).bar));
    for command in Command::ALL {
        let elsewhere = Page::for_command(command).is_some()
            || PAGE_CONTROLS.iter().any(|(c, _, _)| *c == command)
            || DESIGNER_MENU.iter().any(|(c, _)| *c == command);
        assert_eq!(
            command.is_designer_shape_op(),
            DESIGNER_MENU.iter().any(|(c, _)| *c == command),
            "{command:?}"
        );
        let wanted = bar.find(&command.command_ref()).is_some();
        assert_eq!(wanted, !elsewhere, "{command:?} in the menu bar");
        for (target, bar) in &laid_out {
            assert_eq!(
                bar.find(&command.command_ref()).is_some(),
                !elsewhere,
                "{command:?} on {target:?}"
            );
        }
    }
}

/// The menus are GTK's four: File, Edit, Machine, Help.
#[test]
fn test_menus_are_gtks_four() {
    let tr = Translator::new("en");
    let bar = menu::menu_bar(&tr);
    let titles: Vec<&str> = bar.menus.iter().map(|m| m.title.as_str()).collect();
    assert_eq!(titles, ["File", "Edit", "Machine", "Help"]);
}

/// No command is lost: each is in a menu, has a default key, is opened by a page tab, or has
/// a control on its page, which is checked to be there.
#[test]
fn test_every_command_is_reachable() {
    use egui_kittest::kittest::Queryable;
    let tr = Translator::new("en");
    let bar = menu::menu_bar(&tr);
    let keymap = commands::keymap();
    let state = AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    );
    let mut h = crate::machine_support::app_harness(GcodeKitApp::from_state(state));
    h.run_steps(2);
    for command in Command::ALL {
        if let Some((_, label)) = DESIGNER_MENU.iter().find(|(c, _)| *c == command) {
            assert!(
                designer_menu_has(&mut h, label),
                "{command:?}: no {label:?} in the designer's context menu"
            );
            assert!(
                !keymap.bindings(&command.command_ref()).is_empty(),
                "{command:?} keeps its GTK key"
            );
            continue;
        }
        if let Some((_, page, label)) = PAGE_CONTROLS.iter().find(|(c, _, _)| *c == command) {
            h.state_mut().show_page(*page);
            h.run_steps(2);
            // Fit to Content and Top View are each on two buttons, as in GTK.
            assert!(
                h.query_all_by_label(label).next().is_some(),
                "{command:?}: no {label:?} on {page:?}"
            );
            continue;
        }
        if let Some(page) = Page::for_command(command) {
            let label = tr.tr(page.title_msgid()).into_owned();
            h.get_by_role_and_label(egui::accesskit::Role::Tab, &label)
                .click();
            h.run_steps(2);
            assert_eq!(h.state().page(), page, "{command:?}");
            continue;
        }
        let in_menu = bar.find(&command.command_ref()).is_some();
        let has_key = !keymap.bindings(&command.command_ref()).is_empty();
        assert!(
            in_menu || has_key,
            "{command:?} is in no menu, has no key and no page control"
        );
    }
}

/// Whether the designer's context menu, opened over two selected squares (grouped for
/// "Ungroup"), has `label` (in its Align submenu when the label starts with "Align ").
/// Closes the menu again.
fn designer_menu_has(h: &mut Harness<'static, GcodeKitApp>, label: &str) -> bool {
    use egui_kittest::kittest::Queryable;
    use gcodekit5_designer::model::{DesignRectangle, Shape};
    h.state_mut().show_page(Page::Designer);
    h.run_steps(2);
    let designer = std::rc::Rc::clone(h.state().designer());
    if designer.borrow().state().canvas.shape_count() == 0 {
        for x in [0.0, 30.0] {
            designer
                .borrow_mut()
                .state_mut()
                .add_shape_with_undo(Shape::Rectangle(DesignRectangle::new(x, 0.0, 10.0, 10.0)));
        }
    }
    designer.borrow_mut().state_mut().select_all();
    // Group is offered for an ungrouped selection, Ungroup for a grouped one.
    if label == "Ungroup" {
        designer.borrow_mut().state_mut().group_selected();
    } else {
        designer.borrow_mut().state_mut().ungroup_selected();
    }
    h.run_steps(2);
    // The Layers pane has Group and Ungroup buttons too: the menu adds one more.
    let before = h.query_all_by_label(label).count();
    let p = designer
        .borrow()
        .view()
        .world_to_screen(glam::Vec2::new(5.0, 5.0));
    crate::test_designer_canvas::click_with(
        h,
        p,
        egui::PointerButton::Secondary,
        egui::Modifiers::NONE,
    );
    if label.starts_with("Align ") {
        h.get_by_label("Align").click();
        h.run();
    }
    let found = h.query_all_by_label(label).count() > before;
    h.key_press(egui::Key::Escape);
    h.run();
    found
}

/// A command that used to open a manager window now shows its page.
#[test]
fn test_manager_commands_switch_page() {
    let mut app = GcodeKitApp::from_state(AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    ));
    for command in Command::ALL {
        if let Some(page) = Page::for_command(command) {
            app.dispatch(command);
            assert_eq!(app.page(), page, "{command:?}");
        }
    }
}

#[test]
fn test_menu_shows_keymap_shortcuts() {
    let tr = Translator::new("en");
    let mut bar = menu::menu_bar(&tr);
    menu::apply_shortcuts(&mut bar, &commands::keymap());
    let open = bar
        .find(&Command::FileOpen.command_ref())
        .expect("File > Open");
    assert_eq!(open.shortcut, Some(Chord::parse("Cmd+O").expect("chord")));
}

/// A menu item shows only a key that works on every page: Cycle Start's `~` works only while
/// Machine Control has the keyboard, so its item shows no key, while Feed Hold and Reset keep
/// theirs.
#[test]
fn test_menu_shows_only_keys_that_work_on_every_page() {
    let tr = Translator::new("en");
    let keymap = commands::keymap();
    let mut bar = menu::menu_bar(&tr);
    menu::apply_shortcuts(&mut bar, &keymap);
    let shortcut = |command: Command| {
        bar.find(&command.command_ref())
            .expect("menu item")
            .shortcut
    };
    assert_eq!(shortcut(Command::CycleStart), None);
    assert!(shortcut(Command::FeedHold).is_some());
    assert_eq!(
        shortcut(Command::SoftReset),
        Some(Chord::parse("Cmd+Shift+K").expect("chord"))
    );
    for item in bar.items() {
        let Some(chord) = item.shortcut else {
            continue;
        };
        let command = Command::from_ref(&item.command).expect("our command");
        assert_eq!(
            keymap.lookup(&chord, false, &Active::NONE),
            Lookup::Run(command.command_ref()),
            "{command:?} shows {chord} but it does not run from every page"
        );
    }
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
        // The menu item, not the Machine Control pane's Connect button.
        harness
            .get_by_role_and_label(egui::accesskit::Role::MenuItem, "Connect")
            .click();
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

/// The access keys (mnemonics) the in-window menu bar gives its titles in `language`.
fn title_mnemonics(language: &str) -> Vec<String> {
    use egui_kittest::kittest::{NodeT, Queryable};
    let mut config = Config::default();
    config.ui.language = language.to_string();
    let state = AppState::new(
        SettingsManager::detached(config),
        crate::test_smoke::native_decision(),
    );
    let mut h = crate::machine_support::app_harness(GcodeKitApp::from_state(state));
    h.run_steps(2);
    let tr = Translator::new(language);
    // The titles as the menus define them, not a copy of their msgids.
    menu::menu_bar(&tr)
        .menus
        .iter()
        .filter_map(|m| {
            let node = h.get_by_role_and_label(egui::accesskit::Role::MenuItem, &m.title);
            node.accesskit_node()
                .data()
                .access_key()
                .map(str::to_lowercase)
        })
        .collect()
}

/// The designer's Alt keys never take a menu title's mnemonic from the in-window bar, in any
/// shipped language (the keymap runs before the bar, so it would win while the canvas has the
/// keyboard).
#[test]
fn test_designer_alt_keys_leave_the_menu_mnemonics() {
    let keymap = commands::keymap();
    let alt_letters: Vec<String> = Command::ALL
        .into_iter()
        .filter(|c| c.is_designer_shape_op())
        .flat_map(|c| keymap.default_bindings(&c.command_ref()))
        .map(|b| b.text(&craft_keymap::Scope::new("Designer").expect("scope")))
        .filter_map(|text| text.strip_prefix("Alt+").map(str::to_lowercase))
        .collect();
    assert_eq!(alt_letters.len(), 6, "{alt_letters:?}");
    for (language, table) in menu::MENU_MNEMONICS {
        let mnemonics = title_mnemonics(language);
        assert_eq!(mnemonics.len(), 4, "{language}: {mnemonics:?}");
        // The shortcut editor refuses Alt keys by this table.
        let table: Vec<String> = table.iter().map(char::to_string).collect();
        assert_eq!(
            mnemonics, table,
            "{language}: menu::MENU_MNEMONICS is stale"
        );
        for letter in &alt_letters {
            assert!(
                !mnemonics.contains(letter),
                "{language}: Alt+{letter} is a menu mnemonic ({mnemonics:?})"
            );
        }
    }
}

/// `menu::MENU_MNEMONICS` has a row for every shipped language (English and each po file),
/// and each letter is in its menu title as the menus define it in that language.
#[test]
fn test_mnemonic_table_covers_every_shipped_language() {
    let po_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../po");
    let mut shipped: Vec<String> = std::fs::read_dir(po_dir)
        .expect("po folder")
        .filter_map(|e| {
            let path = e.ok()?.path();
            (path.extension()? == "po").then(|| path.file_stem()?.to_str().map(str::to_string))?
        })
        .collect();
    shipped.push("en".to_string());
    shipped.sort();
    let mut table: Vec<String> = menu::MENU_MNEMONICS
        .iter()
        .map(|(language, _)| (*language).to_string())
        .collect();
    table.sort();
    assert_eq!(table, shipped);
    for (language, letters) in menu::MENU_MNEMONICS {
        let titles: Vec<String> = menu::menu_bar(&Translator::new(language))
            .menus
            .iter()
            .map(|m| m.title.to_lowercase())
            .collect();
        assert_eq!(titles.len(), letters.len(), "{language}");
        for (title, letter) in titles.iter().zip(letters) {
            assert!(
                title.contains(letter),
                "{language}: {letter} not in {title}"
            );
        }
    }
}
