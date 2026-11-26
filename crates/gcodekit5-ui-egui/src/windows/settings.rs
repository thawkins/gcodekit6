//! The Preferences window: a copy of the settings edited page by page, saved by Apply and
//! dropped by Cancel.
//!
//! Ported from the GTK crate's `ui/gtk/settings.rs` and the rows
//! `gcodekit5_settings::persistence` fills it with. The rows are the GTK pages' rows that a
//! [`Config`] field backs and that mean something to this front end: General (units, feed rate
//! units, default folder), Machine (jog step, jog feed rate, X/Y/Z travel), User Interface
//! (theme, language, About on startup, grid line widths, and "Use system title bar" where
//! craft-window says it changes something), File Processing, Shortcuts and Advanced. A
//! language or title-bar change takes effect after a restart; [`SettingsOutcome::Applied`] says
//! when one is needed. GTK's Controller page had no rows, so there is none here.
//!
//! The Shortcuts page (documented in `docs/user/70-settings.md`; GTK's page was empty) lists
//! every command with its keys: Change records the next key press as the command's key, Clear
//! removes its keys, Reset gives it its defaults and Reset All every command. A key is given
//! only when [`crate::shortcuts`] allows it, which keeps the E-STOP and feed hold keys fixed;
//! otherwise the page says why. Apply saves the shortcuts with the settings
//! (`ui.keyboard_shortcuts`) and hands the edited keymap to the app
//! ([`SettingsWindow::take_applied_keymap`]), which uses it at once.
//!
//! Dependencies: `egui`, `craft-ui` (combo boxes), `craft-keymap` (the keymap and recording a
//! key), `gcodekit5-settings` (the settings and saving them), `gcodekit5-app` (translations).

use craft_keymap::{Key, KeyInput, Keymap, Layout, Mods, Platform, Trigger};
use craft_ui::combo::ComboBox;
use gcodekit5_app::i18n::{Translator, needs_restart, normalized_language};
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::{Config, FeedRateUnits, MeasurementSystem, Theme, UiSettings};

use crate::commands::{self, Command};
use crate::shortcuts::{self, LOCKED};

/// The UI languages offered, as `(code, name)`; names are each language's own, so they are not
/// translated. `"system"` is listed first under a translated name.
const LANGUAGES: [(&str, &str); 6] = [
    ("en", "English"),
    ("de", "Deutsch"),
    ("es", "Espa\u{f1}ol"),
    ("fr", "Fran\u{e7}ais"),
    ("it", "Italiano"),
    ("pt", "Portugu\u{ea}s"),
];

/// What [`SettingsWindow::show`] did this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsOutcome {
    /// The window is closed.
    Closed,
    /// The window is open and nothing was decided.
    Open,
    /// Cancel or the close button dropped the changes.
    Cancelled,
    /// Apply saved the changes and closed the window; `restart` when a change takes effect
    /// only after a restart.
    Applied {
        /// A restart is needed for a change to take effect.
        restart: bool,
    },
    /// Apply could not save; the window stays open and shows why.
    Failed(String),
}

/// A page of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    General,
    Machine,
    UserInterface,
    FileProcessing,
    Shortcuts,
    Advanced,
}

impl Page {
    const ALL: [Self; 6] = [
        Self::General,
        Self::Machine,
        Self::UserInterface,
        Self::FileProcessing,
        Self::Shortcuts,
        Self::Advanced,
    ];

    fn title_msgid(self) -> &'static str {
        match self {
            Self::General => n_!("General"),
            Self::Machine => n_!("Machine"),
            Self::UserInterface => n_!("User Interface"),
            Self::FileProcessing => n_!("File Processing"),
            Self::Shortcuts => n_!("Shortcuts"),
            Self::Advanced => n_!("Advanced"),
        }
    }
}

/// The copy being edited.
#[doc(hidden)]
#[derive(Debug)]
pub struct Draft {
    /// The settings as edited.
    pub config: Config,
    /// The default folder as typed.
    pub directory: String,
    /// Why the last Apply failed.
    error: Option<String>,
    /// The shortcuts as edited.
    keymap: Keymap,
    /// The command whose key the next key press sets.
    recording: Option<Command>,
    /// Why the last key was not given, or that it was.
    shortcut_notice: Option<String>,
    /// The Shortcuts page's filter text.
    filter: String,
}

