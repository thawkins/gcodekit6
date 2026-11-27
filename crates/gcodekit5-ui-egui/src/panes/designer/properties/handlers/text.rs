//! Text: content, font family, bold, italic, and size in points (stored in millimetres).
//!
//! Each change applies to every selected text shape as one undo step. The family, bold
//! and italic are applied together (one "Change Font" step).
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model).

use gcodekit5_designer::model::{DesignText, Shape};

use super::super::builders::{self, Check, E_POSITIVE, E_TEXT, entry, section};
use super::{Ctx, change_selected};
use crate::panes::designer::canvas::text_tool::{font_combo, format_points, parse_points_mm};

/// The msgids of this section.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Text:"),
        n_!("Content:"),
        n_!("Font:"),
        n_!("Bold"),
        n_!("Italic"),
        n_!("Size:"),
    ]
}

/// Applies `modify` to every selected text shape as one undo step named `name`.
fn change_text<F>(cx: &mut Ctx<'_>, name: &str, modify: F)
where
    F: Fn(&mut DesignText) -> bool,
{
    change_selected(cx.state(), name, |obj| match &mut obj.shape {
        Shape::Text(t) => modify(t),
        _ => false,
    });
}

/// Applies the committed `text` of field `key`, if it is one of this section's: the
/// content (not blank, as the Add Text dialog) or the size in points (above 0).
///
/// # Arguments
/// * `cx` - The frame's context, for the shapes the edit belongs to.
/// * `key` - The field.
/// * `text` - The committed text.
pub fn apply(cx: &mut Ctx<'_>, key: &str, text: &str) -> Option<Check> {
    let result = match key {
        "text_content" if text.trim().is_empty() => Err(E_TEXT),
        "text_content" => {
            change_text(cx, "Change Text", |s| {
                super::set_value(&mut s.text, text.to_string())
            });
            Ok(())
        }
        "font_size" => parse_points_mm(text).ok_or(E_POSITIVE).map(|mm| {
            change_text(cx, "Change Font Size", |s| {
                super::set_f64(&mut s.font_size, mm)
            });
        }),
        _ => return None,
    };
    Some(result)
}

/// The Text section.
pub fn show(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let Some(Shape::Text(t)) = cx.summary.single() else {
        return;
    };
    let t = t.clone();
    let tr = cx.tr;
    section(ui, &tr.tr("Text:"), |ui| {
        entry(
            ui,
            &mut cx.fields(),
            "text_content",
            &tr.tr("Content:"),
            &t.text,
            "",
            true,
        );
        let font_label = ui.label(tr.tr("Font:"));
        let mut family = t.font_family.clone();
        let picked = font_combo(ui, "gcodekit5-properties-font", &mut family, font_label.id);
        ui.end_row();
        let mut bold = builders::check(ui, &tr.tr("Bold"), t.bold, true);
        let mut italic = builders::check(ui, &tr.tr("Italic"), t.italic, true);
        if picked || bold.is_some() || italic.is_some() {
            let (b, i) = (
                bold.take().unwrap_or(t.bold),
                italic.take().unwrap_or(t.italic),
            );
            change_text(cx, "Change Font", |s| {
                let changed = s.font_family != family || s.bold != b || s.italic != i;
                s.font_family.clone_from(&family);
                s.bold = b;
                s.italic = i;
                changed
            });
        }
        entry(
            ui,
            &mut cx.fields(),
            "font_size",
            &tr.tr("Size:"),
            &format_points(t.font_size),
            "pt",
            true,
        );
    });
}
