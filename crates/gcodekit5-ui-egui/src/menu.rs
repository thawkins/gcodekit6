//! The menus: built from the command catalog, laid out by `craft_menu` for the platform, and
//! shown natively on macOS or as an in-window bar elsewhere.
//!
//! [`menu_bar`] is the app's own menu tree: GTK's File, Edit, Machine and Help. [`Menus`] keeps
//! it laid out for [`Target::current`] with the keymap's shortcuts, and either draws it with
//! `craft_menu::MenuBarUi` (Windows, Linux, and macOS when AppKit refused the native menus) or
//! drives the macOS `NativeMenu`, whose key equivalents come back to egui as key presses so the
//! keymap runs them like any other key.
//!
//! Dependencies: `egui`, `craft-menu` (model, layouts, egui bar, macOS backend), `craft-keymap`
//! (shortcuts), `gcodekit5-app` (translations).

use craft_keymap::Keymap;
use craft_menu::{
    BarStyle, Gate, ItemRole, LayoutOptions, Menu, MenuBar, MenuBarUi, MenuRole, Node, Target,
};
use gcodekit5_app::i18n::Translator;

use crate::commands::Command;

/// The application name the layouts write into About, Hide and Quit.
pub const APP_NAME: &str = "GCodeKit";

/// English labels `craft_menu` writes itself and translates through [`LayoutOptions::label`]
/// and [`MenuBarUi::label`], as listed in `po/gcodekit5.pot`. `{app}` is replaced by
/// [`APP_NAME`] after translation. The overflow title (U+22EF) is drawn as an ellipsis.
pub const LAYOUT_LABELS: [&str; 25] = [
    n_!("About {app}"),
    n_!("Settings…"),
    n_!("Settings"),
    n_!("Language"),
    n_!("Appearance"),
    n_!("Hide {app}"),
    n_!("Hide Others"),
    n_!("Quit {app}"),
    n_!("Minimize"),
    n_!("Preferences…"),
    n_!("Preferences"),
    n_!("Exit"),
    n_!("Quit"),
    n_!("File"),
    n_!("Edit"),
    n_!("Window"),
    n_!("Help"),
    n_!("Services"),
    n_!("Show All"),
    n_!("Zoom"),
    n_!("Bring All to Front"),
    n_!("Close Window"),
    n_!("Menu"),
    n_!("Scroll menu up"),
    n_!("Scroll menu down"),
];

/// The English msgids of the top-level menu titles, in bar order: GTK's four.
pub const MENU_TITLES: [&str; 4] = [n_!("File"), n_!("Edit"), n_!("Machine"), n_!("Help")];

/// The access keys (mnemonics) the in-window menu bar gives [`MENU_TITLES`], in bar order, for
/// every shipped language. Alt and one of these letters opens a menu, so the shortcut editor
/// gives no command such a key. `test_commands::test_designer_alt_keys_leave_the_menu_mnemonics`
/// checks the table against the bar.
pub const MENU_MNEMONICS: [(&str, [char; 4]); 6] = [
    ("en", ['f', 'e', 'm', 'h']),
    ("de", ['d', 'b', 'm', 'h']),
    ("es", ['a', 'e', 'm', 'y']),
    ("fr", ['f', 'd', 'm', 'a']),
    ("it", ['f', 'm', 'a', 'i']),
    ("pt", ['f', 'e', 'm', 'a']),
];

/// A menu item running `command`, labelled in the current language.
fn item(tr: &Translator, command: Command) -> Node {
    craft_menu::Item::new(command.command_ref(), tr.tr(command.label_msgid())).into()
}

/// A top-level menu titled `title`: the commands in order, `None` for a separator.
fn menu(tr: &Translator, title: &str, role: MenuRole, rows: &[Option<Command>]) -> Menu {
    let children = rows
        .iter()
        .map(|row| match row {
            Some(command) => item(tr, *command),
            None => Node::Separator,
        })
        .collect();
    Menu::new(tr.tr(title), children).role(role)
}

