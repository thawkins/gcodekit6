//! The panel's widgets: a titled section with a label / field / unit grid, text fields that
//! apply on commit, checkboxes and drop-downs, and the length formatting.
//!
//! A field applies when the user commits it, as GTK's `connect_entry_commit`: Enter, or
//! moving the focus away. Typing changes nothing in the design, so one edit is one undo
//! step. A commit whose text is unchanged, or that comes after the selection changed while
//! the field was being edited, applies nothing (GTK discards it too).
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

/// A field being edited: which one, what it showed when editing began, what is typed now,
/// and the selection then.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    key: &'static str,
    baseline: String,
    text: String,
    selection: Vec<u64>,
}

impl Edit {
    /// The field being edited.
    pub fn key(&self) -> &'static str {
        self.key
    }
}

/// What happened to a field this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldEvent {
    /// Nothing to apply.
    None,
    /// The user typed; the text so far (for a live preview).
    Typed(String),
    /// The user committed this changed text.
    Committed(String),
}

/// The fields of one frame: the edits in progress and the selection now.
///
/// Moving the focus from one field to the next starts the new edit a frame before the old
/// one commits, so two can be in progress at once.
pub struct Fields<'a> {
    /// The fields being edited.
    pub edits: &'a mut Vec<Edit>,
    /// The selected shapes now.
    pub selection: &'a [u64],
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

/// A labelled text field row that applies on commit.
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
    event
}

/// A text field that applies on commit, named by the widget `label_id`.
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
        });
    } else if !response.has_focus() && !response.lost_focus() && editing.is_some() {
        // A stale edit (its field was hidden while focused): show the value again.
        fields.edits.retain(|e| e.key != key);
    }
    let mut event = FieldEvent::None;
    if response.changed()
        && let Some(edit) = fields.edits.iter_mut().find(|e| e.key == key)
    {
        edit.text.clone_from(&buffer);
        event = FieldEvent::Typed(buffer.clone());
    }
    if response.lost_focus()
        && let Some(i) = fields.edits.iter().position(|e| e.key == key)
    {
        let edit = fields.edits.remove(i);
        if edit.selection != fields.selection {
            tracing::debug!(key, "property edit discarded: the selection changed");
        } else if buffer != edit.baseline {
            event = FieldEvent::Committed(buffer);
        }
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

/// The millimetres of a length typed in the display units; `None` for empty or invalid
/// text.
///
/// # Arguments
/// * `text` - The typed length.
/// * `system` - The display units.
pub fn parse_length(text: &str, system: MeasurementSystem) -> Option<f64> {
    let text = text.trim().replace(',', ".");
    if text.is_empty() {
        return None;
    }
    units::parse_length(&text, system)
        .ok()
        .map(f64::from)
        .filter(|v| v.is_finite())
}

/// A typed number (a comma is a decimal point); `None` for invalid text.
pub fn parse_number(text: &str) -> Option<f64> {
    text.trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}
