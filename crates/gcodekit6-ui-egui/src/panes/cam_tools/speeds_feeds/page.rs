//! The Speeds and Feeds page: results, the active device, the tool and material selection
//! (a searchable dialog over each library), the operation, the cutting
//! parameters, the options, and Calculate / Reset.
//!
//! Dependencies: `egui`, `gcodekit6-camtools` (operation types), `gcodekit6-app`
//! (translations).

use gcodekit6_app::i18n::Translator;
use gcodekit6_camtools::speeds_feeds::OperationType;

use super::super::common::{self, CamAction, FieldError, field_row, grid, heading};
use super::{
    DEPTH, MATERIAL, NO_MATERIAL, NO_TOOL, Picker, PickerKind, STEPOVER, STICK_OUT,
    SpeedsFeedsParams, TOOL,
};

/// The operations, in display order, with their msgids.
const OPERATIONS: [(OperationType, &str); 6] = [
    (OperationType::Slotting, n_!("Slotting")),
    (OperationType::Pocketing, n_!("Pocketing")),
    (OperationType::Profiling, n_!("Profiling")),
    (OperationType::Adaptive, n_!("Adaptive / HSM")),
    (OperationType::Drilling, n_!("Drilling")),
    (OperationType::Plunging, n_!("Plunging")),
];

/// The msgids the page shows, for the translation check.
pub(super) fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        n_!("Calculated Results"),
        n_!("Spindle Speed (RPM):"),
        n_!("Feed Rate (mm/min):"),
        n_!("Surface Speed (SFM):"),
        n_!("Chip Load (mm/tooth):"),
        n_!("MRR (mm³/min):"),
        n_!("Power (W):"),
        n_!("Est. Deflection (mm):"),
        n_!("Active Device"),
        n_!("Device:"),
        n_!("Max RPM:"),
        n_!("Spindle Power:"),
        n_!("Max Feed Rate:"),
        n_!("No device selected"),
        n_!("N/A"),
        n_!("Selections"),
        n_!("Select Tool..."),
        n_!("Select Material..."),
        n_!("Operation"),
        n_!("Operation Type:"),
        n_!("Cutting Parameters"),
        n_!("0 = Use operation default"),
        n_!("Options"),
        n_!("Coolant:"),
        n_!("Coolant Enabled"),
        n_!("Device does not support coolant"),
        n_!("Safety Factor:"),
        n_!("Conservative Calculation"),
        n_!("Reduce calculated values by 20%"),
        n_!("Calculate"),
        n_!("Reset"),
        n_!("Select Tool"),
        n_!("Select Material"),
        n_!("Search tools..."),
        n_!("Search materials..."),
        n_!("No tools available"),
        n_!("Add tools in the CNC Tools manager"),
        n_!("No materials available"),
        n_!("Add materials in the Materials manager"),
        n_!("Select"),
        n_!("Cancel"),
    ];
    ids.extend(OPERATIONS.map(|(_, msgid)| msgid));
    ids
}

/// A label and a value of a two-column grid.
fn value_row(ui: &mut egui::Ui, tr: &Translator, label: &str, value: &str) {
    ui.weak(tr.tr(label));
    ui.label(value);
    ui.end_row();
}

/// The results and the active device.
fn results(ui: &mut egui::Ui, params: &SpeedsFeedsParams, tr: &Translator) {
    heading(ui, tr, n_!("Calculated Results"));
    let r = params.results();
    let value = |f: fn(&super::SpeedsFeedsResults) -> &String| r.map_or("--", |r| f(r).as_str());
    grid(ui, "cam-sf-results", |ui| {
        value_row(ui, tr, "Spindle Speed (RPM):", value(|r| &r.spindle_speed));
        value_row(ui, tr, "Feed Rate (mm/min):", value(|r| &r.feed_rate));
        value_row(ui, tr, "Surface Speed (SFM):", value(|r| &r.surface_speed));
        value_row(ui, tr, "Chip Load (mm/tooth):", value(|r| &r.chip_load));
        value_row(ui, tr, "MRR (mm³/min):", value(|r| &r.mrr));
        value_row(ui, tr, "Power (W):", value(|r| &r.power));
        value_row(ui, tr, "Est. Deflection (mm):", value(|r| &r.deflection));
    });
    heading(ui, tr, n_!("Active Device"));
    grid(ui, "cam-sf-device", |ui| match &params.device {
        Some(d) => {
            let na = tr.tr("N/A").into_owned();
            let spindle = |text: String| if d.has_spindle { text } else { na.clone() };
            value_row(ui, tr, "Device:", &d.name);
            value_row(
                ui,
                tr,
                "Max RPM:",
                &spindle(d.max_spindle_speed_rpm.to_string()),
            );
            value_row(
                ui,
                tr,
                "Spindle Power:",
                &spindle(format!("{:.0} W", d.cnc_spindle_watts)),
            );
            value_row(
                ui,
                tr,
                "Max Feed Rate:",
                &format!("{:.0} mm/min", d.max_feed_rate),
            );
        }
        None => {
            value_row(ui, tr, "Device:", &tr.tr("No device selected"));
            for label in ["Max RPM:", "Spindle Power:", "Max Feed Rate:"] {
                value_row(ui, tr, label, "--");
            }
        }
    });
}