/// The Preferences window.
#[derive(Debug)]
pub struct SettingsWindow {
    /// Whether the "Use system title bar" row changes anything on this desktop.
    title_bar_row: bool,
    /// The language the app runs in.
    launch_language: String,
    /// Whether the app was started with the system title bar.
    launch_system_title_bar: bool,
    page: Page,
    draft: Option<Draft>,
    /// The shortcuts the last Apply saved, until the app takes them.
    applied_keymap: Option<Keymap>,
}

impl SettingsWindow {
    /// A closed window.
    ///
    /// # Arguments
    /// * `title_bar_row` - Whether to offer "Use system title bar" (craft-window's decision
    ///   says whether it changes anything here).
    /// * `launch` - The UI settings the app was started with, which a restart would change.
    pub fn new(title_bar_row: bool, launch: &UiSettings) -> Self {
        Self {
            title_bar_row,
            launch_language: launch.language.clone(),
            launch_system_title_bar: launch.use_system_title_bar,
            page: Page::General,
            draft: None,
            applied_keymap: None,
        }
    }

    /// Opens the window on a copy of the current settings and shortcuts; does nothing when it
    /// is open.
    ///
    /// # Arguments
    /// * `settings` - The settings to edit.
    /// * `keymap` - The shortcuts in use.
    pub fn open(&mut self, settings: &SettingsManager, keymap: &Keymap) {
        if self.draft.is_none() {
            let config = settings.config().clone();
            let directory = config
                .file_processing
                .output_directory
                .to_string_lossy()
                .into_owned();
            self.draft = Some(Draft {
                config,
                directory,
                error: None,
                keymap: keymap.clone(),
                recording: None,
                shortcut_notice: None,
                filter: String::new(),
            });
        }
    }

    /// Whether the window is open.
    pub fn is_open(&self) -> bool {
        self.draft.is_some()
    }

    /// Whether the Shortcuts page waits for a key press to record. The app then runs no key
    /// but the E-STOP and feed hold keys, so the press reaches the window.
    pub fn is_recording(&self) -> bool {
        self.draft.as_ref().is_some_and(|d| d.recording.is_some())
    }

    /// The shortcuts the last Apply saved, once: the app uses them from then on.
    pub fn take_applied_keymap(&mut self) -> Option<Keymap> {
        self.applied_keymap.take()
    }

    /// The copy being edited while the window is open (tests: set every edited field at
    /// once).
    #[doc(hidden)]
    pub fn draft_mut(&mut self) -> Option<&mut Draft> {
        self.draft.as_mut()
    }

    /// Draws the window when it is open.
    ///
    /// # Arguments
    /// * `ctx` - The egui context.
    /// * `settings` - The settings Apply saves to.
    /// * `tr` - Translates the labels.
    ///
    /// # Returns
    /// What happened this frame.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        settings: &mut SettingsManager,
        tr: &Translator,
    ) -> SettingsOutcome {
        let Some(draft) = self.draft.as_mut() else {
            return SettingsOutcome::Closed;
        };
        if self.page != Page::Shortcuts {
            draft.recording = None;
        }
        record_key(ctx, tr, draft);
        let mut open = true;
        let mut action = None;
        let page = &mut self.page;
        let title_bar_row = self.title_bar_row;
        egui::Window::new(tr.tr(n_!("Preferences")))
            .id(egui::Id::unique("gcodekit5-preferences"))
            .open(&mut open)
            .collapsible(false)
            .default_size([560.0, 420.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for candidate in Page::ALL {
                        ui.selectable_value(page, candidate, tr.tr(candidate.title_msgid()));
                    }
                });
                ui.separator();
                if *page == Page::Shortcuts {
                    shortcut_page(ui, tr, draft);
                } else {
                    egui::ScrollArea::vertical()
                        .max_height((ui.available_height() - 48.0).max(80.0))
                        .show(ui, |ui| {
                            egui::Grid::new("gcodekit5-preferences-rows")
                                .num_columns(2)
                                .spacing([16.0, 8.0])
                                .show(ui, |ui| match *page {
                                    Page::General => general_rows(ui, tr, draft),
                                    Page::Machine => machine_rows(ui, tr, &mut draft.config),
                                    Page::UserInterface => {
                                        ui_rows(ui, tr, &mut draft.config.ui, title_bar_row);
                                    }
                                    Page::FileProcessing => {
                                        file_rows(ui, tr, &mut draft.config);
                                    }
                                    Page::Advanced => advanced_rows(ui, tr, &mut draft.config.ui),
                                    Page::Shortcuts => {}
                                });
                        });
                }
                if let Some(error) = &draft.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr.tr(n_!("Apply"))).clicked() {
                        action = Some(true);
                    }
                    if ui.button(tr.tr(n_!("Cancel"))).clicked() {
                        action = Some(false);
                    }
                });
            });
        match (open, action) {
            (_, Some(true)) => self.apply(settings),
            (false, _) | (true, Some(false)) => {
                self.draft = None;
                SettingsOutcome::Cancelled
            }
            (true, None) => SettingsOutcome::Open,
        }
    }

    /// Saves the draft; on success closes the window.
    fn apply(&mut self, settings: &mut SettingsManager) -> SettingsOutcome {
        let Some(draft) = self.draft.as_mut() else {
            return SettingsOutcome::Closed;
        };
        // Start from the settings as they are now, not as they were when the window opened:
        // recent files, probe and connection settings may have changed meanwhile.
        let mut config = settings.config().clone();
        copy_edited(&draft.config, &mut config, self.title_bar_row);
        // An empty field means no default directory.
        config.file_processing.output_directory = draft.directory.trim().into();
        config.ui.keyboard_shortcuts = shortcuts::saved(&draft.keymap);
        let restart = needs_restart(&self.launch_language, &config.ui.language)
            || (self.title_bar_row
                && config.ui.use_system_title_bar != self.launch_system_title_bar);
        match settings.apply(config) {
            Ok(()) => {
                if let Some(draft) = self.draft.take() {
                    self.applied_keymap = Some(draft.keymap);
                }
                SettingsOutcome::Applied { restart }
            }
            Err(error) => {
                let message = error.to_string();
                tracing::warn!(%message, "settings not saved");
                draft.error = Some(message.clone());
                SettingsOutcome::Failed(message)
            }
        }
    }
}

