//! Rotation, Corner (radius and slot mode), Polygon (sides) and Path (closed).
//!
//! One shape's rotation is its absolute angle; several shapes show 0 and the value rotates
//! the whole selection about its centre once (GTK's relative rotation). A rotated shape
//! shows the "Rotated object" notice. The corner radius is a rectangle's corner radius or a
//! path's fillet; a polygon's sides need at least 3 and are read-only while it is rotated.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model and commands).

use gcodekit5_designer::commands::{ChangeProperty, DesignerCommand};
use gcodekit5_designer::model::Shape;

use super::super::builders::{
    self, FieldEvent, entry, length_text, length_unit, parse_number, section,
};
use super::super::update::shape_rotation;
use super::{Ctx, change_selected};

/// The notice shown for a rotated shape.
pub const ROTATED_NOTICE: &str = n_!("Rotated object: Dimensions locked");

/// The msgids of these sections.
pub fn msgids() -> Vec<&'static str> {
    vec![
        ROTATED_NOTICE,
        n_!("Rotation:"),
        n_!("Angle:"),
        n_!("Rotation angle (degrees). Applied on Enter or when leaving the field."),
        n_!(
            "Rotate the selection by this angle (degrees). Applied on Enter or when leaving the field."
        ),
        n_!("Corner"),
        n_!("Radius:"),
        n_!("Slot Mode:"),
        n_!("Polygon"),
        n_!("Sides:"),
        n_!("Path"),
        n_!("Closed:"),
    ]
}

/// The Rotation section.
pub fn rotation(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let s = cx.summary;
    let tr = cx.tr;
    let (shown, tip) = match &s.single {
        Some(shape) => (
            format!("{:.1}", shape_rotation(shape)),
            n_!("Rotation angle (degrees). Applied on Enter or when leaving the field."),
        ),
        None => (
            "0.0".to_string(),
            n_!(
                "Rotate the selection by this angle (degrees). Applied on Enter or when leaving the field."
            ),
        ),
    };
    section(ui, &tr.tr("Rotation:"), |ui| {
        let event = entry(
            ui,
            &mut cx.fields(),
            "rotation",
            &tr.tr("Angle:"),
            &shown,
            "deg",
            true,
        );
        if let FieldEvent::Committed(text) = event
            && let Some(degrees) = parse_number(&text)
        {
            cx.state().set_selected_rotation(degrees);
        }
    })
    .on_hover_text(tr.tr(tip));
}

/// The Corner section (a rectangle, or a path's fillet).
pub fn corner(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let s = cx.summary;
    let units = cx.units;
    let (radius, slot, is_rect) = match &s.single {
        Some(Shape::Rectangle(r)) => (r.corner_radius, r.is_slot, true),
        _ => (s.first.fillet, false, false),
    };
    let tr = cx.tr;
    section(ui, &tr.tr("Corner"), |ui| {
        let event = entry(
            ui,
            &mut cx.fields(),
            "corner_radius",
            &tr.tr("Radius:"),
            &length_text(radius, units),
            length_unit(units),
            true,
        );
        if let FieldEvent::Committed(text) = event
            && let Some(v) = builders::parse_length(&text, units)
        {
            cx.state().set_selected_corner_radius(v);
        }
        if let Some(on) = builders::check(ui, &tr.tr("Slot Mode:"), slot, is_rect) {
            cx.state().set_selected_is_slot(on);
        }
    });
}

/// The Polygon section.
pub fn polygon(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let Some(Shape::Polygon(p)) = &cx.summary.single else {
        return;
    };
    let (sides, rotated) = (p.sides, cx.summary.rotated());
    let tr = cx.tr;
    section(ui, &tr.tr("Polygon"), |ui| {
        let event = entry(
            ui,
            &mut cx.fields(),
            "sides",
            &tr.tr("Sides:"),
            &sides.to_string(),
            "",
            !rotated,
        );
        if let FieldEvent::Committed(text) = event
            && let Ok(n) = text.trim().parse::<u32>()
            && n >= 3
        {
            change_selected(cx.state(), "Change Polygon Sides", |obj| {
                match &mut obj.shape {
                    Shape::Polygon(polygon) => super::set_value(&mut polygon.sides, n),
                    _ => false,
                }
            });
        }
    });
}

/// The Path section (open or closed).
pub fn path(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let Some(Shape::Path(p)) = &cx.summary.single else {
        return;
    };
    let (closed, id) = (p.closed, cx.summary.first.id);
    let tr = cx.tr;
    section(ui, &tr.tr("Path"), |ui| {
        if let Some(on) = builders::check(ui, &tr.tr("Closed:"), closed, true) {
            let state = cx.state();
            if let Some(old_state) = state.canvas.get_shape(id).cloned() {
                let mut new_state = old_state.clone();
                if let Shape::Path(path) = &mut new_state.shape {
                    path.set_closed(on);
                }
                state.push_command(DesignerCommand::ChangeProperty(ChangeProperty {
                    id,
                    old_state,
                    new_state,
                }));
            }
        }
    });
}