/// One selection row: the label, the button that opens the dialog and what is chosen.
fn selection_row(
    ui: &mut egui::Ui,
    tr: &Translator,
    (label, button, empty): (&'static str, &str, &str),
    chosen: Option<String>,
    errors: &[FieldError],
) -> bool {
    ui.label(tr.tr(label));
    let mut open = false;
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            open = ui.button(tr.tr(button)).clicked();
            match chosen {
                Some(name) => ui.label(name),
                None => ui.weak(tr.tr(empty)),
            };
        });
        // "Nothing chosen" is already said beside the button.
        if let Some(error) = errors
            .iter()
            .find(|e| e.field == Some(label) && e.message != empty)
        {
            ui.colored_label(ui.visuals().error_fg_color, tr.tr(error.message));
        }
    });
    ui.end_row();
    open
}

/// The selection dialog's rows: library index and text, filtered by the search.
fn picker_rows(params: &SpeedsFeedsParams, picker: &Picker) -> Vec<(usize, String)> {
    let query = picker.search.trim().to_lowercase();
    let rows: Vec<(usize, String)> = match picker.kind {
        PickerKind::Tool => params
            .tools
            .iter()
            .enumerate()
            .map(|(i, t)| {
                (
                    i,
                    format!("{} - ø{:.1}mm {}", t.name, t.diameter, t.tool_type),
                )
            })
            .collect(),
        PickerKind::Material => params
            .materials
            .iter()
            .enumerate()
            .map(|(i, m)| (i, format!("{} ({})", m.name, m.category)))
            .collect(),
    };
    rows.into_iter()
        .filter(|(_, text)| query.is_empty() || text.to_lowercase().contains(&query))
        .collect()
}

/// The open selection dialog, if any; Select (or a double click) chooses the row.
fn show_picker(ui: &mut egui::Ui, params: &mut SpeedsFeedsParams, tr: &Translator) {
    let Some(mut picker) = params.picker.take() else {
        return;
    };
    let rows = picker_rows(params, &picker);
    let (title, hint, empty, add) = match picker.kind {
        PickerKind::Tool => (
            "Select Tool",
            "Search tools...",
            "No tools available",
            "Add tools in the CNC Tools manager",
        ),
        PickerKind::Material => (
            "Select Material",
            "Search materials...",
            "No materials available",
            "Add materials in the Materials manager",
        ),
    };
    let mut done = None;
    let modal = egui::Modal::new(egui::Id::unique("gcodekit6-cam-speeds-feeds-picker")).show(
        ui.ctx(),
        |ui| {
            ui.set_width(420.0);
            ui.heading(tr.tr(title));
            let hint = tr.tr(hint);
            let search =
                ui.add(egui::TextEdit::singleline(&mut picker.search).hint_text(hint.as_ref()));
            search.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &hint));
            if search.changed() {
                picker.chosen = None;
            }
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    if rows.is_empty() {
                        ui.strong(tr.tr(empty));
                        ui.weak(tr.tr(add));
                    }
                    for (index, text) in &rows {
                        let row = ui.selectable_label(picker.chosen == Some(*index), text);
                        if row.clicked() {
                            picker.chosen = Some(*index);
                        }
                        if row.double_clicked() {
                            done = Some(Some(*index));
                        }
                    }
                });
            ui.horizontal(|ui| {
                if ui.button(tr.tr("Cancel")).clicked() {
                    done = Some(None);
                }
                let select = egui::Button::new(tr.tr("Select"));
                if ui.add_enabled(picker.chosen.is_some(), select).clicked() {
                    done = Some(picker.chosen);
                }
            });
        },
    );
    if modal.should_close() {
        done = Some(None);
    }
    match done {
        None => params.picker = Some(picker),
        Some(None) => {}
        Some(Some(index)) => match picker.kind {
            PickerKind::Tool => params.tool = params.tools.get(index).cloned(),
            PickerKind::Material => params.material = params.materials.get(index).cloned(),
        },
    }
}

