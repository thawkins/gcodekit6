//! Position (centre X and Y, start depth Z) and Size (width, height, lock aspect).
//!
//! X and Y are the centre of the selection's box; W and H its size, above 0. With the
//! aspect locked, a new width sets the height by the shape's ratio (and the reverse), in the
//! same step. A rotated shape's size is read-only with the aspect locked, and a raster
//! image's aspect is always locked. A text's aspect is always locked too: the
//! model scales its font by the mean of the two factors, so a free W or H would not be what
//! the text becomes. Z is the start depth below the stock top (0 or more; the G-code starts
//! at stock thickness minus it), and only a CNC's: a laser shows 0, read-only.
//!
//! Dependencies: `egui`, `gcodekit6-designer` (model).

use gcodekit6_designer::model::Shape;

use super::super::builders::{
    self, Check, entry, length, length_text, length_unit, non_negative, positive, section,
};
use super::{Ctx, change_selected};

/// The msgids of these sections.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Position"),
        n_!("X:"),
        n_!("Y:"),
        n_!("(-Z:)"),
        n_!("Size"),
        n_!("Width:"),
        n_!("Height:"),
        n_!("Lock Aspect:"),
    ]
}

/// Whether the selection's aspect is locked (by the user, or forced).
fn locked(cx: &Ctx<'_>) -> bool {
    let s = cx.summary;
    forced_lock(cx) || (s.single().is_some() && s.first.lock_aspect_ratio)
}

/// Whether the selection's aspect is locked whatever the checkbox says.
fn forced_lock(cx: &Ctx<'_>) -> bool {
    let s = cx.summary;
    s.rotated() || s.raster || matches!(s.single(), Some(Shape::Text(_)))
}

/// Applies the committed `text` of field `key`, if it is one of these sections'.
///
/// # Arguments
/// * `cx` - The frame's context, for the shapes the edit belongs to.
/// * `key` - The field.
/// * `text` - The committed text.
pub fn apply(cx: &mut Ctx<'_>, key: &str, text: &str) -> Option<Check> {
    let units = cx.units;
    let s = cx.summary;
    let (cx_mm, cy_mm) = s.center;
    let (w, h) = s.size;
    let ratio = if h > 0.0 { w / h } else { 0.0 };
    let lock = locked(cx);
    let result = match key {
        "pos_x" => length(text, units).map(|v| set_box(cx, Some((v, cy_mm)), None)),
        "pos_y" => length(text, units).map(|v| set_box(cx, Some((cx_mm, v)), None)),
        "pos_z" if !s.laser_mode => length(text, units)
            .and_then(non_negative)
            .map(|v| cx.state().set_selected_start_depth(v)),
        "width" if !s.rotated() => length(text, units).and_then(positive).map(|v| {
            let new_h = if lock && ratio > 0.0 { v / ratio } else { h };
            set_box(cx, None, Some((v, new_h)));
        }),
        "height" if !s.rotated() => length(text, units).and_then(positive).map(|v| {
            let new_w = if lock && ratio > 0.0 { v * ratio } else { w };
            set_box(cx, None, Some((new_w, v)));
        }),
        // A read-only field (laser Z, a rotated shape's size): nothing to apply.
        "pos_z" | "width" | "height" => Ok(()),
        _ => return None,
    };
    Some(result)
}

/// The Position section.
pub fn position(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let s = cx.summary;
    let units = cx.units;
    let (cx_mm, cy_mm) = s.center;
    let z = if s.laser_mode {
        "0".to_string()
    } else {
        length_text(s.first.start_depth, units)
    };
    let tr = cx.tr;
    section(ui, &tr.tr("Position"), |ui| {
        let unit = length_unit(units);
        let x = length_text(cx_mm, units);
        entry(ui, &mut cx.fields(), "pos_x", &tr.tr("X:"), &x, unit, true);
        let y = length_text(cy_mm, units);
        entry(ui, &mut cx.fields(), "pos_y", &tr.tr("Y:"), &y, unit, true);
        let enabled = !s.laser_mode;
        entry(
            ui,
            &mut cx.fields(),
            "pos_z",
            &tr.tr("(-Z:)"),
            &z,
            unit,
            enabled,
        );
    });
}

/// The Size section.
pub fn size(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let s = cx.summary;
    let units = cx.units;
    let (w, h) = s.size;
    let forced = forced_lock(cx);
    let lock = locked(cx);
    let sizable = !s.rotated();
    let polygon = matches!(s.single(), Some(Shape::Polygon(_)));
    let tr = cx.tr;
    section(ui, &tr.tr("Size"), |ui| {
        let unit = length_unit(units);
        let (wt, ht) = (length_text(w, units), length_text(h, units));
        entry(
            ui,
            &mut cx.fields(),
            "width",
            &tr.tr("Width:"),
            &wt,
            unit,
            sizable,
        );
        let height_enabled = sizable && !polygon;
        entry(
            ui,
            &mut cx.fields(),
            "height",
            &tr.tr("Height:"),
            &ht,
            unit,
            height_enabled,
        );
        if let Some(on) = builders::check(ui, &tr.tr("Lock Aspect:"), lock, !forced) {
            change_selected(cx.state(), "Change Lock Aspect Ratio", |obj| {
                super::set_value(&mut obj.lock_aspect_ratio, on)
            });
        }
    });
}

/// Moves the selection's box centre to `center` and/or resizes it to `size` (one step).
fn set_box(cx: &mut Ctx<'_>, center: Option<(f64, f64)>, size: Option<(f64, f64)>) {
    let (x, y) = center.unwrap_or(cx.summary.center);
    let (w, h) = size.unwrap_or(cx.summary.size);
    cx.state().set_selected_position_and_size_with_flags(
        x,
        y,
        w,
        h,
        center.is_some(),
        size.is_some(),
    );
}