/// Copies the fields the window edits from `draft` into `config`, and nothing else.
fn copy_edited(draft: &Config, config: &mut Config, title_bar_row: bool) {
    let (from, to) = (&draft.ui, &mut config.ui);
    to.measurement_system = from.measurement_system;
    to.feed_rate_units = from.feed_rate_units;
    to.theme = from.theme;
    to.language.clone_from(&from.language);
    to.show_about_on_startup = from.show_about_on_startup;
    to.grid_major_line_width = from.grid_major_line_width;
    to.grid_minor_line_width = from.grid_minor_line_width;
    to.enable_stock_removal_3d = from.enable_stock_removal_3d;
    to.enable_stl_import = from.enable_stl_import;
    if title_bar_row {
        to.use_system_title_bar = from.use_system_title_bar;
    }
    let (from, to) = (&draft.machine, &mut config.machine);
    to.jog_increment = from.jog_increment;
    to.jog_feed_rate = from.jog_feed_rate;
    to.x_limit = from.x_limit;
    to.y_limit = from.y_limit;
    to.z_limit = from.z_limit;
    let (from, to) = (&draft.file_processing, &mut config.file_processing);
    to.preserve_comments = from.preserve_comments;
    to.arc_segment_length = from.arc_segment_length;
    to.max_line_length = from.max_line_length;
}

/// A row label; the widget after it is named by it.
fn row_label(ui: &mut egui::Ui, tr: &Translator, msgid: &str) -> egui::Id {
    ui.label(tr.tr(msgid)).id
}

/// A combo box row choosing one of `options` (value and shown text).
fn combo_row<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    tr: &Translator,
    msgid: &str,
    value: &mut T,
    options: &[(T, String)],
) {
    let label = row_label(ui, tr, msgid);
    let selected = options
        .iter()
        .find(|(option, _)| option == value)
        .map(|(_, text)| text.clone())
        .unwrap_or_default();
    ComboBox::from_id_salt(msgid)
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for (option, text) in options {
                ui.selectable_value(value, *option, text.as_str());
            }
        })
        .response
        .labelled_by(label);
    ui.end_row();
}

/// A number row in millimetres (or the unit `suffix` names), shown to 2 decimals.
fn number_row(
    ui: &mut egui::Ui,
    tr: &Translator,
    msgid: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    suffix: &str,
) {
    let label = row_label(ui, tr, msgid);
    ui.add(
        egui::DragValue::new(value)
            .range(range)
            .fixed_decimals(2)
            .speed(0.1)
            .suffix(suffix),
    )
    .labelled_by(label);
    ui.end_row();
}

