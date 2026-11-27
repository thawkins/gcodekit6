//! The outline of a text shape's glyphs, in world millimetres, for painting.
//!
//! The model's `DesignText::render` is only the text's bounding box. GTK draws the glyphs
//! with Cairo; here they are the outlines of the font the model cuts with, placed by the
//! model's one text layout (`DesignText::layout`), which the bounds and the G-code
//! generator also use: the glyphs' box is centred on the text's point, then rotated about
//! it (counter-clockwise positive).
//!
//! Dependencies: `rusttype` (glyph outlines), `lyon` (path), `gcodekit5-designer` (model).

use gcodekit5_designer::model::DesignText;
use lyon::math::{Point as LyonPoint, point};
use lyon::path::Path;
use rusttype::OutlineBuilder;

/// Collects glyph outlines into a lyon path, mapping each layout point to world mm.
struct Outline<F: Fn(f32, f32) -> LyonPoint> {
    builder: lyon::path::path::Builder,
    /// Maps a layout point (Y down) to world millimetres.
    map: F,
    /// Whether a sub-path is open.
    open: bool,
}

impl<F: Fn(f32, f32) -> LyonPoint> OutlineBuilder for Outline<F> {
    fn move_to(&mut self, x: f32, y: f32) {
        if self.open {
            self.builder.end(true);
        }
        self.builder.begin((self.map)(x, y));
        self.open = true;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to((self.map)(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.builder
            .quadratic_bezier_to((self.map)(x1, y1), (self.map)(x, y));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.builder
            .cubic_bezier_to((self.map)(x1, y1), (self.map)(x2, y2), (self.map)(x, y));
    }

    fn close(&mut self) {
        if self.open {
            self.builder.end(true);
            self.open = false;
        }
    }
}

/// The glyph outlines of `text` in world millimetres, or `None` when it has no glyphs (the
/// caller then paints the model's box).
///
/// # Arguments
/// * `text` - The text shape.
pub fn text_path(text: &DesignText) -> Option<Path> {
    let layout = text.layout()?;
    let mut path = Path::builder();
    for glyph in &layout.glyphs {
        let p = glyph.position();
        let mut outline = Outline {
            builder: path,
            map: |x: f32, y: f32| {
                let w = layout.to_world(f64::from(p.x + x), f64::from(p.y + y));
                point(w.x as f32, w.y as f32)
            },
            open: false,
        };
        glyph.unpositioned().build_outline(&mut outline);
        if outline.open {
            outline.builder.end(true);
        }
        path = outline.builder;
    }
    Some(path.build())
}
