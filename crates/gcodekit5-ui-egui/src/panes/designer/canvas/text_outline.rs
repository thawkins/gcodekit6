//! The outline of a text shape's glyphs, in world millimetres, for painting.
//!
//! The model's `DesignText::render` is only the text's bounding box. GTK draws the glyphs
//! with Cairo; here they are the outlines of the font the model cuts with
//! (`font_manager::get_font_for`), laid out as the model lays them out: the glyphs' box is
//! centred on the text's point, then rotated about it (counter-clockwise positive).
//!
//! Dependencies: `rusttype` (glyph outlines), `lyon` (path), `gcodekit5-designer` (model,
//! fonts).

use gcodekit5_designer::font_manager;
use gcodekit5_designer::model::DesignText;
use lyon::math::{Point as LyonPoint, point};
use lyon::path::Path;
use rusttype::{OutlineBuilder, Scale};

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
    let font = font_manager::get_font_for(&text.font_family, text.bold, text.italic);
    let scale = Scale::uniform(text.font_size as f32);
    let ascent = font.v_metrics(scale).ascent;
    let glyphs: Vec<_> = font
        .layout(&text.text, scale, rusttype::point(0.0, ascent))
        .collect();
    // The glyphs' box in layout space (Y down), as the model's bounds compute it.
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for glyph in &glyphs {
        if let Some(bb) = glyph.unpositioned().exact_bounding_box() {
            let p = glyph.position();
            x0 = x0.min(p.x + bb.min.x);
            y0 = y0.min(p.y + bb.min.y);
            x1 = x1.max(p.x + bb.max.x);
            y1 = y1.max(p.y + bb.max.y);
        }
    }
    if x0 > x1 || y0 > y1 {
        return None;
    }
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (sin, cos) = (text.rotation.to_radians() as f32).sin_cos();
    let (ox, oy) = (text.x as f32, text.y as f32);
    let mut path = Path::builder();
    for glyph in &glyphs {
        let p = glyph.position();
        let mut outline = Outline {
            builder: path,
            map: |x: f32, y: f32| {
                // Centre the box on the text's point, flip Y up, then rotate about it.
                let (dx, dy) = (p.x + x - cx, cy - (p.y + y));
                point(ox + dx * cos - dy * sin, oy + dx * sin + dy * cos)
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
