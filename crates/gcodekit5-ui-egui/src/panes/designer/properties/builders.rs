//! The panel's widgets: a titled section with a label / field / unit grid, text fields that
//! apply on commit, checkboxes and drop-downs, and the length formatting.
//!
//! A field applies when the user commits it, as GTK's `connect_entry_commit`: Enter, or
//! moving the focus away. Typing changes nothing in the design, so one edit is one undo
//! step. The panel commits a finished [`Edit`] to the shapes that were selected when the
//! editing began, even when the selection changed since (a click on the canvas both ends
//! the edit and changes the selection). Text that does not parse, or a value out of range,
//! applies nothing and shows an inline error under the field.
//!
//! Lengths are shown in the display units with 2 decimals (the toolbox's `to_display`
//! conversion) and parsed with `gcodekit5_core::units::parse_length`, which also takes
//! inch fractions ("1 1/2").
//!
//! Dependencies: `egui`, `gcodekit5-core` (units).

use gcodekit5_core::units::{self, MeasurementSystem};
use gcodekit5_designer::designer_state::MachineMode;

use crate::panes::designer::toolbox::settings::{Kind, to_display};

/// The text shown for a value the selected shapes do not share.
pub const MIXED: &str = "---";

/// Inline error: the text is not a number.
pub const E_NUMBER: &str = n_!("Enter a number");
/// Inline error: the value must be above 0.
pub const E_POSITIVE: &str = n_!("Enter a value greater than 0");
/// Inline error: the value must be 0 or more.
pub const E_NON_NEGATIVE: &str = n_!("Enter a value of 0 or more");
/// Inline error: an angle outside 0..=90 degrees.
pub const E_ANGLE: &str = n_!("Enter an angle from 0 to 90");
/// Inline error: a count below 1.
pub const E_COUNT: &str = n_!("Enter a whole number of 1 or more");
/// Inline error: any other value out of its range.
pub const E_RANGE: &str = n_!("Value out of range");
/// Inline error: blank text.
pub const E_TEXT: &str = n_!("Text cannot be empty");

/// The msgids of the inline errors.
pub fn msgids() -> Vec<&'static str> {
    vec![
        E_NUMBER,
        E_POSITIVE,
        E_NON_NEGATIVE,
        E_ANGLE,
        E_COUNT,
        E_RANGE,
        E_TEXT,
    ]
}

/// The result of applying a committed field: `Err` holds the inline error's msgid.
pub type Check = Result<(), &'static str>;

/// A field being edited: which one, what it showed when editing began, what is typed now,
/// the shapes it edits (the selection when editing began), and its widget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    key: &'static str,
    baseline: String,
    text: String,
    selection: Vec<u64>,
    widget: egui::Id,
    /// Whether its field was drawn since the panel last looked (a field that is no longer
    /// drawn, because the selection changed, cannot keep the keyboard).
    seen: bool,
}

impl Edit {
    /// The field being edited.
    pub fn key(&self) -> &'static str {
        self.key
    }

    /// The text typed so far.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The shapes the edit applies to (the selection when editing began).
    pub fn selection(&self) -> &[u64] {
        &self.selection
    }

    /// The text field's widget id (the edit is finished when it loses the focus).
    pub fn widget(&self) -> egui::Id {
        self.widget
    }

    /// Whether the field was drawn since the panel last looked.
    pub fn seen(&self) -> bool {
        self.seen
    }

    /// Whether the field was drawn since the last call, which resets it.
    pub fn take_seen(&mut self) -> bool {
        std::mem::take(&mut self.seen)
    }

    /// Whether the text differs from what the field showed when editing began.
    pub fn changed(&self) -> bool {
        self.text != self.baseline
    }
}

/// What happened to a field this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldEvent {
    /// Nothing new.
    None,
    /// The user typed; the text so far (for a live preview).
    Typed(String),
}

