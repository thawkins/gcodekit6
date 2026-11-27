//! Widgets and validation shared by the CAM tool pages: labelled length and number fields
//! with their inline errors, the action buttons, and unit conversion of a field's text.
//!
//! Lengths are typed in the display units (`gcodekit5_core::units`, the same parse and
//! format as everywhere else) and held as text, so a value that does not parse stays on
//! screen with its error instead of being replaced. Feeds stay in mm/min and speeds in RPM,
//! whatever the display units.
//!
//! Dependencies: `egui`, `gcodekit5-core` (units), `gcodekit5-app` (translations).

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::{self, MeasurementSystem};

use crate::icons::{Icon, icon_button};

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
    /// Generate the program, hand it to the editor and show it in the visualizer.
    Preview,
    /// Load the page's input from a preset file.
    LoadPreset,
    /// Save the page's input to a preset file.
    SavePreset,
    /// Generate the page's shapes and add them to the design in the designer (the jigsaw's
    /// Generate and the tabbed box's Generate Shapes).
    SendToDesigner,
    /// Choose the Gerber project folder (Browse).
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
    units::format_length(mm, units)
}

/// Rewrites a length field's text from `from` units to `to` units; text that is not a
/// length is left as it is.
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

/// A section heading: the bold title of the framed group under it (a preferences
/// group title).
///
/// # Arguments
/// * `ui` - The page's `Ui`.
/// * `tr` - Translator for the current UI language.
/// * `title` - The heading's msgid.
pub fn heading(ui: &mut egui::Ui, tr: &Translator, title: &str) {
    ui.add_space(8.0);
    crate::theme::section_heading(ui, &tr.tr(title));
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

/// Narrowest a framed group of fields gets, so the groups of a page line up.
const GROUP_WIDTH: f32 = 460.0;

/// A two-column grid of fields in a framed group (a preferences group card).
///
/// # Arguments
/// * `ui` - The page's `Ui`.
/// * `id` - The grid's id salt.
/// * `rows` - Draws the rows.
pub fn grid(ui: &mut egui::Ui, id: &str, rows: impl FnOnce(&mut egui::Ui)) {
    let frame = crate::theme::group_frame(ui).inner_margin(egui::Margin::symmetric(10, 6));
    frame.show(ui, |ui| {
        ui.set_min_width(GROUP_WIDTH.min(ui.available_width()));
        egui::Grid::new(id)
            .num_columns(2)
            .spacing([12.0, 6.0])
            .min_col_width(160.0)
            .show(ui, rows);
    });
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

/// Load and Save of the page's preset (always enabled), then the page's action buttons,
/// disabled while the input has problems, the first given drawn last as the main one.
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
    // The order: Load and Save, then the page's actions with the main one (the first
    // given, the suggested action Generate) at the right end.
    ui.horizontal_wrapped(|ui| {
        for (icon, msgid, what) in [
            (Icon::Open, n_!("Load"), CamAction::LoadPreset),
            (Icon::Save, n_!("Save"), CamAction::SavePreset),
        ] {
            if icon_button(ui, icon, &tr.tr(msgid)).clicked() {
                action = what;
            }
        }
        ui.separator();
        let main = buttons.first();
        for (msgid, what) in buttons.iter().skip(1).chain(main) {
            let button = if Some(&(*msgid, *what)) == main {
                crate::theme::primary_button(tr.tr(msgid))
            } else {
                egui::Button::new(tr.tr(msgid))
            };
            if ui.add_enabled(valid, button).clicked() {
                action = *what;
            }
        }
    });
    action
}

/// Share of a tool page's width its sidebar takes (40%).
pub const SIDEBAR_SHARE: f32 = 0.40;

/// The id under which [`cam_page`] leaves the sidebar's unused room, for the pane's own
/// sidebar items (the bitmap job's progress, the generated program).
pub(super) fn sidebar_rest_id() -> egui::Id {
    egui::Id::unique("gcodekit5-cam-sidebar-rest")
}

/// Lays a tool page out: a sidebar on the left (the page's description), and
/// on the right the form, which scrolls, over the action buttons, which stay at the bottom
/// right. `state` (the page's input) is handed to each part in turn: first the sidebar,
/// then the form, then the actions.
///
/// # Arguments
/// * `ui` - The page's `Ui`; the page takes all the room left in it.
/// * `share` - The sidebar's share of the width, such as [`SIDEBAR_SHARE`].
/// * `state` - The page's input.
/// * `sidebar` - Draws the sidebar.
/// * `form` - Draws the form; returns what a button in it asked for, if anything.
/// * `actions` - Draws the action buttons; returns the one pressed, if any.
///
/// # Returns
/// The action buttons' answer, else the form's.
pub fn cam_page<S>(
    ui: &mut egui::Ui,
    share: f32,
    state: &mut S,
    sidebar: impl FnOnce(&mut egui::Ui, &mut S),
    form: impl FnOnce(&mut egui::Ui, &mut S) -> CamAction,
    actions: impl FnOnce(&mut egui::Ui, &mut S) -> CamAction,
) -> CamAction {
    const GAP: f32 = 12.0;
    let rect = ui.available_rect_before_wrap();
    let side_width = (rect.width() * share).round();
    let divider = rect.left() + side_width;
    let side_rect = egui::Rect::from_min_max(rect.min, egui::pos2(divider - GAP, rect.bottom()));
    let right = egui::Rect::from_min_max(egui::pos2(divider + GAP, rect.top()), rect.max);
    // The action row's size from the last frame (one row until it has been drawn).
    // Kept per page (each page has its own input type), so a page shown after another
    // does not start from that page's row width.
    let size_id = ui
        .scope_id()
        .with(("gcodekit5-cam-actions-size", std::any::type_name::<S>()));
    let last: egui::Vec2 = ui
        .data(|d| d.get_temp(size_id))
        .unwrap_or(egui::vec2(0.0, ui.spacing().interact_size.y + 12.0));
    let actions_top = right.bottom() - last.y;
    let form_rect = egui::Rect::from_min_max(right.min, egui::pos2(right.right(), actions_top));
    let actions_rect = egui::Rect::from_min_max(egui::pos2(right.left(), actions_top), right.max);
    ui.painter().vline(
        divider,
        rect.y_range(),
        ui.visuals().widgets.noninteractive.bg_stroke,
    );

    let top_down = egui::Layout::top_down(egui::Align::Min);
    let mut side = ui.new_child(egui::UiBuilder::new().max_rect(side_rect).layout(top_down));
    sidebar(&mut side, state);
    let rest_top = side.min_rect().bottom() + GAP;
    let rest = egui::Rect::from_min_max(egui::pos2(side_rect.left(), rest_top), side_rect.max);
    ui.data_mut(|d| d.insert_temp(sidebar_rest_id(), rest));

    let mut form_ui = ui.new_child(egui::UiBuilder::new().max_rect(form_rect).layout(top_down));
    let from_form = egui::ScrollArea::vertical()
        .id_salt("cam-tool-page")
        .auto_shrink([false, false])
        .show(&mut form_ui, |ui| form(ui, state))
        .inner;

    // Right aligned: the row starts its own width, as measured last
    // frame, left of the right edge. Its room reaches past that edge, so the measured width
    // is the row's natural one, not one squeezed by the offset.
    let start = (actions_rect.right() - last.x).max(actions_rect.left());
    let room = egui::Rect::from_min_size(
        egui::pos2(start, actions_rect.top()),
        egui::vec2(actions_rect.width(), actions_rect.height()),
    );
    let mut actions_ui = ui.new_child(egui::UiBuilder::new().max_rect(room).layout(top_down));
    actions_ui.set_clip_rect(actions_rect.intersect(ui.clip_rect()));
    let pressed = actions(&mut actions_ui, state);
    let size = actions_ui.min_rect().size();
    ui.data_mut(|d| d.insert_temp(size_id, size));
    // A row that changed size (first visit, Stop appearing, another language) is placed
    // right in a second pass of this frame instead of waiting for the next input.
    if (size - last).abs().max_elem() > 0.5 {
        ui.ctx().request_discard("cam action row size");
    }
    ui.allocate_rect(rect, egui::Sense::hover());
    if pressed == CamAction::None {
        from_form
    } else {
        pressed
    }
}