/// Draws the page.
///
/// # Arguments
/// * `ui` - The page's `Ui`.
/// * `params` - The calculator's input and results.
/// * `tr` - Translator for the current UI language.
///
/// # Returns
/// [`CamAction::Generate`] when Calculate is pressed (never while the input has problems).
pub fn show(ui: &mut egui::Ui, params: &mut SpeedsFeedsParams, tr: &Translator) -> CamAction {
    let errors = params.input().err().unwrap_or_default();
    // The layout: the results in the sidebar (35%), the input beside them, Calculate
    // and Reset at the bottom right.
    let action = common::cam_page(
        ui,
        SIDEBAR_SHARE,
        params,
        |ui, params| results(ui, params, tr),
        |ui, params| {
            inputs(ui, params, tr, &errors);
            CamAction::None
        },
        |ui, params| buttons(ui, params, tr, &errors),
    );
    show_picker(ui, params, tr);
    action
}

/// Share of the width the results take (35%).
const SIDEBAR_SHARE: f32 = 0.35;

/// The selections, operation, cutting parameters and options.
fn inputs(
    ui: &mut egui::Ui,
    params: &mut SpeedsFeedsParams,
    tr: &Translator,
    errors: &[FieldError],
) {
    heading(ui, tr, n_!("Selections"));
    let mut open = None;
    grid(ui, "cam-sf-selection", |ui| {
        let tool = params
            .tool
            .as_ref()
            .map(|t| format!("{} - ø{:.1}mm", t.name, t.diameter));
        if selection_row(ui, tr, (TOOL, "Select Tool...", NO_TOOL), tool, errors) {
            open = Some(PickerKind::Tool);
        }
        let material = params.material.as_ref().map(|m| m.name.clone());
        let row = (MATERIAL, "Select Material...", NO_MATERIAL);
        if selection_row(ui, tr, row, material, errors) {
            open = Some(PickerKind::Material);
        }
    });
    if let Some(kind) = open {
        params.picker = Some(Picker {
            kind,
            search: String::new(),
            chosen: None,
        });
    }
    heading(ui, tr, n_!("Operation"));
    grid(ui, "cam-sf-operation", |ui| {
        let name = ui.label(tr.tr("Operation Type:"));
        let shown = OPERATIONS
            .iter()
            .find(|(op, _)| *op == params.operation)
            .map_or("", |(_, msgid)| msgid);
        egui::ComboBox::from_id_salt("cam-sf-operation-type")
            .selected_text(tr.tr(shown))
            .show_ui(ui, |ui| {
                for (op, msgid) in OPERATIONS {
                    ui.selectable_value(&mut params.operation, op, tr.tr(msgid));
                }
            })
            .response
            .labelled_by(name.id);
        ui.end_row();
    });
    heading(ui, tr, n_!("Cutting Parameters"));
    grid(ui, "cam-sf-cutting", |ui| {
        field_row(ui, tr, DEPTH, &mut params.depth_of_cut, "", errors);
        field_row(ui, tr, STEPOVER, &mut params.stepover, "", errors);
        field_row(ui, tr, STICK_OUT, &mut params.stick_out, "", errors);
    });
    ui.weak(tr.tr("0 = Use operation default"));
    heading(ui, tr, n_!("Options"));
    grid(ui, "cam-sf-options", |ui| {
        let has_coolant = params.device.as_ref().is_some_and(|d| d.has_coolant);
        ui.label(tr.tr("Coolant:"));
        let on = params.coolant;
        let coolant = crate::theme::checked_scope(ui, on, |ui| {
            ui.add_enabled(
                has_coolant,
                egui::Checkbox::new(&mut params.coolant, tr.tr("Coolant Enabled")),
            )
        });
        if !has_coolant {
            coolant.on_disabled_hover_text(tr.tr("Device does not support coolant"));
        }
        ui.end_row();
        ui.label(tr.tr("Safety Factor:"));
        crate::theme::checkbox(
            ui,
            &mut params.conservative,
            tr.tr("Conservative Calculation"),
        )
        .on_hover_text(tr.tr("Reduce calculated values by 20%"));
        ui.end_row();
    });
    common::page_errors(ui, tr, errors);
}

/// Calculate and Reset.
fn buttons(
    ui: &mut egui::Ui,
    params: &mut SpeedsFeedsParams,
    tr: &Translator,
    errors: &[FieldError],
) -> CamAction {
    let mut action = CamAction::None;
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let calculate = crate::theme::primary_button(tr.tr("Calculate"));
        if ui.add_enabled(errors.is_empty(), calculate).clicked() {
            action = CamAction::Generate;
        }
        if ui.button(tr.tr("Reset")).clicked() {
            params.reset();
        }
    });
    action
}
