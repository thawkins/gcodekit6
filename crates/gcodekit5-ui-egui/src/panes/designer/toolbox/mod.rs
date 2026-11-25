//! The Designer toolbox: the tool buttons, the grid and snap switches, the machine mode, and
//! the tool and stock settings the toolpaths are made with.
//!
//! Ported from the GTK `designer_toolbox.rs`. The buttons drive the canvas's
//! [`DesignerTool`] (the caller applies the returned tool with `DesignerPane::set_tool`);
//! the machine mode and settings write straight to the design's `DesignerState`, as in GTK.
//! The settings follow the display units of the Settings window ([`settings`]).
//!
//! Not ported: the icons on the tool buttons (names only), the Text tool (with the
//! properties pane), the Shape Gallery button, Generate G-Code and Frame (the toolpath
//! preview and file operations), and the stock dialog's second Step Down field (it is the
//! tool settings' Step Down). GTK's tooltips named shortcut keys (S, R, C ...) that no GTK
//! code handled, so none are shown.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (design state), `gcodekit5-core` (units),
//! `gcodekit5-app` (translations).

pub mod settings;

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_designer::DesignerState;
use gcodekit5_designer::designer_state::MachineMode;

use super::canvas::DesignerTool;

/// The msgid of the Polyline tool's usage hint.
const POLYLINE_HINT: &str = n_!(
    "Click to add points. Enter finishes an open polyline; double-click or right-click finishes a closed shape. Esc cancels."
);

/// The msgids the toolbox shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = DesignerTool::ALL
        .into_iter()
        .map(DesignerTool::label_msgid)
        .collect();
    ids.extend([
        n_!("Active tool:"),
        n_!("Show Grid"),
        n_!("Snap to grid"),
        n_!("Machine Mode"),
        n_!("Laser 2D"),
        n_!("CNC 3D"),
        POLYLINE_HINT,
    ]);
    ids.extend(settings::msgids());
    ids
}

/// The toolbox, top to bottom: tools, switches, machine mode, settings.
///
/// # Arguments
/// * `ui` - The toolbox panel's `Ui`.
/// * `state` - The design, whose grid, snap, machine mode and settings are edited.
/// * `tool` - The current tool.
/// * `units` - The display units of the settings.
/// * `tr` - Translator for the current UI language.
///
/// # Returns
/// The tool the user picked this frame, if any.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut DesignerState,
    tool: DesignerTool,
    units: MeasurementSystem,
    tr: &Translator,
) -> Option<DesignerTool> {
    let mut picked = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            picked = tool_buttons(ui, tool, tr);
            ui.label(format!(
                "{} {}",
                tr.tr("Active tool:"),
                tr.tr(tool.label_msgid())
            ));
            if tool == DesignerTool::Polyline {
                ui.add(egui::Label::new(egui::RichText::new(tr.tr(POLYLINE_HINT)).weak()).wrap());
            }
            ui.separator();
            ui.checkbox(&mut state.show_grid, tr.tr("Show Grid"));
            ui.checkbox(&mut state.snap_enabled, tr.tr("Snap to grid"));
            ui.separator();
            machine_mode(ui, state, tr);
            settings::show(ui, state, units, tr);
        });
    picked
}

/// One button per tool, in tool order; the current tool is shown selected.
fn tool_buttons(ui: &mut egui::Ui, current: DesignerTool, tr: &Translator) -> Option<DesignerTool> {
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        for tool in DesignerTool::ALL {
            if ui
                .selectable_label(current == tool, tr.tr(tool.label_msgid()))
                .clicked()
            {
                picked = Some(tool);
            }
        }
    });
    picked
}

/// The Laser 2D / CNC 3D radio buttons.
fn machine_mode(ui: &mut egui::Ui, state: &mut DesignerState, tr: &Translator) {
    ui.label(tr.tr("Machine Mode"));
    let mode = state.machine_mode();
    ui.horizontal(|ui| {
        for (value, msgid) in [
            (MachineMode::Laser2D, n_!("Laser 2D")),
            (MachineMode::Cnc3D, n_!("CNC 3D")),
        ] {
            if ui.radio(mode == value, tr.tr(msgid)).clicked() {
                state.set_machine_mode(value);
            }
        }
    });
}