/// A checkbox row; the checkbox carries the label.
fn check_row(ui: &mut egui::Ui, tr: &Translator, msgid: &str, value: &mut bool) {
    ui.label("");
    crate::theme::checkbox(ui, value, tr.tr(msgid));
    ui.end_row();
}

fn general_rows(ui: &mut egui::Ui, tr: &Translator, draft: &mut Draft) {
    let ui_settings = &mut draft.config.ui;
    combo_row(
        ui,
        tr,
        n_!("Measurement System"),
        &mut ui_settings.measurement_system,
        &[
            (MeasurementSystem::Metric, tr.tr(n_!("Metric")).into_owned()),
            (
                MeasurementSystem::Imperial,
                tr.tr(n_!("Imperial")).into_owned(),
            ),
        ],
    );
    let feed_units = [
        FeedRateUnits::MmPerMin,
        FeedRateUnits::MmPerSec,
        FeedRateUnits::InPerMin,
        FeedRateUnits::InPerSec,
    ]
    .map(|units| (units, units.to_string()));
    combo_row(
        ui,
        tr,
        n_!("Feed Rate Units"),
        &mut ui_settings.feed_rate_units,
        &feed_units,
    );
    let label = row_label(ui, tr, n_!("Default Directory"));
    ui.text_edit_singleline(&mut draft.directory)
        .labelled_by(label);
    ui.end_row();
}

fn machine_rows(ui: &mut egui::Ui, tr: &Translator, config: &mut Config) {
    let machine = &mut config.machine;
    number_row(
        ui,
        tr,
        n_!("Jog Step Size (Linear)"),
        &mut machine.jog_increment,
        0.01..=1000.0,
        " mm",
    );
    number_row(
        ui,
        tr,
        n_!("Jog Feed Rate"),
        &mut machine.jog_feed_rate,
        1.0..=100_000.0,
        " mm/min",
    );
    number_row(
        ui,
        tr,
        n_!("X Axis Limit"),
        &mut machine.x_limit,
        0.0..=100_000.0,
        " mm",
    );
    number_row(
        ui,
        tr,
        n_!("Y Axis Limit"),
        &mut machine.y_limit,
        0.0..=100_000.0,
        " mm",
    );
    number_row(
        ui,
        tr,
        n_!("Z Axis Limit"),
        &mut machine.z_limit,
        0.0..=100_000.0,
        " mm",
    );
}

fn ui_rows(ui: &mut egui::Ui, tr: &Translator, settings: &mut UiSettings, title_bar_row: bool) {
    combo_row(
        ui,
        tr,
        n_!("Theme"),
        &mut settings.theme,
        &[
            (Theme::System, tr.tr(n_!("System")).into_owned()),
            (Theme::Light, tr.tr(n_!("Light")).into_owned()),
            (Theme::Dark, tr.tr(n_!("Dark")).into_owned()),
        ],
    );
    let label = row_label(ui, tr, n_!("Language"));
    let current = normalized_language(&settings.language).to_string();
    let selected = LANGUAGES
        .iter()
        .find(|(code, _)| *code == current)
        .map_or_else(
            || tr.tr(n_!("System")).into_owned(),
            |(_, name)| (*name).into(),
        );
    ComboBox::from_id_salt("language")
        .selected_text(selected)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(current == "system", tr.tr(n_!("System")))
                .clicked()
            {
                settings.language = "system".into();
            }
            for (code, name) in LANGUAGES {
                if ui.selectable_label(current == code, name).clicked() {
                    settings.language = code.into();
                }
            }
        })
        .response
        .labelled_by(label);
    ui.end_row();
    check_row(
        ui,
        tr,
        n_!("Show About on Startup"),
        &mut settings.show_about_on_startup,
    );
    number_row(
        ui,
        tr,
        n_!("Grid Major Line Width (px)"),
        &mut settings.grid_major_line_width,
        0.5..=10.0,
        "",
    );
    number_row(
        ui,
        tr,
        n_!("Grid Minor Line Width (px)"),
        &mut settings.grid_minor_line_width,
        0.5..=10.0,
        "",
    );
    if title_bar_row {
        check_row(
            ui,
            tr,
            n_!("Use system title bar"),
            &mut settings.use_system_title_bar,
        );
    }
}

