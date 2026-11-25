//! Widgets and validation shared by the CAM tool pages: labelled length and number fields
//! with their inline errors, the action buttons, and unit conversion of a field's text.
//!
//! Lengths are typed in the display units (`gcodekit5_core::units`, the same parse and
//! format the GTK pages used) and held as text, so a value that does not parse stays on
//! screen with its error instead of being replaced. Feeds stay in mm/min and speeds in RPM,
//! as on the GTK pages.
//!
//! Dependencies: `egui`, `gcodekit5-core` (units), `gcodekit5-app` (translations).

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::{self, MeasurementSystem};

/// What a page asks the pane to do after drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CamAction {
    /// Nothing.
    None,
    /// Generate the program (or, for the speeds and feeds calculator, calculate) and show it
    /// on the page.
    Generate,
    /// Generate the program and replace the editor's program with it.
    SendToEditor,
    /// Generate the program, hand it to the editor and show it in the visualizer (what the
    /// GTK pages' Generate did).
    Preview,
    /// Load the page's input from a preset file (GTK's Load).
    LoadPreset,
    /// Save the page's input to a preset file (GTK's Save).
    SavePreset,
}

/// Why a page produced no program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CamError {
    /// The input has problems (shown at the fields).
    Invalid,
    /// The generator refused the parameters.
    Generator(String),
}

impl CamError {
    /// The generator's refusal `error`.
    ///
    /// # Arguments
    /// * `error` - The generator's error.
    pub fn generator(error: anyhow::Error) -> Self {
        Self::Generator(format!("{error:#}"))
    }

    /// The message shown on the page.
    ///
    /// # Arguments
    /// * `tr` - Translator for the current UI language.
    pub fn message(&self, tr: &Translator) -> String {
        match self {
            Self::Invalid => tr.tr(n_!("Invalid Parameters")).into_owned(),
            Self::Generator(error) => format!("{}: {error}", tr.tr(n_!("Generation failed"))),
        }
    }
}

/// A problem with the page's input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldError {
    /// The msgid of the field's label, or `None` for a problem with the whole page.
    pub field: Option<&'static str>,
    /// The msgid of the message.
    pub message: &'static str,
}

impl FieldError {
    /// A problem with the field labelled `field`.
    ///
    /// # Arguments
    /// * `field` - The msgid of the field's label.
    /// * `message` - The msgid of the message.
    pub fn field(field: &'static str, message: &'static str) -> Self {
        Self {
            field: Some(field),
            message,
        }
    }

    /// A problem with the whole page.
    ///
    /// # Arguments
    /// * `message` - The msgid of the message.
    pub fn page(message: &'static str) -> Self {
        Self {
            field: None,
            message,
        }
    }
}

/// Message: the text is not a length.
pub const NOT_A_LENGTH: &str = n_!("Not a valid length");
/// Message: the text is not a number.
pub const NOT_A_NUMBER: &str = n_!("Not a number");
/// Message: the value must be above zero.
pub const MUST_BE_POSITIVE: &str = n_!("Must be greater than 0");
/// Message: the value must not be below zero.
pub const MUST_NOT_BE_NEGATIVE: &str = n_!("Must not be negative");
/// Message: the value is outside the field's range.
pub const OUT_OF_RANGE: &str = n_!("Out of range");
/// Message: the tool needs a Z axis.
pub const NEEDS_THREE_AXES: &str =
    n_!("This tool needs at least 3 axes (X, Y, Z); the active device has fewer.");

/// The msgids the shared widgets show, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        NOT_A_LENGTH,
        NOT_A_NUMBER,
        MUST_BE_POSITIVE,
        MUST_NOT_BE_NEGATIVE,
        OUT_OF_RANGE,
        NEEDS_THREE_AXES,
        n_!("Generate"),
        n_!("Send to Editor"),
        n_!("Preview"),
        n_!("Home Device Before Start"),
        n_!("Invalid Parameters"),
        n_!("Generation failed"),
        n_!("Load"),
        n_!("Save"),
    ]
}

/// Reads a length typed in `units` as millimetres.
///
/// # Arguments
/// * `text` - The field's text.
/// * `units` - The display units.
///
/// # Errors
/// [`NOT_A_LENGTH`] when the text is not a finite length.
pub fn parse_len(text: &str, units: MeasurementSystem) -> Result<f64, &'static str> {
    units::parse_length(text, units)
        .ok()
        .filter(|v| v.is_finite())
        .map(f64::from)
        .ok_or(NOT_A_LENGTH)
}

