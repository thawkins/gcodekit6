//! The toolbox's tool settings (feed, spindle or laser power, tool diameter, step down,
//! continuous Z) and stock settings (width, height, thickness, safe Z).
//!
//! Values are millimetres in the design and are shown in the display units: inches in
//! imperial mode, except a laser's passes and width (shown unconverted) and the
//! spindle speed (RPM, or a laser's power in percent: 10 units of the model per percent).
//! The labels follow the machine mode ("Feed Rate" or "Laser Speed", and so on).
//!
//! Dependencies: `egui`, `gcodekit6-designer` (design state, stock), `gcodekit6-core`
//! (units).

use gcodekit6_app::i18n::Translator;
use gcodekit6_core::units::{MM_PER_INCH, MeasurementSystem};
use gcodekit6_designer::DesignerState;
use gcodekit6_designer::designer_state::MachineMode;
use gcodekit6_designer::stock_removal::StockMaterial;

/// Largest value a field accepts.
const MAX_VALUE: f64 = 1.0e6;

/// What a field measures; it decides the unit conversion and the suffix.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Spindle RPM, or a laser's power as a percentage.
    Rpm,
    /// A length (mm or inches); a laser's lengths are shown as they are.
    Length,
    /// A feed rate (mm/min or in/min).
    FeedRate,
}

/// The msgids the settings show, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Tool Settings"),
        n_!("Stock Settings"),
        n_!("Feed Rate"),
        n_!("Laser Speed"),
        n_!("Spindle Speed"),
        n_!("Laser Power"),
        n_!("Tool Dia"),
        n_!("Laser Width"),
        n_!("Step Down"),
        n_!("Passes"),
        n_!("Continuous Z"),
        n_!("Keep tool at cutting depth between passes when XY start is unchanged"),
        n_!("Stock Width"),
        n_!("Stock Height"),
        n_!("Stock Thickness"),
        n_!("Safe Z Height"),
        n_!("Depth per pass"),
        n_!("Safe height above stock for rapid moves"),
    ]
}

/// The value shown for `mm` (the model's value), and the suffix after it.
///
/// # Arguments
/// * `kind` - What the field measures.
/// * `mode` - The machine mode.
/// * `units` - The display units.
/// * `mm` - The model's value.
pub fn to_display(
    kind: Kind,
    mode: MachineMode,
    units: MeasurementSystem,
    mm: f64,
) -> (f64, &'static str) {
    let imperial = units == MeasurementSystem::Imperial;
    match (kind, mode) {
        (Kind::Rpm, MachineMode::Laser2D) => ((mm / 10.0).clamp(0.0, 100.0), "%"),
        (Kind::Rpm, MachineMode::Cnc3D) => (mm, "RPM"),
        (Kind::Length, MachineMode::Laser2D) => (mm, ""),
        (Kind::Length, MachineMode::Cnc3D) if imperial => (mm / MM_PER_INCH, "in"),
        (Kind::Length, MachineMode::Cnc3D) => (mm, "mm"),
        (Kind::FeedRate, _) if imperial => (mm / MM_PER_INCH, "in/min"),
        (Kind::FeedRate, _) => (mm, "mm/min"),
    }
}

/// The inverse of [`to_display`]: the model's value for the `shown` one.
///
/// # Arguments
/// * `kind` - What the field measures.
/// * `mode` - The machine mode.
/// * `units` - The display units.
/// * `shown` - The value in the display units.
pub fn from_display(kind: Kind, mode: MachineMode, units: MeasurementSystem, shown: f64) -> f64 {
    let imperial = units == MeasurementSystem::Imperial;
    match (kind, mode) {
        (Kind::Rpm, MachineMode::Laser2D) => shown * 10.0,
        (Kind::Rpm, MachineMode::Cnc3D) | (Kind::Length, MachineMode::Laser2D) => shown,
        (Kind::Length | Kind::FeedRate, _) if imperial => shown * MM_PER_INCH,
        (Kind::Length | Kind::FeedRate, _) => shown,
    }
}

/// One labelled field. Returns the model's new value when the user changed it.
fn field(
    ui: &mut egui::Ui,
    label: &str,
    hint: &str,
    kind: Kind,
    ctx: (MachineMode, MeasurementSystem),
    mm: f64,
) -> Option<f64> {
    let (mode, units) = ctx;
    let (mut shown, suffix) = to_display(kind, mode, units, mm);
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(format!("{label}:"));
        changed = ui
            .add(
                egui::DragValue::new(&mut shown)
                    .range(0.0..=MAX_VALUE)
                    .max_decimals(3)
                    .speed(0.1)
                    .suffix(if suffix.is_empty() {
                        String::new()
                    } else {
                        format!(" {suffix}")
                    }),
            )
            .on_hover_text(hint)
            .changed();
    });
    changed.then(|| from_display(kind, mode, units, shown))
}

