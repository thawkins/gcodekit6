//! Geometry Operations: offset and chamfer, with a live preview.
//!
//! While the user types a distance the canvas previews every selected shape with it (in
//! the warning colour); committing applies it as one step, and only when it differs from a
//! shape's value by more than the shown precision (so moving through the field records
//! nothing, and a path's fillet is kept). A chamfer clears a path's fillet, as in the model.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model, `ops`).

use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_designer::DesignerState;
use gcodekit5_designer::canvas::DrawingObject;
use gcodekit5_designer::model::Shape;

use super::super::builders::{FieldEvent, entry, length_text, length_unit, parse_length, section};
use super::Ctx;

/// The keys of the two fields; the panel clears the preview when neither is being edited.
pub const KEYS: [&str; 2] = ["offset", "chamfer"];

/// The msgids of this section.
pub fn msgids() -> Vec<&'static str> {
    vec![n_!("Geometry Operations"), n_!("Offset:"), n_!("Chamfer:")]
}

/// The parts of an effect that differ.
struct Effect {
    key: &'static str,
    msgid: &'static str,
    preview: fn(&Shape, f64) -> Shape,
    current: fn(&DrawingObject) -> f64,
    apply: fn(&mut DesignerState, f64),
}

/// Half the last shown digit in `units`, mm: a smaller difference is display rounding.
fn tolerance_mm(units: MeasurementSystem) -> f64 {
    match units {
        MeasurementSystem::Metric => 0.005 + 1e-9,
        MeasurementSystem::Imperial => 0.005 * 25.4 + 1e-9,
    }
}

/// The Geometry Operations section.
pub fn show(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let tr = cx.tr;
    let effects = [
        Effect {
            key: KEYS[0],
            msgid: n_!("Offset:"),
            preview: gcodekit5_designer::ops::perform_offset,
            current: |o| o.offset,
            apply: DesignerState::set_offset_selected,
        },
        Effect {
            key: KEYS[1],
            msgid: n_!("Chamfer:"),
            preview: gcodekit5_designer::ops::perform_chamfer,
            current: |o| o.chamfer,
            apply: DesignerState::set_chamfer_selected,
        },
    ];
    section(ui, &tr.tr("Geometry Operations"), |ui| {
        for effect in effects {
            effect_row(ui, cx, &effect);
        }
    });
}

/// One effect's row: preview while typing, apply on commit.
fn effect_row(ui: &mut egui::Ui, cx: &mut Ctx<'_>, effect: &Effect) {
    let units = cx.units;
    let tr = cx.tr;
    let shown = length_text((effect.current)(&cx.summary.first), units);
    let event = entry(
        ui,
        &mut cx.fields(),
        effect.key,
        &tr.tr(effect.msgid),
        &shown,
        length_unit(units),
        true,
    );
    match event {
        FieldEvent::Typed(text) => {
            let shapes = parse_length(&text, units).map_or_else(Vec::new, |d| {
                cx.designer
                    .state()
                    .canvas
                    .shapes()
                    .filter(|s| s.selected)
                    .map(|s| (effect.preview)(&s.shape, d))
                    .collect()
            });
            cx.designer.set_effect_preview(shapes);
        }
        FieldEvent::Committed(text) => {
            cx.designer.set_effect_preview(Vec::new());
            if let Some(d) = parse_length(&text, units) {
                let state = cx.state();
                let changed = state
                    .canvas
                    .shapes()
                    .filter(|s| s.selected)
                    .any(|s| ((effect.current)(s) - d).abs() > tolerance_mm(units));
                if changed {
                    (effect.apply)(state, d);
                }
            }
        }
        FieldEvent::None => {}
    }
}