/// Reads a plain number (a feed in mm/min, a speed in RPM, a percentage).
///
/// # Arguments
/// * `text` - The field's text.
///
/// # Errors
/// [`NOT_A_NUMBER`] when the text is not a finite number.
pub fn parse_number(text: &str) -> Result<f64, &'static str> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or(NOT_A_NUMBER)
}

/// The text of `mm` in `units`, as a length field shows it.
///
/// # Arguments
/// * `mm` - The length in millimetres.
/// * `units` - The display units.
pub fn format_len(mm: f64, units: MeasurementSystem) -> String {
    units::format_length(mm as f32, units)
}

/// Rewrites a length field's text from `from` units to `to` units; text that is not a
/// length is left as it is (as the GTK pages did).
///
/// # Arguments
/// * `text` - The field's text.
/// * `from` - The units the text is in.
/// * `to` - The new display units.
pub fn convert_len(text: &mut String, from: MeasurementSystem, to: MeasurementSystem) {
    if from == to {
        return;
    }
    if let Ok(mm) = units::parse_length(text, from) {
        *text = units::format_length(mm, to);
    }
}

/// Checks `value` with `rule`, recording a problem with the field labelled `field`.
///
/// # Arguments
/// * `errors` - Where problems are recorded.
/// * `field` - The msgid of the field's label.
/// * `value` - The parsed value, or the parse error.
/// * `rule` - The value's rule: `None` when it holds, else the message msgid.
///
/// # Returns
/// The value, or 0 when it did not parse (the caller then has an error to report).
pub fn check(
    errors: &mut Vec<FieldError>,
    field: &'static str,
    value: Result<f64, &'static str>,
    rule: impl Fn(f64) -> Option<&'static str>,
) -> f64 {
    match value {
        Ok(v) => {
            if let Some(message) = rule(v) {
                errors.push(FieldError::field(field, message));
            }
            v
        }
        Err(message) => {
            errors.push(FieldError::field(field, message));
            0.0
        }
    }
}

/// Rule: greater than zero.
pub fn positive(v: f64) -> Option<&'static str> {
    (v <= 0.0).then_some(MUST_BE_POSITIVE)
}

/// Rule: zero or more.
pub fn not_negative(v: f64) -> Option<&'static str> {
    (v < 0.0).then_some(MUST_NOT_BE_NEGATIVE)
}

/// Rule: any value.
pub fn any(_v: f64) -> Option<&'static str> {
    None
}

/// A section heading.
///
/// # Arguments
/// * `ui` - The page's `Ui`.
/// * `tr` - Translator for the current UI language.
/// * `title` - The heading's msgid.
pub fn heading(ui: &mut egui::Ui, tr: &Translator, title: &str) {
    ui.add_space(6.0);
    ui.strong(tr.tr(title));
}

/// One labelled text field of a two-column grid, with its unit after it and the first
/// error about it underneath.
///
/// # Arguments
/// * `ui` - The grid's `Ui`.
/// * `tr` - Translator for the current UI language.
/// * `label` - The msgid of the field's label.
/// * `text` - The field's text.
/// * `unit` - The unit shown after the field (empty for none).
/// * `errors` - The page's problems.
pub fn field_row(
    ui: &mut egui::Ui,
    tr: &Translator,
    label: &'static str,
    text: &mut String,
    unit: &str,
    errors: &[FieldError],
) {
    let name = ui.label(tr.tr(label));
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(text)
                    .id_salt(label)
                    .desired_width(110.0),
            )
            .labelled_by(name.id);
            if !unit.is_empty() {
                ui.weak(unit);
            }
        });
        if let Some(error) = errors.iter().find(|e| e.field == Some(label)) {
            ui.colored_label(ui.visuals().error_fg_color, tr.tr(error.message));
        }
    });
    ui.end_row();
}

/// One labelled checkbox of a two-column grid.
///
/// # Arguments
/// * `ui` - The grid's `Ui`.
/// * `tr` - Translator for the current UI language.
/// * `label` - The msgid of the label.
/// * `value` - The checkbox's state.
pub fn check_row(ui: &mut egui::Ui, tr: &Translator, label: &'static str, value: &mut bool) {
    let name = ui.label(tr.tr(label));
    ui.checkbox(value, "").labelled_by(name.id);
    ui.end_row();
}

/// A two-column grid of fields.
///
/// # Arguments
/// * `ui` - The page's `Ui`.
/// * `id` - The grid's id salt.
/// * `rows` - Draws the rows.
pub fn grid(ui: &mut egui::Ui, id: &str, rows: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, rows);
}

