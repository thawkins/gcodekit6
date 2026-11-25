//! Position (centre X and Y, start depth Z) and Size (width, height, lock aspect).
//!
//! X and Y are the centre of the selection's box; W and H its size. With the aspect locked,
//! a new width sets the height by the shape's ratio (and the reverse), in the same step. A
//! rotated shape's size is read-only with the aspect locked, and a raster image's aspect is
//! always locked, as in GTK. Z is the start depth, and only a CNC's: a laser shows 0,
//! read-only.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model).

use super::super::builders::{self, FieldEvent, entry, length_text, length_unit, section};
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
        let x = entry(
            ui,
            &mut cx.fields(),
            "pos_x",
            &tr.tr("X:"),
            &length_text(cx_mm, units),
            unit,
            true,
        );
        if let FieldEvent::Committed(text) = x
            && let Some(v) = builders::parse_length(&text, units)
        {
            set_box(cx, Some((v, cy_mm)), None);
        }
        let y = entry(
            ui,
            &mut cx.fields(),
            "pos_y",
            &tr.tr("Y:"),
            &length_text(cy_mm, units),
            unit,
            true,
        );
        if let FieldEvent::Committed(text) = y
            && let Some(v) = builders::parse_length(&text, units)
        {
            set_box(cx, Some((cx_mm, v)), None);
        }
        let z = entry(
            ui,
            &mut cx.fields(),
            "pos_z",
            &tr.tr("(-Z:)"),
            &z,
            unit,
            !s.laser_mode,
        );
        if let FieldEvent::Committed(text) = z
            && let Some(v) = builders::parse_length(&text, units)
        {
            cx.state().set_selected_start_depth(v);
        }
    });
}

/// The Size section.
pub fn size(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let s = cx.summary;
    let units = cx.units;
    let (w, h) = s.size;
    let forced_lock = s.rotated() || s.raster;
    let locked = forced_lock || (s.single.is_some() && s.first.lock_aspect_ratio);
    let sizable = !s.rotated();
    let polygon = matches!(s.single, Some(gcodekit5_designer::model::Shape::Polygon(_)));
    let tr = cx.tr;
    section(ui, &tr.tr("Size"), |ui| {
        let unit = length_unit(units);
        let ratio = if h > 0.0 { w / h } else { 0.0 };
        let we = entry(
            ui,
            &mut cx.fields(),
            "width",
            &tr.tr("Width:"),
            &length_text(w, units),
            unit,
            sizable,
        );
        if let FieldEvent::Committed(text) = we
            && let Some(v) = builders::parse_length(&text, units)
        {
            let new_h = if locked && ratio > 0.0 { v / ratio } else { h };
            set_box(cx, None, Some((v, new_h)));
        }
        let he = entry(
            ui,
            &mut cx.fields(),
            "height",
            &tr.tr("Height:"),
            &length_text(h, units),
            unit,
            sizable && !polygon,
        );
        if let FieldEvent::Committed(text) = he
            && let Some(v) = builders::parse_length(&text, units)
        {
            let new_w = if locked && ratio > 0.0 { v * ratio } else { w };
            set_box(cx, None, Some((new_w, v)));
        }
        if let Some(lock) = builders::check(ui, &tr.tr("Lock Aspect:"), locked, !forced_lock) {
            change_selected(cx.state(), "Change Lock Aspect Ratio", |obj| {
                super::set_value(&mut obj.lock_aspect_ratio, lock)
            });
        }
    });
}

/// Moves the selection's box centre to `center` and/or resizes it to `size` (one step).
fn set_box(cx: &mut Ctx<'_>, center: Option<(f64, f64)>, size: Option<(f64, f64)>) {
    let (x, y) = center.unwrap_or(cx.summary.center);
    let (w, h) = size.unwrap_or(cx.summary.size);
    if size.is_some() && (w <= 0.0 || h <= 0.0) {
        return;
    }
    cx.state().set_selected_position_and_size_with_flags(
        x,
        y,
        w,
        h,
        center.is_some(),
        size.is_some(),
    );
}
