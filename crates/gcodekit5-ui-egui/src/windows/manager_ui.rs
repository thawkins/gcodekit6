//! Shared pieces of the manager pages (Device Manager, CNC Tools, Materials), drawn
//! as: a sidebar header with the page title and help button, a boxed list of
//! selectable rows, a centred tab switcher, framed form groups, the "select an entry"
//! placeholder and the in-page question card, plus the pages' shared notice state
//! ([`LibraryNotice`]).
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations).

use std::path::PathBuf;

use gcodekit5_app::i18n::Translator;

use crate::icons::Icon;
use crate::theme;
use crate::windows::help::{HelpTopic, help_button};

/// Width of the manager pages' left sidebar (250 to 280 px).
pub(crate) const SIDEBAR_WIDTH: f32 = 270.0;

/// The sidebar's header: the page title (Adwaita `title-4`) with the help button at the
/// right end.
///
/// # Arguments
/// * `ui` - The sidebar's `Ui`.
/// * `title` - The title, already translated.
/// * `topic` - The help topic the button opens.
/// * `tr` - Translates the help button's tooltip.
pub(crate) fn sidebar_header(ui: &mut egui::Ui, title: &str, topic: HelpTopic, tr: &Translator) {
    ui.horizontal(|ui| {
        theme::title(ui, title, theme::TITLE_4);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            help_button(ui, topic, tr);
        });
    });
}

/// A boxed list (libadwaita's `boxed-list`): a rounded, bordered card whose rows sit flush, one
/// under the other.
///
/// # Arguments
/// * `ui` - Where the list goes.
/// * `rows` - Draws the rows with [`list_row`].
pub(crate) fn boxed_list(ui: &mut egui::Ui, rows: impl FnOnce(&mut egui::Ui)) {
    theme::group_frame(ui)
        .inner_margin(egui::Margin::same(0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            rows(ui);
        });
}

/// One row of a [`boxed_list`]: `name` in Adwaita's `title-4` weight, then what `details`
/// draws (the dim summary line and badges). A line separates it from the row above unless
/// it is the `first`. The whole row is clickable; `name` is its accessible name, so a
/// screen reader (or a test) finds the row by it.
///
/// # Arguments
/// * `ui` - The list's `Ui`.
/// * `first` - Whether it is the list's first row.
/// * `selected` - Whether to draw it selected.
/// * `name` - The entry's name.
/// * `details` - Draws the lines under the name.
///
/// # Returns
/// The row's response (clicked when the row was clicked).
pub(crate) fn list_row(
    ui: &mut egui::Ui,
    first: bool,
    selected: bool,
    name: &str,
    details: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    if !first {
        let y = ui.cursor().top();
        ui.painter().hline(
            ui.max_rect().x_range(),
            y,
            ui.visuals().widgets.noninteractive.bg_stroke,
        );
    }
    let background = ui.painter().add(egui::Shape::Noop);
    let inner = egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Labels in the row must not take its clicks.
            ui.style_mut().interaction.selectable_labels = false;
            let color = if selected {
                ui.visuals().selection.stroke.color
            } else {
                ui.visuals().strong_text_color()
            };
            let font = egui::FontId::proportional(theme::TITLE_4);
            let wrap = ui.available_width();
            let galley = ui.painter().layout(name.to_owned(), font, color, wrap);
            let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
            // The same faux bold as `theme::title`.
            ui.painter().galley(rect.min, galley.clone(), color);
            ui.painter()
                .galley(rect.min + egui::vec2(0.6, 0.0), galley, color);
            ui.add_space(2.0);
            if selected {
                ui.visuals_mut().override_text_color = Some(color);
            }
            details(ui);
        });
    let rect = inner.response.rect;
    // The auto id advances with every row, so rows with the same name stay apart.
    let id = ui.next_auto_id().with("gcodekit5-list-row");
    let response = ui.interact(rect, id, egui::Sense::click());
    let enabled = ui.is_enabled();
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, selected, name));
    let fill = if selected {
        ui.visuals().selection.bg_fill
    } else if response.hovered() {
        ui.visuals().widgets.hovered.weak_bg_fill
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter()
        .set(background, egui::Shape::rect_filled(rect, 0.0, fill));
    response
}

/// A stack switcher: the form's tabs as a row of linked toggle buttons, centred.
///
/// # Arguments
/// * `ui` - Where the switcher goes.
/// * `current` - The open tab; a click changes it.
/// * `tabs` - Each tab and its label, already translated.
pub(crate) fn tab_switcher<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    current: &mut T,
    tabs: &[(T, String)],
) {
    let padding = ui.spacing().button_padding.x * 2.0;
    let gap = 2.0;
    let font = egui::TextStyle::Button.resolve(ui.style());
    let width: f32 = tabs
        .iter()
        .map(|(_, label)| {
            let text =
                ui.painter()
                    .layout_no_wrap(label.clone(), font.clone(), egui::Color32::WHITE);
            text.size().x + padding + gap
        })
        .sum();
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
        ui.spacing_mut().item_spacing.x = gap;
        for (tab, label) in tabs {
            ui.selectable_value(current, *tab, label.as_str());
        }
    });
}

