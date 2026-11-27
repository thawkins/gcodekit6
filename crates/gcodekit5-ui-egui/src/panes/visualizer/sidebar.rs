//! The visualizer's left sidebar: a toolbar (Fit to Content,
//! Reset View, Fit to Device Working Area and Hide), the 2D / 3D view mode pair, and
//! collapsible sections: Toolpath
//! (Show Rapid Moves, Show Cutting Moves, Show Laser/Spindle), Guides (Show Grid, Grid
//! spacing, Show Machine Bounds), Simulation and the
//! Inspector with the program's bounds. The toggles are [`Display`], which the pane hands to
//! the renderer; they start on, with a 50 mm grid.
//!
//! Reset View (the `view-restore` icon)
//! is also run by `Cmd+0`. The Inspector has the Bounds X/Y and the S-value
//! statistics (Min S, Max S, Avg S with one decimal, N/A when no move sets a power).
//!
//! The Simulation section ("Show Stock Removal", and the stock, tool and zero references
//! while it is checked) is shown only while the `enable_stock_removal_3d` setting is on.
//! The stock fields start from the Designer's stock and are
//! edited in place.
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations), `gcodekit5-core` (units),
//! `gcodekit5-visualizer` (stock references).

use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::{MeasurementSystem, get_unit_label};

use gcodekit5_visualizer::visualizer::{StockXyReference, StockZeroReference};

use super::camera::Aabb;
use super::stock::StockSetup;
use super::toolpath::Intensity;
use crate::icons::{Icon, icon_only_button};
use crate::panes::machine_control::format_length;
use crate::workspace::named_icon_button;

/// The sidebar's default width in points.
pub const WIDTH: f32 = 250.0;
/// The sidebar's least width in points.
pub const MIN_WIDTH: f32 = 200.0;

/// The msgids the sidebar shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Fit to Content"),
        n_!("Toolpath"),
        n_!("Show Rapid Moves"),
        n_!("Show Cutting Moves"),
        n_!("Show Laser/Spindle"),
        n_!("Guides"),
        n_!("Show Grid"),
        n_!("Grid spacing"),
        n_!("Show Machine Bounds"),
        n_!("2D"),
        n_!("Top-down 2D view"),
        n_!("3D"),
        n_!("Rotatable 3D view"),
        n_!("Fit to Device Working Area"),
        n_!("Reset View"),
        n_!("Hide"),
        n_!("Hide Sidebar"),
        n_!("Show Sidebar"),
        n_!("Inspector"),
        n_!("Bounds"),
        n_!("Min S:"),
        n_!("Max S:"),
        n_!("Avg S:"),
        n_!("N/A"),
        n_!("to"),
        n_!("Simulation"),
        n_!("Show Stock Removal"),
        n_!("The simulation has a resolution of "),
        n_!("mm. Cuts smaller than "),
        n_!("mm will not be visually displayed, but the G-code will execute correctly."),
        n_!("Stock"),
        n_!("Width"),
        n_!("Height"),
        n_!("Thickness"),
        n_!("Tool Diameter"),
        n_!("Stock Z0 at:"),
        n_!("Top"),
        n_!("Bottom"),
        n_!("Stock XY0 at:"),
        n_!("Lower left"),
        n_!("Center"),
        n_!("Simulating stock removal\u{2026}"),
        n_!("Cancel"),
    ]
}

/// The grid spacings offered, in mm.
pub const GRID_SPACINGS: [f32; 5] = [1.0, 5.0, 10.0, 25.0, 50.0];

/// What the view draws: the sidebar's Toolpath and Guides toggles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    /// Show Rapid Moves: travel (rapids and moves with the spindle/laser off).
    pub rapids: bool,
    /// Show Cutting Moves.
    pub cuts: bool,
    /// Show Laser/Spindle: the tool marker.
    pub marker: bool,
    /// Show Grid.
    pub grid: bool,
    /// Grid spacing in mm, one of [`GRID_SPACINGS`].
    pub grid_spacing: f32,
    /// Show Machine Bounds: the active device's work area.
    pub bounds: bool,
}

