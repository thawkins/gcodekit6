//! CAM Properties (CNC only): use the global values, operation (profile or pocket), step
//! down, step in, ramp angle, pocket strategy and raster fill.
//!
//! As in GTK, the fields are editable only when the shapes use their own values ("Use
//! global values" off); step in, strategy and raster fill only for a pocket. Several
//! selected shapes show the first one's values and every change applies to all of them.
//! Step down and step in are lengths in the display units; the raster fill is a percentage
//! (0 to 100) stored as a ratio.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model).

use gcodekit5_designer::pocket_operations::PocketStrategy;
use gcodekit5_designer::shapes::OperationType;

use super::super::builders::{
    self, FieldEvent, entry, length_text, length_unit, parse_length, parse_number, section,
};
use super::Ctx;

/// The msgid of the raster fill hint.
const RASTER_HINT: &str = n_!("0 = no raster, 100 = full length");

/// The msgids of this section.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("CAM Properties"),
        n_!("Use global values"),
        n_!("Operation:"),
        n_!("Profile"),
        n_!("Pocket"),
        n_!("Step Down:"),
        n_!("Step In:"),
        n_!("Ramp Angle:"),
        n_!("Strategy:"),
        n_!("Raster"),
        n_!("Offset"),
        n_!("Adaptive"),
        n_!("Raster Fill (%):"),
        RASTER_HINT,
    ]
}

/// The index of `strategy` in the Strategy drop-down.
fn strategy_index(strategy: PocketStrategy) -> usize {
    match strategy {
        PocketStrategy::Raster { .. } => 0,
        PocketStrategy::ContourParallel => 1,
        PocketStrategy::Adaptive => 2,
    }
}

/// The strategy at drop-down index `i` (GTK's raster is at 0 degrees, bidirectional).
fn strategy_at(i: usize) -> PocketStrategy {
    match i {
        0 => PocketStrategy::Raster {
            angle: 0.0,
            bidirectional: true,
        },
        2 => PocketStrategy::Adaptive,
        _ => PocketStrategy::ContourParallel,
    }
}

/// The CAM Properties section.
pub fn show(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let first = &cx.summary.first;
    let units = cx.units;
    let tr = cx.tr;
    let custom = first.use_custom_values;
    let pocket = first.operation_type == OperationType::Pocket;
    let (step_down, step_in, ramp) = (
        f64::from(first.step_down),
        f64::from(first.step_in),
        f64::from(first.ramp_angle),
    );
    let (strategy, fill, depth) = (
        first.pocket_strategy,
        first.raster_fill_ratio,
        first.pocket_depth,
    );
    section(ui, &tr.tr("CAM Properties"), |ui| {
        if let Some(global) = builders::check(ui, &tr.tr("Use global values"), !custom, true) {
            cx.state().set_selected_use_custom_values(!global);
        }
        let ops = [tr.tr("Profile").into_owned(), tr.tr("Pocket").into_owned()];
        if let Some(i) =
            builders::combo(ui, &tr.tr("Operation:"), &ops, usize::from(pocket), custom)
        {
            cx.state().set_selected_pocket_properties(i == 1, depth);
        }
        let unit = length_unit(units);
        if let FieldEvent::Committed(text) = entry(
            ui,
            &mut cx.fields(),
            "step_down",
            &tr.tr("Step Down:"),
            &length_text(step_down, units),
            unit,
            custom,
        ) && let Some(v) = parse_length(&text, units)
        {
            cx.state().set_selected_step_down(v);
        }
        if let FieldEvent::Committed(text) = entry(
            ui,
            &mut cx.fields(),
            "step_in",
            &tr.tr("Step In:"),
            &length_text(step_in, units),
            unit,
            custom && pocket,
        ) && let Some(v) = parse_length(&text, units)
        {
            cx.state().set_selected_step_in(v);
        }
        if let FieldEvent::Committed(text) = entry(
            ui,
            &mut cx.fields(),
            "ramp_angle",
            &tr.tr("Ramp Angle:"),
            &format!("{ramp:.1}"),
            "deg",
            custom,
        ) && let Some(v) = parse_number(&text)
        {
            cx.state().set_selected_ramp_angle(v);
        }
        let strategies = [
            tr.tr("Raster").into_owned(),
            tr.tr("Offset").into_owned(),
            tr.tr("Adaptive").into_owned(),
        ];
        if let Some(i) = builders::combo(
            ui,
            &tr.tr("Strategy:"),
            &strategies,
            strategy_index(strategy),
            custom && pocket,
        ) {
            cx.state().set_selected_pocket_strategy(strategy_at(i));
        }
        if let FieldEvent::Committed(text) = entry(
            ui,
            &mut cx.fields(),
            "raster_fill",
            &tr.tr("Raster Fill (%):"),
            &format!("{:.0}", fill * 100.0),
            "%",
            custom && pocket,
        ) && let Some(v) = parse_number(&text)
        {
            cx.state()
                .set_selected_raster_fill_ratio(v.clamp(0.0, 100.0) / 100.0);
        }
        ui.add_enabled(
            custom && pocket,
            egui::Label::new(egui::RichText::new(tr.tr(RASTER_HINT)).weak()),
        );
        ui.end_row();
    });
}