fn file_rows(ui: &mut egui::Ui, tr: &Translator, config: &mut Config) {
    let file = &mut config.file_processing;
    check_row(
        ui,
        tr,
        n_!("Preserve Comments"),
        &mut file.preserve_comments,
    );
    number_row(
        ui,
        tr,
        n_!("Arc Segment Length"),
        &mut file.arc_segment_length,
        0.01..=100.0,
        " mm",
    );
    let label = row_label(ui, tr, n_!("Max Line Length"));
    ui.add(egui::DragValue::new(&mut file.max_line_length).range(1..=100_000))
        .labelled_by(label);
    ui.end_row();
}

fn advanced_rows(ui: &mut egui::Ui, tr: &Translator, settings: &mut UiSettings) {
    check_row(
        ui,
        tr,
        n_!("3D Stock Removal Visualization"),
        &mut settings.enable_stock_removal_3d,
    );
    check_row(
        ui,
        tr,
        n_!("STL File Import"),
        &mut settings.enable_stl_import,
    );
}

/// Takes the frame's first key press as the key of the command being recorded, before any
/// widget sees it. Escape alone stops recording; a key the rules refuse leaves the command as
/// it was and says why.
fn record_key(ctx: &egui::Context, tr: &Translator, draft: &mut Draft) {
    let Some(command) = draft.recording else {
        return;
    };
    let platform: Platform = draft.keymap.platform();
    let event = ctx.input_mut(|input| {
        let press = |e: &egui::Event| {
            matches!(
                e,
                egui::Event::Key {
                    pressed: true,
                    repeat: false,
                    ..
                }
            )
        };
        let clipboard = |e: &egui::Event| {
            matches!(
                e,
                egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_)
            )
        };
        let found = input
            .events
            .iter()
            .position(press)
            .or_else(|| input.events.iter().position(clipboard))
            .map(|i| input.events[i].clone());
        // Nothing typed reaches a widget while a key is recorded.
        input
            .events
            .retain(|e| !(press(e) || clipboard(e) || matches!(e, egui::Event::Text(_))));
        found
    });
    let chord = match event {
        Some(egui::Event::Copy) => Some(craft_keymap::Chord::new(Mods::CMD, Key::C)),
        Some(egui::Event::Cut) => Some(craft_keymap::Chord::new(Mods::CMD, Key::X)),
        Some(egui::Event::Paste(_)) => Some(craft_keymap::Chord::new(Mods::CMD, Key::V)),
        Some(event) => {
            KeyInput::from_egui(&event, platform).and_then(|input| Layout::new().resolve(&input))
        }
        None => None,
    };
    let Some(chord) = chord else {
        return;
    };
    draft.recording = None;
    if chord.key == Key::Escape && chord.mods.is_none() {
        draft.shortcut_notice = None;
        return;
    }
    draft.shortcut_notice =
        match shortcuts::set_key(&mut draft.keymap, command, Trigger::single(chord)) {
            Ok(()) => None,
            Err(refusal) => Some(refusal.message(tr, &chord.display(platform))),
        };
}

/// Where a command's keys work, as the Shortcuts page names it.
fn where_msgid(keymap: &Keymap, command: Command) -> &'static str {
    match keymap
        .bindable(&command.command_ref())
        .map(|b| b.scope.as_str())
    {
        Some(commands::MACHINE_CONTROL_SCOPE) => n_!("Machine Control"),
        Some(commands::VISUALIZER_SCOPE) => n_!("Visualizer"),
        Some(commands::DESIGNER_SCOPE) => n_!("Designer"),
        _ => n_!("Everywhere"),
    }
}

/// The Shortcuts page: a filter, Reset All, the last refusal, and a row per command.
fn shortcut_page(ui: &mut egui::Ui, tr: &Translator, draft: &mut Draft) {
    ui.horizontal(|ui| {
        let label = row_label(ui, tr, n_!("Filter:"));
        ui.text_edit_singleline(&mut draft.filter)
            .labelled_by(label);
        if ui.button(tr.tr(n_!("Reset All"))).clicked() {
            shortcuts::reset_all(&mut draft.keymap);
            draft.recording = None;
            draft.shortcut_notice = None;
        }
    });
    if let Some(notice) = &draft.shortcut_notice {
        ui.colored_label(ui.visuals().error_fg_color, notice);
    }
    let query = draft.filter.trim().to_lowercase();
    let platform = draft.keymap.platform();
    egui::ScrollArea::vertical()
        .max_height((ui.available_height() - 48.0).max(80.0))
        .show(ui, |ui| {
            egui::Grid::new("gcodekit5-shortcut-rows")
                .num_columns(6)
                .striped(true)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    for msgid in [n_!("Command"), n_!("Where"), n_!("Shortcut")] {
                        ui.strong(tr.tr(msgid));
                    }
                    ui.end_row();
                    for command in Command::ALL {
                        let label = tr.tr(command.label_msgid());
                        if !query.is_empty() && !label.to_lowercase().contains(&query) {
                            continue;
                        }
                        shortcut_row(ui, tr, draft, command, &label, platform);
                    }
                });
        });
}

