//! `DesignText::layout` is the one text layout: the bounds are its ink box centred on the
//! text's point, and a newline starts a new line one line-advance lower (P2DES-3).

use gcodekit5_designer::font_manager;
use gcodekit5_designer::model::{DesignText, DesignerShape};
use rusttype::Scale;

/// A text in the bundled fallback font, the same on every machine.
fn text(s: &str) -> DesignText {
    let mut t = DesignText::new(s.to_string(), 12.0, 7.0, 10.0);
    t.font_family = "GCodeKit5 Bundled Fallback".to_string();
    t
}

#[test]
fn test_text_layout_bounds_are_the_ink_box_centred_on_the_point() {
    let t = text("gyp");
    let layout = t.layout().expect("glyphs");
    let (x0, y0, x1, y1) = t.bounds();
    assert!((x1 - x0 - layout.width).abs() < 1e-9);
    assert!((y1 - y0 - layout.height).abs() < 1e-9);
    assert!(((x0 + x1) / 2.0 - 12.0).abs() < 1e-9);
    assert!(((y0 + y1) / 2.0 - 7.0).abs() < 1e-9);
    assert!(text("   ").layout().is_none());
}

#[test]
fn test_text_layout_newline_starts_a_lower_line() {
    let layout = text("H\nH").layout().expect("glyphs");
    // The newline is a line break, not a glyph.
    assert_eq!(layout.glyphs.len(), 2);
    let v = font_manager::get_font().v_metrics(Scale::uniform(10.0));
    let advance = f64::from(v.ascent - v.descent + v.line_gap);
    let (a, b) = (
        layout.glyph_origin(&layout.glyphs[0]),
        layout.glyph_origin(&layout.glyphs[1]),
    );
    assert!((a.x - b.x).abs() < 1e-6);
    assert!((a.y - b.y - advance).abs() < 1e-4, "{a:?} {b:?}");
}