/// The fields of one frame: the edits in progress, the selection now, and the inline
/// errors.
///
/// Moving the focus from one field to the next starts the new edit a frame before the old
/// one commits, so two can be in progress at once.
pub struct Fields<'a> {
    /// The fields being edited.
    pub edits: &'a mut Vec<Edit>,
    /// The selected shapes now.
    pub selection: &'a [u64],
    /// The inline errors: field key and msgid.
    pub errors: &'a [(&'static str, &'static str)],
    /// Translator for the errors.
    pub tr: &'a gcodekit5_app::i18n::Translator,
}

/// A section: a framed group with a bold title over a three-column grid (label, field,
/// unit).
///
/// # Arguments
/// * `ui` - Where to show it.
/// * `title` - The translated title.
/// * `contents` - The rows.
///
/// # Returns
/// The section's response (for a tooltip).
pub fn section(
    ui: &mut egui::Ui,
    title: &str,
    contents: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    egui::Frame::group(ui.style())
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.strong(title);
            egui::Grid::new(("gcodekit5-properties", title))
                .num_columns(3)
                .spacing([8.0, 6.0])
                .show(ui, contents);
        })
        .response
}

/// A labelled text field row that applies on commit, with its inline error below.
///
/// # Arguments
/// * `ui` - The section's grid.
/// * `fields` - The edits in progress and the selection.
/// * `key` - The field's unique key.
/// * `label` - The translated label (also the field's accessible name).
/// * `shown` - The value shown while the field is not being edited.
/// * `unit` - The unit after the field ("" for none).
/// * `enabled` - Whether the field can be edited.
#[allow(clippy::too_many_arguments)] // one grid row: label, field, unit and their states
pub fn entry(
    ui: &mut egui::Ui,
    fields: &mut Fields<'_>,
    key: &'static str,
    label: &str,
    shown: &str,
    unit: &str,
    enabled: bool,
) -> FieldEvent {
    let label = ui.add_enabled(enabled, egui::Label::new(label));
    let event = text_field(ui, fields, key, shown, enabled, 110.0, label.id);
    ui.add_enabled(enabled, egui::Label::new(unit));
    ui.end_row();
    if let Some((_, msgid)) = fields.errors.iter().find(|(k, _)| *k == key) {
        let color = crate::theme::palette(ui.ctx().theme()).error;
        ui.label("");
        ui.colored_label(color, fields.tr.tr(msgid));
        ui.end_row();
    }
    event
}

/// A text field that records an [`Edit`] while focused (the panel commits it), named by
/// the widget `label_id`.
///
/// # Arguments
/// * `ui` - Where to show it.
/// * `fields` - The edits in progress and the selection.
/// * `key` - The field's unique key.
/// * `shown` - The value shown while the field is not being edited.
/// * `enabled` - Whether the field can be edited.
/// * `width` - The field's width, points.
/// * `label_id` - The label naming the field.
pub fn text_field(
    ui: &mut egui::Ui,
    fields: &mut Fields<'_>,
    key: &'static str,
    shown: &str,
    enabled: bool,
    width: f32,
    label_id: egui::Id,
) -> FieldEvent {
    let editing = fields.edits.iter().find(|e| e.key == key).cloned();
    let mut buffer = editing
        .as_ref()
        .map_or_else(|| shown.to_string(), |e| e.text.clone());
    let response = ui
        .add_enabled(
            enabled,
            egui::TextEdit::singleline(&mut buffer)
                .id_salt(("gcodekit5-property", key))
                .desired_width(width),
        )
        .labelled_by(label_id);
    if response.has_focus() && editing.is_none() {
        fields.edits.push(Edit {
            key,
            baseline: shown.to_string(),
            text: buffer.clone(),
            selection: fields.selection.to_vec(),
            widget: response.id,
            seen: true,
        });
    }
    if let Some(edit) = fields.edits.iter_mut().find(|e| e.key == key) {
        edit.seen = true;
    }
    let mut event = FieldEvent::None;
    if response.changed()
        && let Some(edit) = fields.edits.iter_mut().find(|e| e.key == key)
    {
        edit.text.clone_from(&buffer);
        event = FieldEvent::Typed(buffer);
    }
    event
}

