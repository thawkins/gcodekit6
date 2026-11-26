//! Drawing of the Probe Tools page: the machine's state, the routine and its parameters
//! (only the settings the chosen routine uses, as GTK showed them), the last result, and
//! the actions.
//!
//! Dependencies: `egui`, `gcodekit5-core` (probe types), `gcodekit5-app` (translations).

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units;
use gcodekit5_core::{Corner, ProbeAxis, ProbeDirection};

use super::super::common::{self, CamAction, check_row, field_row, grid, heading};
use super::{
    BACKOFF, DIAMETER, EDGE_DISTANCE, FAST_FEED, MAX_DEPTH, PLATE_THICKNESS, PROBE_DEPTH,
    ProbeParams, RoutineKind, SAFE_HEIGHT, SETTER_X, SETTER_Y, SETTER_Z, SLOW_FEED, TIP_RADIUS,
};

const DESCRIPTION: &str = n_!(
    "Configure and run touch probe routines for work coordinate setup. Includes Z-touch, edge \
     find, corner find, bore/boss center, and tool length measurement."
);
const OFFLINE: &str = n_!("Device connection required to use Probe Tools");
const ROUTINE: &str = n_!("Routine");
const TARGET_WCS: &str = n_!("Target WCS");
const AUTO_UPDATE: &str = n_!("Auto-Update WCS");
const AUTO_UPDATE_HELP: &str = n_!(
    "Write the result into the selected WCS (G10 L20) from the generated G-code. Z-touch, \
     edge and corner only; bore/boss centres are not set automatically."
);
const PROBE_AXIS: &str = n_!("Probe Axis");
const PROBE_DIRECTION: &str = n_!("Probe Direction");
const CORNER: &str = n_!("Corner");
const STATUS: &str = n_!("Status:");
const PROBE_INPUT: &str = n_!("Probe:");
const TRIGGERED: &str = n_!("Triggered");
const OPEN: &str = n_!("Not triggered");
const PROBE: &str = n_!("Probe");
const APPLY: &str = n_!("Apply to WCS");
const STOP: &str = n_!("Stop Probe");

/// The msgids the page shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        DESCRIPTION,
        OFFLINE,
        ROUTINE,
        TARGET_WCS,
        AUTO_UPDATE,
        AUTO_UPDATE_HELP,
        PROBE_AXIS,
        PROBE_DIRECTION,
        CORNER,
        STATUS,
        PROBE_INPUT,
        TRIGGERED,
        OPEN,
        PROBE,
        APPLY,
        STOP,
        n_!("Routine Selection"),
        n_!("Parameters"),
        n_!("Routine Settings"),
        n_!("Results"),
    ];
    ids.extend(AXES.map(|(_, m)| m));
    ids.extend(DIRECTIONS.map(|(_, m)| m));
    ids.extend(CORNERS.map(|(_, m)| m));
    ids
}

const AXES: [(ProbeAxis, &str); 3] = [
    (ProbeAxis::X, n_!("X Axis")),
    (ProbeAxis::Y, n_!("Y Axis")),
    (ProbeAxis::Z, n_!("Z Axis")),
];
const DIRECTIONS: [(ProbeDirection, &str); 2] = [
    (ProbeDirection::Negative, n_!("Negative")),
    (ProbeDirection::Positive, n_!("Positive")),
];
const CORNERS: [(Corner, &str); 4] = [
    (Corner::XminYmin, n_!("X-min / Y-min")),
    (Corner::XmaxYmin, n_!("X-max / Y-min")),
    (Corner::XminYmax, n_!("X-min / Y-max")),
    (Corner::XmaxYmax, n_!("X-max / Y-max")),
];

/// What the page shows of the machine and of the runs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProbeView {
    /// Whether the machine is connected.
    pub connected: bool,
    /// The machine state of the latest status report.
    pub state: Option<String>,
    /// Whether the probe input is triggered, when the report says.
    pub probe_pin: Option<bool>,
    /// Why a run cannot start now (translated), if it cannot.
    pub not_ready: Option<String>,
    /// Whether the routine streams (Stop Probe is offered).
    pub streaming: bool,
    /// Whether a successful run can be applied.
    pub can_apply: bool,
    /// The last run's text.
    pub result: Option<String>,
}

