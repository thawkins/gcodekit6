//! Reading and writing a CAM page's fields as the preset files hold them: lengths in
//! millimetres as `f32`, numbers as typed, and the JSON object helpers of the
//! spoilboard presets. Re-exported from [`super::common`].
//!
//! Dependencies: `gcodekit5-core` (units), `serde` / `serde_json`, `gcodekit5-app`
//! (translations).

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::MeasurementSystem;

use super::common::{FieldError, format_len, parse_len, parse_number};

/// A length field's text as the preset files hold it: millimetres, as `f32`.
///
/// # Arguments
/// * `field` - The msgid of the field's label.
/// * `text` - The field's text.
/// * `units` - The display units.
///
/// # Errors
/// The field and [`super::common::NOT_A_LENGTH`] when the text is not a length.
pub fn preset_len(
    field: &'static str,
    text: &str,
    units: MeasurementSystem,
) -> Result<f32, FieldError> {
    parse_len(text, units)
        .map(|mm| mm as f32)
        .map_err(|message| FieldError::field(field, message))
}

/// A number field's text as the Drill Press preset holds it.
///
/// # Arguments
/// * `field` - The msgid of the field's label.
/// * `text` - The field's text.
///
/// # Errors
/// The field and [`NOT_A_NUMBER`](super::common::NOT_A_NUMBER) when the text is not a number.
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

/// A spoilboard preset file's JSON object (unknown keys are ignored).
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