impl Default for Display {
    /// The start: everything shown, a 50 mm grid.
    fn default() -> Self {
        Self {
            rapids: true,
            cuts: true,
            marker: true,
            grid: true,
            grid_spacing: super::pipeline::DEFAULT_GRID_SPACING,
            bounds: true,
        }
    }
}

/// The Toolpath and Guides sections.
fn display(ui: &mut egui::Ui, display: &mut Display, units: MeasurementSystem, tr: &Translator) {
    egui::CollapsingHeader::new(tr.tr("Toolpath"))
        .id_salt("visualizer-toolpath")
        .default_open(true)
        .show(ui, |ui| {
            crate::theme::checkbox(ui, &mut display.rapids, tr.tr("Show Rapid Moves"));
            crate::theme::checkbox(ui, &mut display.cuts, tr.tr("Show Cutting Moves"));
            crate::theme::checkbox(ui, &mut display.marker, tr.tr("Show Laser/Spindle"));
        });
    egui::CollapsingHeader::new(tr.tr("Guides"))
        .id_salt("visualizer-guides")
        .default_open(true)
        .show(ui, |ui| {
            crate::theme::checkbox(ui, &mut display.grid, tr.tr("Show Grid"));
            let spacing =
                |mm: f32| format!("{} {}", format_length(mm, units), get_unit_label(units));
            ui.horizontal(|ui| {
                let label = ui.small(tr.tr("Grid spacing"));
                egui::ComboBox::from_id_salt("visualizer-grid-spacing")
                    .selected_text(spacing(display.grid_spacing))
                    .show_ui(ui, |ui| {
                        for mm in GRID_SPACINGS {
                            ui.selectable_value(&mut display.grid_spacing, mm, spacing(mm));
                        }
                    })
                    .response
                    .on_hover_text(tr.tr("Grid spacing"))
                    .labelled_by(label.id);
            });
            crate::theme::checkbox(ui, &mut display.bounds, tr.tr("Show Machine Bounds"));
        });
}

/// The Simulation section's state, which [`show`] edits in place.
#[derive(Debug)]
pub struct Simulation<'a> {
    /// Whether "Show Stock Removal" is checked.
    pub show: &'a mut bool,
    /// The stock and tool.
    pub setup: &'a mut StockSetup,
    /// The running simulation's progress in percent.
    pub progress: Option<u8>,
    /// Why no stock is shown, translated.
    pub notice: Option<String>,
}

/// A labelled length field: `value` is kept in mm and shown in `units` with 2 decimals.
fn length_row(ui: &mut egui::Ui, label: &str, value: &mut f32, units: MeasurementSystem) {
    let per_unit = match units {
        MeasurementSystem::Metric => 1.0,
        MeasurementSystem::Imperial => 25.4,
    };
    let mut shown = *value / per_unit;
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let changed = ui
                .add(
                    egui::DragValue::new(&mut shown)
                        .range(0.0..=10_000.0 / per_unit)
                        .speed(0.1 / per_unit)
                        .fixed_decimals(2)
                        .suffix(format!(" {}", get_unit_label(units))),
                )
                .on_hover_text(label)
                .changed();
            // Written back only when edited, so showing a value never rounds the stored mm.
            if changed {
                *value = shown * per_unit;
            }
        });
    });
}

/// A labelled choice of two values.
fn choice_row<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    value: &mut T,
    options: [(T, String); 2],
) {
    ui.horizontal(|ui| {
        let label = ui.label(label);
        let selected = options
            .iter()
            .find(|(v, _)| *v == *value)
            .map_or_else(String::new, |(_, text)| text.clone());
        egui::ComboBox::from_id_salt(id)
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for (v, text) in options {
                    ui.selectable_value(value, v, text);
                }
            })
            .response
            .labelled_by(label.id);
    });
}

