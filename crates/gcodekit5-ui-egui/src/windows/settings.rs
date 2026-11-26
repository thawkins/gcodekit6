//! The Preferences window: a copy of the settings edited page by page, saved by Apply and
//! dropped by Cancel.
//!
//! Ported from the GTK crate's `ui/gtk/settings.rs` and the rows
//! `gcodekit5_settings::persistence` fills it with. The rows are the GTK pages' rows that a
//! [`Config`] field backs and that mean something to this front end: General (units, feed rate
//! units, default folder), Machine (jog step, jog feed rate, X/Y/Z travel), User Interface
//! (theme, language, About on startup, grid line widths, and "Use system title bar" where
//! craft-window says it changes something), File Processing and Advanced. A language or
//! title-bar change takes effect after a restart; [`SettingsOutcome::Applied`] says when one
//! is needed.
//!
//! Dependencies: `egui`, `craft-ui` (combo boxes), `gcodekit5-settings` (the settings and
//! saving them), `gcodekit5-app` (translations).

use craft_ui::combo::ComboBox;
use gcodekit5_app::i18n::{Translator, needs_restart, normalized_language};
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::{Config, FeedRateUnits, MeasurementSystem, Theme, UiSettings};

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
    Advanced,
}

impl Page {
    const ALL: [Self; 5] = [
        Self::General,
        Self::Machine,
        Self::UserInterface,
        Self::FileProcessing,
        Self::Advanced,
    ];

    fn title_msgid(self) -> &'static str {
        match self {
            Self::General => n_!("General"),
            Self::Machine => n_!("Machine"),
            Self::UserInterface => n_!("User Interface"),
            Self::FileProcessing => n_!("File Processing"),
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
        }
    }

    /// Opens the window on a copy of the current settings; does nothing when it is open.
    ///
    /// # Arguments
    /// * `settings` - The settings to edit.
    pub fn open(&mut self, settings: &SettingsManager) {
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
            });
        }
    }

    /// Whether the window is open.
    pub fn is_open(&self) -> bool {
        self.draft.is_some()
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
                            });
                    });
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
        let restart = needs_restart(&self.launch_language, &config.ui.language)
            || (self.title_bar_row
                && config.ui.use_system_title_bar != self.launch_system_title_bar);
        match settings.apply(config) {
            Ok(()) => {
                self.draft = None;
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
    ]);
    ids
}
