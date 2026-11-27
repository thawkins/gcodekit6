//! # Text Shape
//!
//! A text design shape that renders glyphs as Lyon paths for CNC toolpath
//! generation. Uses the font manager for font loading and supports
//! bold, italic, font size, and letter spacing.

use lyon::math::{Transform, point};
use lyon::path::Path;
use serde::{Deserialize, Serialize};

use csgrs::sketch::Sketch;
use csgrs::traits::CSG;
use nalgebra::{Matrix4, Vector3};
use rusttype::{PositionedGlyph, Scale, point as rt_point};

use super::{DesignerShape, LaserParams, Point, Property, PropertyValue};
use crate::font_manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignText {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub font_size: f64,
    pub font_family: String,
    pub bold: bool,
    pub italic: bool,
    pub rotation: f64,
    pub laser_params: LaserParams,
}

/// The laid-out glyphs of a [`DesignText`] and the map from layout space to world mm.
///
/// Built by [`DesignText::layout`]. Layout space is `rusttype`'s: millimetres, Y down,
/// the first line's top at y = 0. A glyph's outline point `(x, y)` (relative to the glyph
/// origin) lies at layout point `glyph.position() + (x, y)`.
#[derive(Debug, Clone)]
pub struct TextLayout {
    /// The glyphs in layout order, positioned in layout space.
    pub glyphs: Vec<PositionedGlyph<'static>>,
    /// Width of the glyphs' ink box, mm.
    pub width: f64,
    /// Height of the glyphs' ink box, mm.
    pub height: f64,
    /// Centre of the ink box in layout space.
    centre: (f64, f64),
    /// The text's point in world mm, where the ink box centre lands.
    origin: (f64, f64),
    sin: f64,
    cos: f64,
}

impl TextLayout {
    /// Maps a layout-space point (Y down) to world millimetres (Y up): the ink box centre
    /// goes to the text's point, then the point is rotated about it counter-clockwise.
    ///
    /// # Arguments
    /// * `x`, `y` - The layout-space point.
    pub fn to_world(&self, x: f64, y: f64) -> Point {
        let (dx, dy) = (x - self.centre.0, self.centre.1 - y);
        Point::new(
            self.origin.0 + dx * self.cos - dy * self.sin,
            self.origin.1 + dx * self.sin + dy * self.cos,
        )
    }

    /// The world position of glyph `glyph`'s origin (its pen position on the baseline).
    ///
    /// # Arguments
    /// * `glyph` - A glyph of this layout.
    pub fn glyph_origin(&self, glyph: &PositionedGlyph<'_>) -> Point {
        let p = glyph.position();
        self.to_world(f64::from(p.x), f64::from(p.y))
    }
}

impl DesignText {
    pub fn new(text: String, x: f64, y: f64, font_size: f64) -> Self {
        Self {
            text,
            x,
            y,
            font_size,
            font_family: "Sans".to_string(),
            bold: false,
            italic: false,
            rotation: 0.0,
            laser_params: LaserParams::default(),
        }
    }

    /// Lays out the text's glyphs: the one layout the canvas painter, the bounds and the
    /// G-code generator all use, so what is cut is what is drawn.
    ///
    /// Each line is laid out by `rusttype` (with kerning) from x = 0; line `i` has its
    /// baseline at `ascent + i * (ascent - descent + line_gap)` (layout space, Y down).
    /// The glyphs' ink box is centred on `(x, y)` and rotated about it, counter-clockwise
    /// positive, like every other shape.
    ///
    /// # Returns
    /// The layout, or `None` when no glyph has an outline (empty or blank text).
    pub fn layout(&self) -> Option<TextLayout> {
        let font = font_manager::get_font_for(&self.font_family, self.bold, self.italic);
        let scale = Scale::uniform(self.font_size as f32);
        let v_metrics = font.v_metrics(scale);
        let line_advance = v_metrics.ascent - v_metrics.descent + v_metrics.line_gap;

        let glyphs: Vec<PositionedGlyph<'static>> = self
            .text
            .split('\n')
            .enumerate()
            .flat_map(|(i, line)| {
                let baseline = v_metrics.ascent + i as f32 * line_advance;
                font.layout(line, scale, rt_point(0.0, baseline))
            })
            .collect();

        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for glyph in &glyphs {
            if let Some(bb) = glyph.unpositioned().exact_bounding_box() {
                let pos = glyph.position();
                x0 = x0.min(pos.x + bb.min.x);
                y0 = y0.min(pos.y + bb.min.y);
                x1 = x1.max(pos.x + bb.max.x);
                y1 = y1.max(pos.y + bb.max.y);
            }
        }
        if x0 > x1 || y0 > y1 {
            return None;
        }

        let (sin, cos) = self.rotation.to_radians().sin_cos();
        Some(TextLayout {
            glyphs,
            width: f64::from(x1 - x0),
            height: f64::from(y1 - y0),
            centre: (f64::from(x0 + x1) / 2.0, f64::from(y0 + y1) / 2.0),
            origin: (self.x, self.y),
            sin,
            cos,
        })
    }