/// The problems with the whole page, in the error colour.
///
/// # Arguments
/// * `ui` - The page's `Ui`.
/// * `tr` - Translator for the current UI language.
/// * `errors` - The page's problems.
pub fn page_errors(ui: &mut egui::Ui, tr: &Translator, errors: &[FieldError]) {
    for error in errors.iter().filter(|e| e.field.is_none()) {
        ui.colored_label(ui.visuals().error_fg_color, tr.tr(error.message));
    }
}

/// A length field's text as GTK's preset files hold it: millimetres, as `f32`.
///
/// # Arguments
/// * `field` - The msgid of the field's label.
/// * `text` - The field's text.
/// * `units` - The display units.
///
/// # Errors
/// The field and [`NOT_A_LENGTH`] when the text is not a length.
pub fn preset_len(
    field: &'static str,
    text: &str,
    units: MeasurementSystem,
) -> Result<f32, FieldError> {
    parse_len(text, units)
        .map(|mm| mm as f32)
        .map_err(|message| FieldError::field(field, message))
}

/// A number field's text as GTK's Drill Press preset holds it.
///
/// # Arguments
/// * `field` - The msgid of the field's label.
/// * `text` - The field's text.
///
/// # Errors
/// The field and [`NOT_A_NUMBER`] when the text is not a number.
pub fn preset_number(field: &'static str, text: &str) -> Result<f64, FieldError> {
    parse_number(text).map_err(|message| FieldError::field(field, message))
}

/// A page's input as preset file text, or why it cannot be saved.
///
/// # Arguments
/// * `value` - The page's preset ([`preset_len`] / [`preset_number`] failures become the
///   field's name and problem).
/// * `tr` - Translator for the current UI language.
///
/// # Errors
/// The field that does not parse, or the serialiser's error.
pub fn preset_text<T: serde::Serialize>(
    value: Result<T, FieldError>,
    tr: &Translator,
) -> Result<String, String> {
    let value = value.map_err(|e| {
        let field = e.field.map_or_else(String::new, |f| tr.tr(f).into_owned());
        format!("{field} {}", tr.tr(e.message))
    })?;
    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
}

/// A spoilboard preset file's JSON object (GTK read the keys it knew and ignored the rest).
///
/// # Arguments
/// * `json` - The file's text.
///
/// # Errors
/// The text is not JSON, or not an object.
pub fn preset_object(json: &str) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    match serde_json::from_str(json).map_err(|e| e.to_string())? {
        serde_json::Value::Object(map) => Ok(map),
        _ => Err("expected a JSON object".to_string()),
    }
}

/// Sets a length field from a preset object's number `key` (millimetres), if present.
///
/// # Arguments
/// * `text` - The field's text.
/// * `map` - The preset object.
/// * `key` - The key.
/// * `units` - The display units.
pub fn apply_len(
    text: &mut String,
    map: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    units: MeasurementSystem,
) {
    if let Some(mm) = map.get(key).and_then(serde_json::Value::as_f64) {
        *text = format_len(mm, units);
    }
}

/// Sets a field from a preset object's text `key`, if present.
///
/// # Arguments
/// * `text` - The field's text.
/// * `map` - The preset object.
/// * `key` - The key.
pub fn apply_text(text: &mut String, map: &serde_json::Map<String, serde_json::Value>, key: &str) {
    if let Some(value) = map.get(key).and_then(serde_json::Value::as_str) {
        *text = value.to_string();
    }
}

/// Generate, Send to Editor and Preview, disabled while the input has problems, then
/// Load and Save of the page's preset.
///
/// # Arguments
/// * `ui` - The page's `Ui`.
/// * `tr` - Translator for the current UI language.
/// * `valid` - Whether the input has no problems.
///
/// # Returns
/// The button pressed, if any.
pub fn gcode_actions(ui: &mut egui::Ui, tr: &Translator, valid: bool) -> CamAction {
    let mut action = CamAction::None;
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_enabled_ui(valid, |ui| {
            for (msgid, what) in [
                (n_!("Generate"), CamAction::Generate),
                (n_!("Send to Editor"), CamAction::SendToEditor),
                (n_!("Preview"), CamAction::Preview),
            ] {
                if ui.button(tr.tr(msgid)).clicked() {
                    action = what;
                }
            }
        });
        ui.separator();
        for (msgid, what) in [
            (n_!("Load"), CamAction::LoadPreset),
            (n_!("Save"), CamAction::SavePreset),
        ] {
            if ui.button(tr.tr(msgid)).clicked() {
                action = what;
            }
        }
    });
    action
}