/// The app's menus, labelled through `tr`, before any platform layout and without shortcuts
/// ([`apply_shortcuts`] adds them): GTK's File, Edit, Machine and Help. The commands
/// left out have a home elsewhere: jogging and the visualizer's view commands are buttons on
/// their pages (Reset View also keeps its `Cmd+0` key), and the manager windows of earlier
/// builds are page tabs ([`crate::workspace::Page::for_command`]). About, Preferences and
/// Quit are tagged so each layout puts them where the platform expects.
///
/// # Arguments
/// * `tr` - Translator for the labels.
pub fn menu_bar(tr: &Translator) -> MenuBar {
    use Command as C;
    let [file, edit, machine, help] = MENU_TITLES;
    let menus = vec![
        menu(
            tr,
            file,
            MenuRole::File,
            &[
                Some(C::FileNew),
                Some(C::FileNew3d),
                Some(C::FileOpen),
                Some(C::FileSave),
                Some(C::FileSaveAs),
                None,
                Some(C::FileImport),
                Some(C::FileImportImage),
                Some(C::FileExportGcode),
                Some(C::FileExportSvg),
                None,
                Some(C::FileRun),
                None,
                Some(C::Quit),
            ],
        ),
        menu(
            tr,
            edit,
            MenuRole::Edit,
            &[
                Some(C::Undo),
                Some(C::Redo),
                None,
                Some(C::Cut),
                Some(C::Copy),
                Some(C::Paste),
                None,
                Some(C::OpenSettings),
            ],
        ),
        menu(
            tr,
            machine,
            MenuRole::Other,
            &[
                Some(C::Connect),
                Some(C::Disconnect),
                None,
                Some(C::StreamStart),
                Some(C::StreamPause),
                Some(C::StreamStop),
                None,
                Some(C::FeedHold),
                Some(C::CycleStart),
                None,
                Some(C::Home),
                Some(C::Unlock),
                Some(C::SoftReset),
                None,
                Some(C::OpenDeviceInfo),
            ],
        ),
        menu(
            tr,
            help,
            MenuRole::Help,
            &[Some(C::OpenHelp), None, Some(C::About)],
        ),
    ];
    let mut bar = MenuBar {
        menus,
        ..MenuBar::default()
    };
    bar.tag_item(&C::About.command_ref(), ItemRole::About);
    bar.tag_item(&C::OpenSettings.command_ref(), ItemRole::Settings);
    bar.tag_item(&C::Quit.command_ref(), ItemRole::Quit);
    bar
}

/// Puts `keymap`'s shortcuts on `bar`'s items (`craft_menu::apply_keymap`), then shows on each
/// item only a key that works on every page. A key bound in one pane's scope only (Cycle
/// Start's `~`, which works only while Machine Control has the keyboard) would promise a key
/// that does nothing elsewhere, so such an item shows no key; Preferences' Shortcuts page
/// lists it with where it works. GTK's menus had no such item.
///
/// # Arguments
/// * `bar` - The menus to label.
/// * `keymap` - The keymap whose bindings the items show.
pub fn apply_shortcuts(bar: &mut MenuBar, keymap: &Keymap) {
    craft_menu::apply_keymap(bar, keymap);
    for menu in &mut bar.menus {
        for node in &mut menu.children {
            let Node::Item(item) = node else {
                continue;
            };
            let Some(bindable) = keymap.bindable(&item.command) else {
                continue;
            };
            let app_wide = keymap.bindings(&item.command).into_iter().any(|b| {
                Some(b.trigger.chord) == item.shortcut && b.scope_or(&bindable.scope).is_app()
            });
            if !app_wide {
                item.shortcut = keymap
                    .bindings(&item.command)
                    .into_iter()
                    .find(|b| !b.trigger.double && b.scope_or(&bindable.scope).is_app())
                    .map(|b| b.trigger.chord);
            }
        }
    }
}

/// The overflow menu's title, which egui's default fonts cannot draw.
const OVERFLOW: &str = "\u{22ef}";

/// Translates a label `craft_menu` writes itself. The overflow title becomes a horizontal
/// ellipsis, which egui's default fonts have.
fn layout_label(tr: &Translator, english: &'static str) -> String {
    if english == OVERFLOW {
        return "\u{2026}".to_owned();
    }
    tr.tr(english).into_owned()
}

/// The bar's look from `style`, with check and radio marks egui's default fonts can draw
/// (they have no U+2713 check mark or U+25CF circle).
fn bar_style(style: &egui::Style) -> BarStyle {
    BarStyle {
        check_mark: "\u{221a}".to_owned(),
        radio_mark: "\u{2022}".to_owned(),
        ..BarStyle::from_style(style)
    }
}

/// The commands among `chosen` that are ours; others (none today) are logged and dropped.
fn commands(chosen: Vec<craft_keymap::CommandRef>) -> Vec<Command> {
    chosen
        .into_iter()
        .filter_map(|command| {
            let ours = Command::from_ref(&command);
            if ours.is_none() {
                tracing::debug!(%command, "menu chose a command the app does not know");
            }
            ours
        })
        .collect()
}

/// The menus, laid out for this platform with the keymap's shortcuts.
pub struct Menus {
    /// The bar laid out for [`Target::current`], drawn in the window when there is no native
    /// menu.
    laid_out: MenuBar,
    /// Counts the changes of `laid_out`, so the native menu is laid out again after one.
    version: u64,
    /// The macOS menu bar, when AppKit accepted it.
    #[cfg(target_os = "macos")]
    native: Option<craft_menu::NativeMenu>,
}

impl std::fmt::Debug for Menus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Menus")
            .field("menus", &self.laid_out.menus.len())
            .field("native", &self.has_native())
            .finish()
    }
}

