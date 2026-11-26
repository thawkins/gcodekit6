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
    /// Generate the page's shapes and add them to the design in the designer (GTK's jigsaw
    /// Generate and the tabbed box's Generate Shapes).
    SendToDesigner,
    /// Choose the Gerber project folder (GTK's Browse).
    ChooseFolder,
    /// Choose the image to engrave (the Laser Image Engraver's Browse).
    ChooseImage,
    /// Run the probe routine on the machine.
    Probe,
    /// Write the last probe result into the work coordinate system.
    ApplyWcs,
    /// Stop the running probe routine (soft reset).
    StopProbe,
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
/// Message: the text is not a whole number.
pub const NOT_A_WHOLE_NUMBER: &str = n_!("Not a whole number");
/// Message: the value is outside the field's range.
pub const OUT_OF_RANGE: &str = n_!("Out of range");
/// Message: the tool needs a Z axis.
pub const NEEDS_THREE_AXES: &str =
    n_!("This tool needs at least 3 axes (X, Y, Z); the active device has fewer.");

pub use super::preset_fields::{
    apply_len, apply_text, preset_len, preset_number, preset_object, preset_text,
};

/// The msgids the shared widgets show, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        NOT_A_LENGTH,
        NOT_A_NUMBER,
        NOT_A_WHOLE_NUMBER,
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

/// Reads a whole number (a piece count, laser passes).
///
/// # Arguments
/// * `text` - The field's text.
///
/// # Errors
/// [`NOT_A_WHOLE_NUMBER`] when the text is not an integer.
pub fn parse_whole(text: &str) -> Result<f64, &'static str> {
    text.trim()
        .parse::<i64>()
        .map(|v| v as f64)
        .map_err(|_| NOT_A_WHOLE_NUMBER)
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

/// `v` as the generators get it (`f32`), or [`OUT_OF_RANGE`] when it does not fit one (a
/// value too large becomes infinity).
fn as_f32(v: f64) -> Result<f32, &'static str> {
    let v = v as f32;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(OUT_OF_RANGE)
    }
}

/// Rule: greater than zero (compared as the generators do, in `f32`, so a value that rounds
/// to zero is refused, and one too large for `f32` is [`OUT_OF_RANGE`]).
pub fn positive(v: f64) -> Option<&'static str> {
    if v <= 0.0 {
        return Some(MUST_BE_POSITIVE);
    }
    match as_f32(v) {
        Ok(v) => (v <= 0.0).then_some(MUST_BE_POSITIVE),
        Err(message) => Some(message),
    }
}

/// Rule: zero or more (compared as the generators do, in `f32`).
pub fn not_negative(v: f64) -> Option<&'static str> {
    match as_f32(v) {
        Ok(v) => (v < 0.0).then_some(MUST_NOT_BE_NEGATIVE),
        Err(_) if v < 0.0 => Some(MUST_NOT_BE_NEGATIVE),
        Err(message) => Some(message),
    }
}

/// Rule: at least `min` (compared as the generators do, in `f32`; a value too large for
/// `f32` is [`OUT_OF_RANGE`]).
///
/// # Arguments
/// * `min` - The smallest allowed value.
/// * `message` - The msgid of the message for a smaller value.
pub fn at_least(min: f64, message: &'static str) -> impl Fn(f64) -> Option<&'static str> {
    move |v| match as_f32(v) {
        Ok(v) => (v < min as f32).then_some(message),
        Err(_) if v < min => Some(message),
        Err(range) => Some(range),
    }
}

/// Rule: from `min` to `max`, both allowed (compared as the generators do, in `f32`).
///
/// # Arguments
/// * `min` - The smallest allowed value.
/// * `max` - The largest allowed value.
/// * `message` - The msgid of the message for a value outside.
pub fn between(min: f64, max: f64, message: &'static str) -> impl Fn(f64) -> Option<&'static str> {
    move |v| {
        let v = v as f32;
        (v < min as f32 || v > max as f32).then_some(message)
    }
}

/// `rule` while `strict`, no rule otherwise: a preset only needs its fields to parse.
///
/// # Arguments
/// * `strict` - Whether the rule applies.
/// * `rule` - The rule.
pub fn gated(
    strict: bool,
    rule: impl Fn(f64) -> Option<&'static str>,
) -> impl Fn(f64) -> Option<&'static str> {
    move |v| if strict { rule(v) } else { None }
}

/// The first of `errors`, as a preset's refusal.
///
/// # Arguments
/// * `errors` - The problems found.
pub fn first_error(errors: Vec<FieldError>) -> FieldError {
    errors
        .into_iter()
        .next()
        .unwrap_or_else(|| FieldError::page(n_!("Invalid Parameters")))
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

/// One labelled drop-down of a two-column grid.
///
/// # Arguments
/// * `ui` - The grid's `Ui`.
/// * `tr` - Translator for the current UI language.
/// * `id` - The drop-down's id salt.
/// * `label` - The msgid of the label.
/// * `value` - The chosen option.
/// * `options` - The options and the msgids of their names.
pub fn combo_row<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    tr: &Translator,
    id: &str,
    label: &'static str,
    value: &mut T,
    options: &[(T, &'static str)],
) {
    let name = ui.label(tr.tr(label));
    let shown = options
        .iter()
        .find(|(option, _)| option == value)
        .map_or("", |(_, msgid)| msgid);
    egui::ComboBox::from_id_salt(id)
        .selected_text(tr.tr(shown))
        .show_ui(ui, |ui| {
            for (option, msgid) in options {
                ui.selectable_value(value, *option, tr.tr(msgid));
            }
        })
        .response
        .labelled_by(name.id);
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
    crate::theme::checkbox(ui, value, "").labelled_by(name.id);
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
    action_row(
        ui,
        tr,
        valid,
        &[
            (n_!("Generate"), CamAction::Generate),
            (n_!("Send to Editor"), CamAction::SendToEditor),
            (n_!("Preview"), CamAction::Preview),
        ],
    )
}

/// The page's action buttons, disabled while the input has problems, then Load and Save of
/// the page's preset (always enabled).
///
/// # Arguments
/// * `ui` - The page's `Ui`.
/// * `tr` - Translator for the current UI language.
/// * `valid` - Whether the input has no problems.
/// * `buttons` - The action buttons: label msgid and what the button asks for.
///
/// # Returns
/// The button pressed, if any.
pub fn action_row(
    ui: &mut egui::Ui,
    tr: &Translator,
    valid: bool,
    buttons: &[(&'static str, CamAction)],
) -> CamAction {
    let mut action = CamAction::None;
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        for (msgid, what) in buttons {
            if ui
                .add_enabled(valid, egui::Button::new(tr.tr(msgid)))
                .clicked()
            {
                action = *what;
            }
        }
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