/// The tooltip on "Show Stock Removal": the simulation's resolution and the smallest cut
/// it shows, in mm with two decimals.
///
/// # Arguments
/// * `tr` - Translator for the current UI language.
pub fn resolution_tooltip(tr: &Translator) -> String {
    let resolution = super::stock::RESOLUTION;
    format!(
        "{}{resolution:.2}{}{:.2}{}",
        tr.tr("The simulation has a resolution of "),
        tr.tr("mm. Cuts smaller than "),
        resolution + 0.01,
        tr.tr("mm will not be visually displayed, but the G-code will execute correctly."),
    )
}

/// The Simulation section.
fn simulation(ui: &mut egui::Ui, sim: Simulation<'_>, units: MeasurementSystem, tr: &Translator) {
    egui::CollapsingHeader::new(tr.tr("Simulation"))
        .id_salt("visualizer-simulation")
        .default_open(true)
        .show(ui, |ui| {
            crate::theme::checkbox(ui, sim.show, tr.tr("Show Stock Removal"))
                .on_hover_text(resolution_tooltip(tr));
            if !*sim.show {
                return;
            }
            crate::theme::group_frame(ui).show(ui, |ui| {
                crate::theme::section_heading(ui, &tr.tr("Stock"));
                let setup = sim.setup;
                length_row(ui, &tr.tr("Width"), &mut setup.width, units);
                length_row(ui, &tr.tr("Height"), &mut setup.height, units);
                length_row(ui, &tr.tr("Thickness"), &mut setup.thickness, units);
                length_row(ui, &tr.tr("Tool Diameter"), &mut setup.tool_diameter, units);
                choice_row(
                    ui,
                    "visualizer-stock-z0",
                    &tr.tr("Stock Z0 at:"),
                    &mut setup.zero,
                    [
                        (StockZeroReference::Top, tr.tr("Top").into_owned()),
                        (StockZeroReference::Bottom, tr.tr("Bottom").into_owned()),
                    ],
                );
                choice_row(
                    ui,
                    "visualizer-stock-xy0",
                    &tr.tr("Stock XY0 at:"),
                    &mut setup.xy,
                    [
                        (
                            StockXyReference::LowerLeft,
                            tr.tr("Lower left").into_owned(),
                        ),
                        (StockXyReference::Centre, tr.tr("Center").into_owned()),
                    ],
                );
            });
            if let Some(pct) = sim.progress {
                ui.add_space(4.0);
                ui.label(tr.tr("Simulating stock removal\u{2026}"));
                ui.horizontal(|ui| {
                    ui.add(
                        egui::ProgressBar::new(f32::from(pct) / 100.0)
                            .show_percentage()
                            .desired_width(ui.available_width() - 70.0),
                    );
                    if ui.button(tr.tr("Cancel")).clicked() {
                        *sim.show = false;
                    }
                });
            }
            if let Some(notice) = sim.notice {
                ui.add_space(4.0);
                ui.colored_label(ui.visuals().warn_fg_color, notice);
            }
        });
}

/// What a sidebar button asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarAction {
    /// Fit the toolpath into the view.
    Fit,
    /// Reset the camera and fit the toolpath.
    ResetView,
    /// Fit the active device's work area into the view.
    FitDevice,
    /// Switch to the 2D view (`true`) or the 3D view (`false`).
    ViewMode(bool),
    /// Hide the sidebar.
    Hide,
}

/// The inspector's bounds value: `0.00 to 40.00 mm`.
fn range(min: f32, max: f32, units: MeasurementSystem, tr: &Translator) -> String {
    format!(
        "{} {} {} {}",
        format_length(min, units),
        tr.tr("to"),
        format_length(max, units),
        get_unit_label(units)
    )
}

/// What the Inspector describes.
#[derive(Debug, Clone, Copy, Default)]
pub struct Inspector<'a> {
    /// The program's bounds, once built.
    pub bounds: Option<&'a Aabb>,
    /// The program's S-value statistics, once built.
    pub intensity: Option<Intensity>,
}

/// A row of the Inspector: `key` at the left, `value` at the right.
fn inspector_row(ui: &mut egui::Ui, key: &str, value: String) {
    ui.horizontal(|ui| {
        ui.small(key);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.monospace(value)
        });
    });
}