/// One command's row: its label, where its keys work, the keys, Change, Clear and Reset.
fn shortcut_row(
    ui: &mut egui::Ui,
    tr: &Translator,
    draft: &mut Draft,
    command: Command,
    label: &str,
    platform: Platform,
) {
    let command_ref = command.command_ref();
    let bindings = draft.keymap.bindings(&command_ref);
    let recording = draft.recording == Some(command);
    let locked = LOCKED.contains(&command);
    ui.label(label);
    ui.label(tr.tr(where_msgid(&draft.keymap, command)));
    let keys = if recording {
        tr.tr(n_!("Press a key (Escape cancels)")).into_owned()
    } else if bindings.is_empty() {
        tr.tr(n_!("None")).into_owned()
    } else {
        bindings
            .iter()
            .map(|b| b.trigger.display(platform))
            .collect::<Vec<_>>()
            .join(", ")
    };
    ui.label(keys);
    let fixed = || shortcuts::Refusal::Locked.message(tr, "");
    let change = ui.add_enabled(
        !locked,
        egui::Button::new(tr.tr(n_!("Change"))).selected(recording),
    );
    if change.clicked() {
        draft.recording = (!recording).then_some(command);
        draft.shortcut_notice = None;
    }
    change.on_disabled_hover_text(fixed());
    let clear = ui.add_enabled(
        !locked && !bindings.is_empty(),
        egui::Button::new(tr.tr(n_!("Clear"))),
    );
    if clear.clicked() {
        if let Err(refusal) = shortcuts::clear(&mut draft.keymap, command) {
            draft.shortcut_notice = Some(refusal.message(tr, ""));
        }
        draft.recording = None;
    }
    let changed = bindings != draft.keymap.default_bindings(&command_ref);
    if ui
        .add_enabled(!locked && changed, egui::Button::new(tr.tr(n_!("Reset"))))
        .clicked()
    {
        draft.recording = None;
        draft.shortcut_notice = shortcuts::reset(&mut draft.keymap, command)
            .err()
            .map(|(trigger, refusal)| refusal.message(tr, &trigger.display(platform)));
    }
    ui.end_row();
}

/// The English msgids this window shows, for the translation test.
pub fn msgids() -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = Page::ALL.into_iter().map(Page::title_msgid).collect();
    ids.extend([
        n_!("Preferences"),
        n_!("Apply"),
        n_!("Cancel"),
        n_!("Measurement System"),
        n_!("Metric"),
        n_!("Imperial"),
        n_!("Feed Rate Units"),
        n_!("Default Directory"),
        n_!("Jog Step Size (Linear)"),
        n_!("Jog Feed Rate"),
        n_!("X Axis Limit"),
        n_!("Y Axis Limit"),
        n_!("Z Axis Limit"),
        n_!("Theme"),
        n_!("System"),
        n_!("Light"),
        n_!("Dark"),
        n_!("Language"),
        n_!("Show About on Startup"),
        n_!("Grid Major Line Width (px)"),
        n_!("Grid Minor Line Width (px)"),
        n_!("Use system title bar"),
        n_!("Preserve Comments"),
        n_!("Arc Segment Length"),
        n_!("Max Line Length"),
        n_!("3D Stock Removal Visualization"),
        n_!("STL File Import"),
        n_!("Filter:"),
        n_!("Reset All"),
        n_!("Command"),
        n_!("Where"),
        n_!("Shortcut"),
        n_!("Everywhere"),
        n_!("Machine Control"),
        n_!("Visualizer"),
        n_!("Designer"),
        n_!("Press a key (Escape cancels)"),
        n_!("None"),
        n_!("Change"),
        n_!("Clear"),
        n_!("Reset"),
    ]);
    ids.extend(shortcuts::msgids());
    ids
}