/// A framed group of form rows (like libadwaita's `PreferencesGroup` cards) with an
/// optional bold heading above it.
///
/// # Arguments
/// * `ui` - Where the group goes.
/// * `heading` - The heading, already translated; `None` for none.
/// * `add` - Draws the group's contents.
pub(crate) fn form_group<R>(
    ui: &mut egui::Ui,
    heading: Option<&str>,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    if let Some(heading) = heading {
        theme::section_heading(ui, heading);
    }
    let inner = theme::group_frame(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    });
    ui.add_space(8.0);
    inner.inner
}

/// The right panel's text while nothing is selected (a dim `title-2` placeholder),
/// centred in the room left.
///
/// # Arguments
/// * `ui` - The panel's `Ui`.
/// * `text` - The text, already translated.
pub(crate) fn placeholder(ui: &mut egui::Ui, text: &str) {
    ui.centered_and_justified(|ui| {
        ui.label(
            egui::RichText::new(text)
                .size(theme::TITLE_2)
                .color(ui.visuals().weak_text_color()),
        );
    });
}

/// A manager page's notice and the library-file state it reports: why the file could not
/// be read (until the page shows it) and the backup of an unreadable file it last named.
#[derive(Debug, Default)]
pub(crate) struct LibraryNotice {
    /// Why the file could not be read, until the page shows it.
    load_error: Option<String>,
    /// The backup of an unreadable file the notice last named.
    backup_named: Option<PathBuf>,
    /// The latest notice and whether it reports a failure.
    notice: Option<(String, bool)>,
}

impl LibraryNotice {
    /// No notice yet, holding `load_error` until [`Self::show_load_error`].
    ///
    /// # Arguments
    /// * `load_error` - Why the library file could not be read, if it could not.
    pub(crate) fn new(load_error: Option<String>) -> Self {
        Self {
            load_error,
            ..Self::default()
        }
    }

    /// Replaces the notice.
    ///
    /// # Arguments
    /// * `text` - The notice, already translated.
    /// * `error` - Whether it reports a failure.
    pub(crate) fn say(&mut self, text: String, error: bool) {
        self.notice = Some((text, error));
    }

    /// Removes the notice.
    pub(crate) fn clear(&mut self) {
        self.notice = None;
    }

    /// The notice's text, if any.
    pub(crate) fn text(&self) -> Option<&str> {
        self.notice.as_ref().map(|(text, _)| text.as_str())
    }

    /// The notice and whether it is an error, as [`notice`] draws it.
    pub(crate) fn get(&self) -> Option<&(String, bool)> {
        self.notice.as_ref()
    }

    /// Shows the load error once, as an error notice `"{title} {error}"`.
    ///
    /// # Arguments
    /// * `title` - The sentence before the error, already translated.
    pub(crate) fn show_load_error(&mut self, title: &str) {
        if let Some(error) = self.load_error.take() {
            self.say(format!("{title} {error}"), true);
        }
    }

    /// Adds the path of a backup the library file got since the last notice to the notice,
    /// turning it into an error notice.
    ///
    /// # Arguments
    /// * `backup` - The file's latest backup, if it has one.
    /// * `kept_in` - The sentence before the path, already translated.
    pub(crate) fn name_backup(&mut self, backup: Option<PathBuf>, kept_in: &str) {
        let Some(backup) = backup else {
            return;
        };
        if self.backup_named.as_ref() == Some(&backup) {
            return;
        }
        let note = format!("{kept_in} {}", backup.display());
        self.notice = Some(match self.notice.take() {
            Some((text, _)) => (format!("{text} {note}"), true),
            None => (note, true),
        });
        self.backup_named = Some(backup);
    }
}

/// The page's notice: an error in the error colour, else plain.
///
/// # Arguments
/// * `ui` - Where the notice goes.
/// * `notice` - The text and whether it is an error.
pub(crate) fn notice(ui: &mut egui::Ui, notice: Option<&(String, bool)>) {
    match notice {
        Some((text, true)) => {
            ui.colored_label(ui.visuals().error_fg_color, text);
        }
        Some((text, false)) => {
            ui.label(text);
        }
        None => {}
    }
}

/// A question asked in the page, not in a dialog: a framed card with a bold title,
/// the text and one button per choice.
///
/// # Arguments
/// * `ui` - Where the card goes.
/// * `title` - The question's title, already translated.
/// * `text` - The question, already translated.
/// * `choices` - The answers' labels, already translated, first the one that acts.
///
/// # Returns
/// The index of the answer clicked this frame, if any.
pub(crate) fn question(
    ui: &mut egui::Ui,
    title: &str,
    text: &str,
    choices: &[String],
) -> Option<usize> {
    let mut answer = None;
    theme::group_frame(ui).show(ui, |ui| {
        theme::section_heading(ui, title);
        ui.label(text);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for (index, choice) in choices.iter().enumerate() {
                if ui.button(choice).clicked() {
                    answer = Some(index);
                }
            }
        });
    });
    ui.add_space(8.0);
    answer
}

/// A button with an icon before its label; the label is its accessible name.
///
/// # Arguments
/// * `ui` - The ui the button goes into; its theme decides the icon's tint.
/// * `icon` - The icon.
/// * `text` - The label, already translated.
pub(crate) fn icon_text_button(
    ui: &egui::Ui,
    icon: Icon,
    text: impl Into<egui::WidgetText>,
) -> egui::Button<'static> {
    egui::Button::image_and_text(icon.themed(ui), text)
}
