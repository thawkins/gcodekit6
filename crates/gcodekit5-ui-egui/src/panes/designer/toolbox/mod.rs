//! The Designer toolbox: the tool buttons, the grid and snap switches, the machine mode, and
//! the tool and stock settings the toolpaths are made with.
//!
//! Ported from the GTK `designer_toolbox.rs`. The buttons drive the canvas's
//! [`DesignerTool`] (the caller applies the returned tool with `DesignerPane::set_tool`);
//! the machine mode and settings write straight to the design's `DesignerState`, as in GTK.
//! The settings follow the display units of the Settings window ([`settings`]).
//!
//! The "Shape Gallery…", "Generate G-Code" and "Frame" buttons are reported in
//! [`ToolboxOutput`]; the pane acts on them.
//!
//! Not ported: the icons on the tool buttons (names only) and the stock dialog's second Step
//! Down field (it is the tool settings' Step Down). GTK's tooltips named shortcut keys (S, R, C ...) that no GTK
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
        n_!("Show Toolpaths"),
        n_!("Machine Mode"),
        n_!("Laser 2D"),
        n_!("CNC 3D"),
        POLYLINE_HINT,
    ]);
    ids.extend(settings::msgids());
    ids
}

/// What the user asked for in the toolbox this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToolboxOutput {
    /// The tool the user picked, if any.
    pub tool: Option<DesignerTool>,
    /// "Shape Gallery…" was clicked.
    pub gallery: bool,
    /// "Generate G-Code" was clicked.
    pub generate: bool,
    /// "Frame" was clicked.
    pub frame: bool,
}

/// The toolbox, top to bottom: tools, the shape gallery, switches, machine mode with
/// Generate G-Code and Frame, settings.
///
/// # Arguments
/// * `ui` - The toolbox panel's `Ui`.
/// * `state` - The design, whose grid, snap, machine mode and settings are edited.
/// * `tool` - The current tool.
/// * `units` - The display units of the settings.
/// * `tr` - Translator for the current UI language.
///
/// # Returns
/// What the user asked for this frame.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut DesignerState,
    tool: DesignerTool,
    units: MeasurementSystem,
    tr: &Translator,
) -> ToolboxOutput {
    let mut out = ToolboxOutput::default();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            out.tool = tool_buttons(ui, tool, tr);
            ui.label(format!(
                "{} {}",
                tr.tr("Active tool:"),
                tr.tr(tool.label_msgid())
            ));
            if tool == DesignerTool::Polyline {
                ui.add(egui::Label::new(egui::RichText::new(tr.tr(POLYLINE_HINT)).weak()).wrap());
            }
            ui.label(egui::RichText::new(tr.tr("Fast Shapes")).small());
            out.gallery = ui.button(tr.tr("Shape Gallery…")).clicked();
            ui.separator();
            crate::theme::checkbox(ui, &mut state.show_grid, tr.tr("Show Grid"));
            crate::theme::checkbox(ui, &mut state.snap_enabled, tr.tr("Snap to grid"));
            crate::theme::checkbox(ui, &mut state.show_toolpaths, tr.tr("Show Toolpaths"));
            ui.separator();
            machine_mode(ui, state, tr);
            ui.horizontal(|ui| {
                out.generate = ui.button(tr.tr("Generate G-Code")).clicked();
                out.frame = ui
                    .button(tr.tr("Frame ").trim())
                    .on_hover_text(tr.tr("Generate low-power frame to position material"))
                    .clicked();
            });
            settings::show(ui, state, units, tr);
        });
    out
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