    /// Computes the text's bounding box, taking (x, y) as the CENTRE.
    fn calculate_bounds(&self) -> (f64, f64, f64, f64) {
        let Some(layout) = self.layout() else {
            let approx_width = self.text.len() as f64 * self.font_size * 0.6;
            let height = self.font_size;
            return (
                self.x - approx_width / 2.0,
                self.y - height / 2.0,
                self.x + approx_width / 2.0,
                self.y + height / 2.0,
            );
        };

        // Actual width and height of the glyph block (rusttype, Y-down).
        let rel_width = layout.width;
        let rel_height = layout.height;

        // Centre the actual AABB on (self.x, self.y) in canvas coordinates (Y-up).
        // Wherever the baseline is, the glyph block (its AABB)
        // is centred on the point the user controls.
        let canvas_x_left = self.x - rel_width / 2.0;
        let canvas_y_bottom = self.y - rel_height / 2.0;

        (
            canvas_x_left,
            canvas_y_bottom,
            canvas_x_left + rel_width,
            canvas_y_bottom + rel_height,
        )
    }
}

impl DesignerShape for DesignText {
    fn render(&self) -> Path {
        let (x1, y1, x2, y2) = self.calculate_bounds();

        let mut builder = Path::builder();
        builder.add_rectangle(
            &lyon::math::Box2D::new(point(x1 as f32, y1 as f32), point(x2 as f32, y2 as f32)),
            lyon::path::Winding::Positive,
        );
        let path = builder.build();

        if self.rotation.abs() > 1e-6 {
            let cx = (x1 + x2) / 2.0;
            let cy = (y1 + y2) / 2.0;
            let transform = Transform::translation(-cx as f32, -cy as f32)
                .then_rotate(lyon::math::Angle::radians(self.rotation.to_radians() as f32))
                .then_translate(lyon::math::vector(cx as f32, cy as f32));
            return path.transformed(&transform);
        }
        path
    }

    fn as_csg(&self) -> Sketch<()> {
        let (x1, y1, x2, y2) = self.calculate_bounds();
        let w = x2 - x1;
        let h = y2 - y1;
        let sketch = Sketch::rectangle(w, h, None);
        let cx = x1 + w / 2.0;
        let cy = y1 + h / 2.0;

        let center_fix = Matrix4::new_translation(&Vector3::new(-w / 2.0, -h / 2.0, 0.0));
        let rotation = Matrix4::new_rotation(Vector3::new(0.0, 0.0, self.rotation.to_radians()));
        let translation = Matrix4::new_translation(&Vector3::new(cx, cy, 0.0));

        sketch.transform(&(translation * rotation * center_fix))
    }

    fn bounds(&self) -> (f64, f64, f64, f64) {
        self.calculate_bounds()
    }

    fn transform(&mut self, t: &Transform) {
        let p = t.transform_point(point(self.x as f32, self.y as f32));
        self.x = p.x as f64;
        self.y = p.y as f64;

        let sx = (t.m11 * t.m11 + t.m12 * t.m12).sqrt();
        let sy = (t.m21 * t.m21 + t.m22 * t.m22).sqrt();
        let s = ((sx + sy) / 2.0).max(1e-6);
        self.font_size *= s as f64;

        let angle_deg = t.m12.atan2(t.m11).to_degrees() as f64;
        let det = t.m11 * t.m22 - t.m12 * t.m21;
        let mut new_rotation = self.rotation + angle_deg;
        if det < 0.0 {
            new_rotation = -new_rotation;
        }
        self.rotation = new_rotation;
    }

    fn properties(&self) -> Vec<Property> {
        vec![
            Property {
                name: "Text".to_string(),
                value: PropertyValue::String(self.text.clone()),
            },
            Property {
                name: "X".to_string(),
                value: PropertyValue::Number(self.x),
            },
            Property {
                name: "Y".to_string(),
                value: PropertyValue::Number(self.y),
            },
            Property {
                name: "Font Size".to_string(),
                value: PropertyValue::Number(self.font_size),
            },
            Property {
                name: "Rotation".to_string(),
                value: PropertyValue::Number(self.rotation),
            },
        ]
    }

    fn contains_point(&self, p: Point, tolerance: f64) -> bool {
        let (x1, y1, x2, y2) = self.calculate_bounds();
        p.x >= x1 - tolerance
            && p.x <= x2 + tolerance
            && p.y >= y1 - tolerance
            && p.y <= y2 + tolerance
    }

    fn resize(&mut self, handle: usize, dx: f64, dy: f64) {
        if handle == 4 {
            self.translate(dx, dy);
            return;
        }

        let (x1, y1, x2, y2) = self.calculate_bounds();
        let current_width = x2 - x1;
        let current_height = y2 - y1;

        let new_width = (current_width + dx).max(10.0);
        let new_height = (current_height + dy).max(10.0);

        let scale_x = new_width / current_width;
        let scale_y = new_height / current_height;
        let scale = (scale_x + scale_y) / 2.0;

        self.font_size = (self.font_size * scale).max(4.0);
    }

    fn translate(&mut self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
    }
}