/// Draws the sidebar's contents.
///
/// # Arguments
/// * `ui` - The sidebar panel's `Ui`.
/// * `inspector` - What the Inspector describes.
/// * `view_2d` - Whether the 2D view mode is on.
/// * `display_state` - The Toolpath and Guides toggles, edited in place.
/// * `units` - The measurement system to show.
/// * `simulation` - The Simulation section's state, or `None` to leave the section out (the
///   `enable_stock_removal_3d` setting is off).
/// * `tr` - Translator for the current UI language.
///
/// # Returns
/// The action of the button clicked this frame, if any.
pub fn show(
    ui: &mut egui::Ui,
    inspector: Inspector<'_>,
    view_2d: bool,
    display_state: &mut Display,
    units: MeasurementSystem,
    simulation: Option<Simulation<'_>>,
    tr: &Translator,
) -> Option<SidebarAction> {
    let mut action = None;
    ui.add_space(f32::from(crate::theme::PAGE_MARGIN) / 2.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(8.0, 8.0);
        if icon_only_button(ui, Icon::Fit, &tr.tr("Fit to Content")).clicked() {
            action = Some(SidebarAction::Fit);
        }
        if icon_only_button(ui, Icon::Restore, &tr.tr("Reset View")).clicked() {
            action = Some(SidebarAction::ResetView);
        }
        if icon_only_button(ui, Icon::Display, &tr.tr("Fit to Device Working Area")).clicked() {
            action = Some(SidebarAction::FitDevice);
        }
        if named_icon_button(ui, Icon::Hide, &tr.tr("Hide"), &tr.tr("Hide Sidebar")) {
            action = Some(SidebarAction::Hide);
        }
    });
    ui.horizontal(|ui| {
        let modes = [
            (true, "2D", "Top-down 2D view"),
            (false, "3D", "Rotatable 3D view"),
        ];
        for (mode, label, tooltip) in modes {
            let on = view_2d == mode;
            let clicked = crate::theme::checked_scope(ui, on, |ui| ui.radio(on, tr.tr(label)))
                .on_hover_text(tr.tr(tooltip))
                .clicked();
            if clicked && !on {
                action = Some(SidebarAction::ViewMode(mode));
            }
        }
    });
    ui.add_space(4.0);
    display(ui, display_state, units, tr);
    if let Some(sim) = simulation {
        self::simulation(ui, sim, units, tr);
    }
    egui::CollapsingHeader::new(tr.tr("Inspector"))
        .id_salt("visualizer-inspector")
        .default_open(false)
        .show(ui, |ui| {
            let label = tr.tr("Bounds");
            for (axis, name) in ["X", "Y"].into_iter().enumerate() {
                let value = inspector.bounds.map_or_else(
                    || "-".to_owned(),
                    |b| range(b.min[axis], b.max[axis], units, tr),
                );
                inspector_row(ui, &format!("{label} {name}"), value);
            }
            let stat = |pick: fn(&Intensity) -> f32| {
                inspector
                    .intensity
                    .map_or_else(|| tr.tr("N/A").into_owned(), |s| format!("{:.1}", pick(&s)))
            };
            inspector_row(ui, &tr.tr("Min S:"), stat(|s| s.min));
            inspector_row(ui, &tr.tr("Max S:"), stat(|s| s.max));
            inspector_row(ui, &tr.tr("Avg S:"), stat(|s| s.avg));
        });
    action
}

/// The floating Show Sidebar button at the top left of the view `rect` while the sidebar
/// is hidden (the `view-reveal` button).
///
/// # Returns
/// Whether it was clicked.
pub fn show_button(ui: &mut egui::Ui, rect: egui::Rect, tr: &Translator) -> bool {
    let margin = f32::from(crate::theme::PAGE_MARGIN);
    let strip = egui::Rect::from_min_max(
        rect.min + egui::vec2(margin, margin),
        egui::pos2(rect.center().x, rect.top() + margin + 36.0),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(strip), |ui| {
        let name = tr.tr("Show Sidebar");
        named_icon_button(ui, Icon::Show, &name, &name)
    })
    .inner
}