/// The collapsible Tool Settings and Stock Settings sections.
///
/// # Arguments
/// * `ui` - The toolbox's `Ui`.
/// * `state` - The design, whose settings are edited.
/// * `units` - The display units.
/// * `tr` - Translator for the current UI language.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut DesignerState,
    units: MeasurementSystem,
    tr: &Translator,
) {
    egui::CollapsingHeader::new(tr.tr("Tool Settings"))
        .default_open(true)
        .show(ui, |ui| tool_settings(ui, state, units, tr));
    egui::CollapsingHeader::new(tr.tr("Stock Settings"))
        .default_open(false)
        .show(ui, |ui| stock_settings(ui, state, units, tr));
}

/// Feed, speed, tool diameter, step down and continuous Z.
fn tool_settings(
    ui: &mut egui::Ui,
    state: &mut DesignerState,
    units: MeasurementSystem,
    tr: &Translator,
) {
    let mode = state.machine_mode();
    let laser = mode == MachineMode::Laser2D;
    let ctx = (mode, units);
    let pick =
        |laser_id: &'static str, cnc_id: &'static str| tr.tr(if laser { laser_id } else { cnc_id });
    let feed = state.tool_settings.feed_rate;
    if let Some(v) = field(
        ui,
        &pick(n_!("Laser Speed"), n_!("Feed Rate")),
        &tr.tr("Feed Rate"),
        Kind::FeedRate,
        ctx,
        feed,
    ) {
        state.set_feed_rate(v);
    }
    let speed = f64::from(state.tool_settings.spindle_speed);
    if let Some(v) = field(
        ui,
        &pick(n_!("Laser Power"), n_!("Spindle Speed")),
        &tr.tr("Spindle Speed"),
        Kind::Rpm,
        ctx,
        speed,
    ) {
        // Rounded and clamped into u32 range by the field's own range.
        state.set_spindle_speed(v.round() as u32);
    }
    let dia = state.tool_settings.tool_diameter;
    if let Some(v) = field(
        ui,
        &pick(n_!("Laser Width"), n_!("Tool Dia")),
        &tr.tr("Tool Dia"),
        Kind::Length,
        ctx,
        dia,
    ) {
        state.set_tool_diameter(v);
    }
    let step = state.tool_settings.step_down;
    let step = if laser && step <= 0.0 { 1.0 } else { step };
    if let Some(v) = field(
        ui,
        &pick(n_!("Passes"), n_!("Step Down")),
        &tr.tr("Depth per pass"),
        Kind::Length,
        ctx,
        step,
    ) {
        state.set_step_down(v);
    }
    if !laser {
        let mut on = state.tool_settings.continuous_z_between_passes;
        if crate::theme::checkbox(ui, &mut on, tr.tr("Continuous Z"))
            .on_hover_text(
                tr.tr("Keep tool at cutting depth between passes when XY start is unchanged"),
            )
            .changed()
        {
            state.set_continuous_z_between_passes(on);
        }
    }
}

/// Stock width, height, thickness, and the safe Z clearance above the stock.
fn stock_settings(
    ui: &mut egui::Ui,
    state: &mut DesignerState,
    units: MeasurementSystem,
    tr: &Translator,
) {
    let ctx = (MachineMode::Cnc3D, units);
    let Some(stock) = state.stock_material.as_mut() else {
        return;
    };
    if let Some(v) = field(
        ui,
        &tr.tr("Stock Width"),
        &tr.tr("Stock Width"),
        Kind::Length,
        ctx,
        stock.width,
    ) {
        stock.width = v;
    }
    if let Some(v) = field(
        ui,
        &tr.tr("Stock Height"),
        &tr.tr("Stock Height"),
        Kind::Length,
        ctx,
        stock.height,
    ) {
        stock.height = v;
    }
    // The safe Z follows the thickness: the clearance above the stock is kept.
    let clearance = (stock.safe_z - stock.thickness).max(StockMaterial::MIN_SAFE_Z_ABOVE_STOCK_MM);
    if let Some(v) = field(
        ui,
        &tr.tr("Stock Thickness"),
        &tr.tr("Stock Thickness"),
        Kind::Length,
        ctx,
        stock.thickness,
    ) {
        stock.thickness = v;
        stock.safe_z = stock.thickness + clearance;
    }
    if let Some(v) = field(
        ui,
        &tr.tr("Safe Z Height"),
        &tr.tr("Safe height above stock for rapid moves"),
        Kind::Length,
        ctx,
        clearance,
    ) {
        stock.safe_z = stock.thickness + v.max(StockMaterial::MIN_SAFE_Z_ABOVE_STOCK_MM);
    }
}