impl Menus {
    /// The menus for `keymap`'s shortcuts, labelled through `tr`, with no native menu yet.
    ///
    /// # Arguments
    /// * `tr` - Translator for the labels.
    /// * `keymap` - The keymap whose bindings the items show.
    pub fn new(tr: &Translator, keymap: &Keymap) -> Self {
        Self {
            laid_out: Self::layout(tr, keymap, Target::current()),
            version: 0,
            #[cfg(target_os = "macos")]
            native: None,
        }
    }

    /// Shows `keymap`'s shortcuts from now on, after the user changed them.
    ///
    /// # Arguments
    /// * `tr` - Translator for the labels.
    /// * `keymap` - The keymap whose bindings the items show.
    pub fn set_keymap(&mut self, tr: &Translator, keymap: &Keymap) {
        self.laid_out = Self::layout(tr, keymap, Target::current());
        self.version += 1;
    }

    /// The app's menus with `keymap`'s shortcuts, laid out for `target`.
    fn layout(tr: &Translator, keymap: &Keymap, target: Target) -> MenuBar {
        let mut bar = menu_bar(tr);
        apply_shortcuts(&mut bar, keymap);
        let label = |english: &'static str| layout_label(tr, english);
        let options = LayoutOptions {
            label: &label,
            ..LayoutOptions::new(APP_NAME)
        };
        let layout = craft_menu::layout(&bar, target, &options);
        for clash in &layout.clashes {
            tracing::debug!(
                shortcut = %clash.shortcut,
                system = clash.system,
                command = %clash.command,
                resolution = clash.resolution,
                "menu shortcut clash"
            );
        }
        layout.bar
    }

    /// Installs the macOS menu bar. Call once, from eframe's creation context; when AppKit
    /// refuses, the in-window bar stays.
    ///
    /// # Arguments
    /// * `ctx` - The egui context of the main window.
    #[cfg(target_os = "macos")]
    pub fn install_native(&mut self, ctx: &egui::Context) {
        self.native = craft_menu::mac::install(ctx, &self.laid_out);
        if self.native.is_none() {
            tracing::warn!("native menu bar refused; using the in-window menu bar");
        }
    }

    /// Whether the macOS menu bar is installed, so the window draws no menu bar of its own.
    pub fn has_native(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.native.is_some()
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    /// Feeds the native menu's key equivalents to egui as the key presses they were. Call from
    /// `eframe::App::raw_input_hook`, before egui reads the input.
    ///
    /// # Arguments
    /// * `raw` - The input egui is about to read.
    #[cfg(target_os = "macos")]
    pub fn raw_input(&mut self, raw: &mut egui::RawInput) {
        if let Some(native) = &mut self.native {
            native.raw_input(raw);
        }
    }

    /// The commands chosen from the native menu since the last frame. Empty without one.
    ///
    /// # Arguments
    /// * `ctx` - The egui context of the main window.
    /// * `gate` - What may run this frame.
    pub fn take_native(&mut self, ctx: &egui::Context, gate: Gate) -> Vec<Command> {
        #[cfg(target_os = "macos")]
        if let Some(native) = &mut self.native {
            native.set_gate(gate);
            let chosen = native.take_clicks(ctx);
            // The menus change only with the shortcuts, which bump `version`.
            let layout = self.laid_out.clone();
            native.sync_with(self.version, move || craft_menu::Layout {
                bar: layout,
                clashes: Vec::new(),
            });
            return commands(chosen);
        }
        let _ = (ctx, gate);
        Vec::new()
    }

    /// Draws the in-window menu bar into `ui`, taking at most `max_width` points, and returns
    /// the commands chosen this frame.
    ///
    /// # Arguments
    /// * `ui` - Where the bar goes: a top panel or the title-bar strip.
    /// * `tr` - Translator for the labels the bar writes itself.
    /// * `gate` - What may run this frame.
    /// * `max_width` - The widest the bar may be; titles past it overflow into a `⋯` menu.
    pub fn show(
        &self,
        ui: &mut egui::Ui,
        tr: &Translator,
        gate: Gate,
        max_width: f32,
    ) -> Vec<Command> {
        let label = |english: &'static str| layout_label(tr, english);
        let chosen = MenuBarUi::new(&self.laid_out)
            .id(egui::Id::unique("gcodekit5-menu-bar"))
            .gate(gate)
            .style(bar_style(ui.style()))
            .label(&label)
            .shrink_to_fit(max_width)
            .show(ui);
        commands(chosen)
    }
}

/// Every msgid the menus can show, for the translation guard test.
pub fn msgids() -> Vec<&'static str> {
    let commands = Command::ALL
        .into_iter()
        .filter(|c| !c.is_jog())
        .map(Command::label_msgid);
    MENU_TITLES
        .into_iter()
        .chain(commands)
        .chain(LAYOUT_LABELS)
        .collect()
}
