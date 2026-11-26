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
//! Laid out as GTK's sidebar: a 3-column grid of square icon buttons (the app SVGs; each
//! button's tooltip and accessible name is the tool's name), Fast Shapes, the Machine Mode
//! group, the tool and stock settings group, the Generate G-Code / Frame group, and the
//! View Controls (grid, grid spacing, toolpaths, snap and snap step).
//!
//! Not ported: the stock dialog's second Step Down field (it is the tool settings' Step
//! Down). GTK's tool and stock settings are buttons opening dialogs; here they are
//! collapsible sections in the settings group. GTK's tooltips named shortcut keys (S, R,
//! C ...) that no GTK code handled, so none are shown.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (design state), `gcodekit5-core` (units),
//! `gcodekit5-app` (translations).

pub mod settings;

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::{self, MeasurementSystem};
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
        n_!("View Controls"),
        n_!("Grid spacing"),
        n_!("Snap threshold"),
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

/// The tools in GTK's grid order (three per row).
const GRID_ORDER: [DesignerTool; 12] = [
    DesignerTool::Select,
    DesignerTool::Pan,
    DesignerTool::Rectangle,
    DesignerTool::Circle,
    DesignerTool::Line,
    DesignerTool::Ellipse,
    DesignerTool::Triangle,
    DesignerTool::Polygon,
    DesignerTool::Polyline,
    DesignerTool::Text,
    DesignerTool::Gear,
    DesignerTool::Sprocket,
];

/// Edge length of a tool button in points (GTK's 40 px).
const TOOL_BUTTON: f32 = 40.0;

/// The grid spacings offered, in millimetres (GTK's list).
const GRID_SPACINGS: [f64; 5] = [1.0, 5.0, 10.0, 25.0, 50.0];

/// Height of the toolbox's full-width buttons in points.
const WIDE_BUTTON: f32 = 28.0;

/// The toolbox, top to bottom: the tool grid, Fast Shapes, the machine mode, the settings,
/// Generate G-Code and Frame, and the view controls.
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
            ui.spacing_mut().item_spacing.y = 6.0;
            out.tool = tool_buttons(ui, tool, tr);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{} {}",
                        tr.tr("Active tool:"),
                        tr.tr(tool.label_msgid())
                    ))
                    .small(),
                );
            });
            if tool == DesignerTool::Polyline {
                ui.add(egui::Label::new(egui::RichText::new(tr.tr(POLYLINE_HINT)).weak()).wrap());
            }
            ui.label(egui::RichText::new(tr.tr("Fast Shapes")).small());
            out.gallery = wide_button(ui, egui::Button::new(tr.tr("Shape Gallery…"))).clicked();
            group(ui, |ui| machine_mode(ui, state, tr));
            group(ui, |ui| settings::show(ui, state, units, tr));
            group(ui, |ui| {
                out.generate =
                    wide_button(ui, egui::Button::new(tr.tr("Generate G-Code"))).clicked();
                let frame = egui::Button::image_and_text(
                    crate::icons::Icon::Fit.themed(ui),
                    tr.tr("Frame ").trim(),
                )
                .frame(false);
                out.frame = wide_button(ui, frame)
                    .on_hover_text(tr.tr("Generate low-power frame to position material"))
                    .clicked();
            });
            egui::CollapsingHeader::new(tr.tr("View Controls"))
                .default_open(true)
                .show(ui, |ui| view_controls(ui, state, units, tr));
        });
    out
}

/// A framed group as wide as the toolbox.
fn group(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    crate::theme::group_frame(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
}

/// `button` across the whole width of `ui`.
fn wide_button(ui: &mut egui::Ui, button: egui::Button<'_>) -> egui::Response {
    ui.add_sized([ui.available_width(), WIDE_BUTTON], button)
}

/// The tool grid: three square icon buttons per row, centred; the current tool is shown
/// selected.
fn tool_buttons(ui: &mut egui::Ui, current: DesignerTool, tr: &Translator) -> Option<DesignerTool> {
    let mut picked = None;
    let gap = 4.0;
    let width = 3.0 * TOOL_BUTTON + 2.0 * gap;
    let indent = ((ui.available_width() - width) * 0.5).max(0.0);
    ui.horizontal(|ui| {
        ui.add_space(indent);
        egui::Grid::new("gcodekit5-designer-tools")
            .spacing([gap, gap])
            .show(ui, |ui| {
                for (i, tool) in GRID_ORDER.into_iter().enumerate() {
                    let name = tr.tr(tool.label_msgid());
                    let response = crate::icons::app_icon_button(
                        ui,
                        tool.icon(),
                        TOOL_BUTTON,
                        current == tool,
                        &name,
                    );
                    if response.clicked() {
                        picked = Some(tool);
                    }
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
    });
    picked
}

/// Show Grid, the grid spacing, Show Toolpaths, then the snap switch and its step.
fn view_controls(
    ui: &mut egui::Ui,
    state: &mut DesignerState,
    system: MeasurementSystem,
    tr: &Translator,
) {
    crate::theme::checkbox(ui, &mut state.show_grid, tr.tr("Show Grid"));
    let length = |mm: f64| {
        let shown = match system {
            MeasurementSystem::Metric => mm,
            MeasurementSystem::Imperial => mm / 25.4,
        };
        format!("{shown:.2} {}", units::get_unit_label(system))
    };
    ui.horizontal(|ui| {
        let name = tr.tr("Grid spacing");
        ui.label(name.as_ref());
        let response = egui::ComboBox::from_id_salt("gcodekit5-designer-grid-spacing")
            .selected_text(length(state.grid_spacing_mm))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for mm in GRID_SPACINGS {
                    ui.selectable_value(&mut state.grid_spacing_mm, mm, length(mm));
                }
            })
            .response;
        let enabled = response.enabled();
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, enabled, &name));
    });
    crate::theme::checkbox(ui, &mut state.show_toolpaths, tr.tr("Show Toolpaths"));
    ui.separator();
    crate::theme::checkbox(ui, &mut state.snap_enabled, tr.tr("Snap to grid"));
    ui.horizontal(|ui| {
        ui.label(tr.tr("Snap threshold"));
        let scale = match system {
            MeasurementSystem::Metric => 1.0,
            MeasurementSystem::Imperial => 25.4,
        };
        let mut shown = state.snap_threshold_mm / scale;
        if ui
            .add(
                egui::DragValue::new(&mut shown)
                    .range(0.0..=10.0)
                    .speed(0.1)
                    .max_decimals(2),
            )
            .on_hover_text(tr.tr("Snap threshold"))
            .changed()
        {
            state.snap_threshold_mm = (shown * scale).max(0.0);
        }
    });
}

/// The Laser 2D / CNC 3D radio buttons.
fn machine_mode(ui: &mut egui::Ui, state: &mut DesignerState, tr: &Translator) {
    crate::theme::section_heading(ui, &tr.tr("Machine Mode"));
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