/// A labelled checkbox row; returns the new value when the user toggled it.
///
/// # Arguments
/// * `ui` - The section's grid.
/// * `label` - The translated label.
/// * `value` - The value shown.
/// * `enabled` - Whether it can be toggled.
pub fn check(ui: &mut egui::Ui, label: &str, value: bool, enabled: bool) -> Option<bool> {
    let mut v = value;
    let changed = ui
        .add_enabled(enabled, egui::Checkbox::new(&mut v, label))
        .changed();
    ui.end_row();
    changed.then_some(v)
}

/// A labelled drop-down row; returns the index the user picked when it differs.
///
/// # Arguments
/// * `ui` - The section's grid.
/// * `label` - The translated label.
/// * `options` - The translated option labels.
/// * `selected` - The index shown.
/// * `enabled` - Whether it can be changed.
pub fn combo(
    ui: &mut egui::Ui,
    label: &str,
    options: &[String],
    selected: usize,
    enabled: bool,
) -> Option<usize> {
    let label_id = ui.add_enabled(enabled, egui::Label::new(label)).id;
    let mut picked = None;
    ui.add_enabled_ui(enabled, |ui| {
        egui::ComboBox::from_id_salt(("gcodekit5-property-combo", label))
            .selected_text(options.get(selected).map_or("", String::as_str))
            .show_ui(ui, |ui| {
                for (i, option) in options.iter().enumerate() {
                    if ui.selectable_label(i == selected, option).clicked() && i != selected {
                        picked = Some(i);
                    }
                }
            })
            .response
            .labelled_by(label_id);
    });
    ui.end_row();
    picked
}

/// `mm` in the display units with 2 decimals.
///
/// # Arguments
/// * `mm` - A length, millimetres.
/// * `system` - The display units.
pub fn length_text(mm: f64, system: MeasurementSystem) -> String {
    format!(
        "{:.2}",
        to_display(Kind::Length, MachineMode::Cnc3D, system, mm).0
    )
}

/// The unit suffix of lengths ("mm" or "in").
pub fn length_unit(system: MeasurementSystem) -> &'static str {
    units::get_unit_label(system)
}

/// The millimetres of a length typed in the display units.
///
/// # Arguments
/// * `text` - The typed length (a comma is a decimal point; inch fractions allowed).
/// * `system` - The display units.
///
/// # Errors
/// [`E_NUMBER`] for empty or invalid text.
pub fn length(text: &str, system: MeasurementSystem) -> Result<f64, &'static str> {
    let text = text.trim().replace(',', ".");
    if text.is_empty() {
        return Err(E_NUMBER);
    }
    units::parse_length(&text, system)
        .ok()
        .map(f64::from)
        .filter(|v| v.is_finite())
        .ok_or(E_NUMBER)
}

/// A typed number (a comma is a decimal point).
///
/// # Errors
/// [`E_NUMBER`] for invalid text.
pub fn number(text: &str) -> Result<f64, &'static str> {
    text.trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or(E_NUMBER)
}

/// `v` when it is above 0.
///
/// # Errors
/// [`E_POSITIVE`] otherwise.
pub fn positive(v: f64) -> Result<f64, &'static str> {
    if v > 0.0 { Ok(v) } else { Err(E_POSITIVE) }
}

/// `v` when it is 0 or more.
///
/// # Errors
/// [`E_NON_NEGATIVE`] otherwise.
pub fn non_negative(v: f64) -> Result<f64, &'static str> {
    if v >= 0.0 { Ok(v) } else { Err(E_NON_NEGATIVE) }
}

/// A typed whole number of at least `min`.
///
/// # Errors
/// [`E_NUMBER`] for text that is not a whole number, `below` when it is under `min`.
pub fn whole(text: &str, min: usize, below: &'static str) -> Result<usize, &'static str> {
    let n = text.trim().parse::<usize>().map_err(|_| E_NUMBER)?;
    if n >= min { Ok(n) } else { Err(below) }
}