/// A drop-down over `options` whose unchosen value (`None`) shows the first option.
fn optional_combo<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    tr: &Translator,
    label: &'static str,
    value: &mut Option<T>,
    options: &[(T, &'static str)],
) {
    let name = ui.label(tr.tr(label));
    let shown = value
        .and_then(|v| options.iter().find(|(o, _)| *o == v))
        .or(options.first())
        .map_or("", |(_, msgid)| msgid);
    egui::ComboBox::from_id_salt(label)
        .selected_text(tr.tr(shown))
        .show_ui(ui, |ui| {
            for (option, msgid) in options {
                if ui
                    .selectable_label(*value == Some(*option), tr.tr(msgid))
                    .clicked()
                {
                    *value = Some(*option);
                }
            }
        })
        .response
        .labelled_by(name.id);
    ui.end_row();
}

/// Draws the page.
///
/// # Arguments
/// * `ui` - The page's `Ui`.
/// * `params` - The page's input.
/// * `view` - The machine's state and the runs.
/// * `tr` - Translator for the current UI language.
///
/// # Returns
/// The action button pressed, if any; never one whose conditions do not hold.
pub fn show(
    ui: &mut egui::Ui,
    params: &mut ProbeParams,
    view: &ProbeView,
    tr: &Translator,
) -> CamAction {
    ui.label(tr.tr(DESCRIPTION));
    ui.horizontal(|ui| {
        ui.label(format!(
            "{} {}",
            tr.tr(STATUS),
            view.state.as_deref().unwrap_or("-")
        ));
        let pin = if view.probe_pin == Some(true) {
            TRIGGERED
        } else {
            OPEN
        };
        ui.label(format!("{} {}", tr.tr(PROBE_INPUT), tr.tr(pin)));
    });
    if !view.connected {
        ui.colored_label(ui.visuals().error_fg_color, tr.tr(OFFLINE));
    }
    let errors = params.parameters().err().unwrap_or_default();
    let unit = units::get_unit_label(params.units);
    heading(ui, tr, n_!("Routine Selection"));
    grid(ui, "cam-probe-routine", |ui| {
        let kinds = RoutineKind::ALL.map(|k| (k, k.msgid()));
        common::combo_row(
            ui,
            tr,
            "cam-probe-kind",
            ROUTINE,
            &mut params.routine,
            &kinds,
        );
    });
    heading(ui, tr, n_!("Parameters"));
    grid(ui, "cam-probe-params", |ui| {
        field_row(ui, tr, SAFE_HEIGHT, &mut params.safe_height, unit, &errors);
        field_row(ui, tr, FAST_FEED, &mut params.fast_feed, "mm/min", &errors);
        field_row(ui, tr, SLOW_FEED, &mut params.slow_feed, "mm/min", &errors);
        field_row(ui, tr, BACKOFF, &mut params.backoff, unit, &errors);
        field_row(ui, tr, MAX_DEPTH, &mut params.max_depth, unit, &errors);
        let wcs: Vec<(u8, &'static str)> = vec![
            (54, "G54"),
            (55, "G55"),
            (56, "G56"),
            (57, "G57"),
            (58, "G58"),
            (59, "G59"),
        ];
        common::combo_row(ui, tr, "cam-probe-wcs", TARGET_WCS, &mut params.wcs, &wcs);
        check_row(ui, tr, AUTO_UPDATE, &mut params.auto_update);
    });
    ui.weak(tr.tr(AUTO_UPDATE_HELP));
    heading(ui, tr, n_!("Routine Settings"));
    grid(ui, "cam-probe-settings", |ui| {
        routine_settings(ui, params, tr, &errors, unit);
    });
    if let Some(result) = &view.result {
        heading(ui, tr, n_!("Results"));
        ui.label(result);
    }
    common::page_errors(ui, tr, &errors);
    // A field the routine does not show (one loaded from a preset) is named here.
    let shown = shown_fields(params.routine);
    for error in &errors {
        if let Some(field) = error.field.filter(|f| !shown.contains(f)) {
            let text = format!("{} {}", tr.tr(field), tr.tr(error.message));
            ui.colored_label(ui.visuals().error_fg_color, text);
        }
    }
    let valid = errors.is_empty();
    let mut action = common::gcode_actions(ui, tr, valid);
    ui.horizontal_wrapped(|ui| {
        let ready = view.not_ready.is_none();
        if ui
            .add_enabled(valid && ready, egui::Button::new(tr.tr(PROBE)))
            .clicked()
        {
            action = CamAction::Probe;
        }
        if ui
            .add_enabled(view.can_apply && ready, egui::Button::new(tr.tr(APPLY)))
            .clicked()
        {
            action = CamAction::ApplyWcs;
        }
        if view.streaming
            && ui
                .add(crate::theme::destructive_button(ui, tr.tr(STOP)))
                .clicked()
        {
            action = CamAction::StopProbe;
        }
        if let Some(reason) = &view.not_ready {
            ui.weak(reason);
        }
    });
    action
}

/// The rows of the settings the chosen routine uses (GTK's visibility rules).
fn routine_settings(
    ui: &mut egui::Ui,
    p: &mut ProbeParams,
    tr: &Translator,
    errors: &[common::FieldError],
    unit: &str,
) {
    let kind = p.routine;
    let sideways = matches!(
        kind,
        RoutineKind::EdgeFind
            | RoutineKind::CornerFind
            | RoutineKind::BoreCenter
            | RoutineKind::BossCenter
    );
    if kind == RoutineKind::EdgeFind {
        optional_combo(ui, tr, PROBE_AXIS, &mut p.edge_axis, &AXES);
        optional_combo(ui, tr, PROBE_DIRECTION, &mut p.edge_direction, &DIRECTIONS);
    }
    if kind == RoutineKind::CornerFind {
        optional_combo(ui, tr, CORNER, &mut p.corner, &CORNERS);
    }
    if matches!(kind, RoutineKind::EdgeFind | RoutineKind::CornerFind) {
        field_row(ui, tr, EDGE_DISTANCE, &mut p.edge_distance, unit, errors);
    }
    if matches!(kind, RoutineKind::BoreCenter | RoutineKind::BossCenter) {
        field_row(ui, tr, DIAMETER, &mut p.diameter, unit, errors);
    }
    if kind == RoutineKind::ToolLength {
        field_row(ui, tr, SETTER_X, &mut p.setter_x, unit, errors);
        field_row(ui, tr, SETTER_Y, &mut p.setter_y, unit, errors);
        field_row(ui, tr, SETTER_Z, &mut p.setter_z, unit, errors);
    }
    if sideways {
        field_row(ui, tr, PROBE_DEPTH, &mut p.probe_depth, unit, errors);
        field_row(ui, tr, TIP_RADIUS, &mut p.tip_radius, unit, errors);
    }
    if matches!(kind, RoutineKind::ZTouch | RoutineKind::EdgeFind) {
        field_row(
            ui,
            tr,
            PLATE_THICKNESS,
            &mut p.plate_thickness,
            unit,
            errors,
        );
    }
}

/// The fields the page shows for `kind`: the common parameters and the routine's settings
/// (GTK's visibility rules, as [`routine_settings`] draws them).
fn shown_fields(kind: RoutineKind) -> Vec<&'static str> {
    let mut fields = vec![SAFE_HEIGHT, FAST_FEED, SLOW_FEED, BACKOFF, MAX_DEPTH];
    let sideways = matches!(
        kind,
        RoutineKind::EdgeFind
            | RoutineKind::CornerFind
            | RoutineKind::BoreCenter
            | RoutineKind::BossCenter
    );
    if matches!(kind, RoutineKind::EdgeFind | RoutineKind::CornerFind) {
        fields.push(EDGE_DISTANCE);
    }
    if matches!(kind, RoutineKind::BoreCenter | RoutineKind::BossCenter) {
        fields.push(DIAMETER);
    }
    if kind == RoutineKind::ToolLength {
        fields.extend([SETTER_X, SETTER_Y, SETTER_Z]);
    }
    if sideways {
        fields.extend([PROBE_DEPTH, TIP_RADIUS]);
    }
    if matches!(kind, RoutineKind::ZTouch | RoutineKind::EdgeFind) {
        fields.push(PLATE_THICKNESS);
    }
    fields
}
