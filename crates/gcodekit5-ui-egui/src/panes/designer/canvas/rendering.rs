//! Paints the design canvas with `egui::Painter`, as the GTK canvas draws it: the grid
//! (minor lines at a fifth of the spacing), the work-area bounds, the origin axes, the shapes
//! (selected in the error colour, grouped in the success colour, with offset/fillet/chamfer
//! in the warning colour), text as its glyph outlines, the properties panel's live effect
//! preview (warning colour), the selection handles, the dashed box of a drag, and the polyline
//! being drawn with its rubber band.
//!
//! Line widths are in screen points, so they look the same at any zoom. Shapes are the
//! model's lyon paths, flattened to a quarter point.
//!
//! Dependencies: `egui`, `lyon` (path flattening), `gcodekit5-designer` (model).

use egui::{Color32, Painter, Pos2, Shape as EguiShape, Stroke, StrokeKind};
use gcodekit5_designer::DesignerState;
use gcodekit5_designer::model::{DesignerShape, Shape};
use lyon::path::iterator::PathIterator;

use super::editing::{self, HANDLE_SIZE_PT};
use super::{CanvasInput, CanvasView, ShapeFlags, text_outline};
use crate::theme::{self, Palette};

/// Grid and line widths, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineWidths {
    /// Major grid lines and axes (the setting "Grid Major Line Width").
    pub grid_major: f32,
    /// Minor grid lines (the setting "Grid Minor Line Width").
    pub grid_minor: f32,
}

impl Default for LineWidths {
    fn default() -> Self {
        Self {
            grid_major: 2.0,
            grid_minor: 1.0,
        }
    }
}

/// Grid lines closer than this many points are not drawn (they would fill the canvas).
const MIN_GRID_GAP_PT: f32 = 3.0;
/// How far the origin axes reach, mm (GTK's world extent).
const AXIS_EXTENT_MM: f64 = 2000.0;
/// Flattening tolerance of curves, in points.
const FLATTEN_PT: f32 = 0.25;

/// Paints the canvas.
///
/// # Arguments
/// * `painter` - Painter clipped to the canvas.
/// * `view` - The view to paint with.
/// * `state` - The design.
/// * `flags` - The hidden shapes, which are not painted.
/// * `input` - The drag and polyline in progress.
/// * `palette` - Theme colours.
/// * `widths` - Grid line widths.
/// * `preview` - The properties panel's live offset or chamfer preview, in the warning colour.
/// * `work_area` - The device's work area (mm), outlined in GTK's device-bounds colour.
#[allow(clippy::too_many_arguments)] // the frame's whole paint context, called once
pub fn paint(
    painter: &Painter,
    view: &CanvasView,
    state: &DesignerState,
    flags: &ShapeFlags,
    input: &CanvasInput,
    palette: &Palette,
    widths: LineWidths,
    preview: &[Shape],
    work_area: editing::Bounds,
) {
    painter.rect_filled(view.rect(), 0.0, palette.view_bg);
    if state.show_grid {
        paint_grid(
            painter,
            view,
            state.grid_spacing_mm.max(0.1),
            palette.window_fg,
            widths,
        );
    }
    let (x0, y0, x1, y1) = work_area;
    let bounds = egui::Rect::from_two_pos(view.to_screen(x0, y0), view.to_screen(x1, y1));
    painter.rect_stroke(
        bounds,
        0.0,
        Stroke::new(2.0, theme::DEVICE_BOUNDS),
        StrokeKind::Middle,
    );
    let (e, o) = (AXIS_EXTENT_MM, 0.0);
    painter.line_segment(
        [view.to_screen(-e, o), view.to_screen(e, o)],
        Stroke::new(1.0, theme::AXIS_X),
    );
    painter.line_segment(
        [view.to_screen(o, -e), view.to_screen(o, e)],
        Stroke::new(1.0, theme::AXIS_Y),
    );
    paint_polyline(painter, view, input, palette.accent);
    for obj in state.canvas.shapes().filter(|o| !flags.is_hidden(o.id)) {
        let modified =
            obj.offset.abs() > 1e-6 || obj.fillet.abs() > 1e-6 || obj.chamfer.abs() > 1e-6;
        let stroke = if obj.selected {
            Stroke::new(3.0, palette.error)
        } else if obj.group_id.is_some() {
            Stroke::new(2.0, palette.success)
        } else if modified {
            Stroke::new(2.0, palette.warning)
        } else {
            Stroke::new(2.0, palette.window_fg)
        };
        if modified {
            paint_shape(painter, view, &obj.get_effective_shape(), stroke);
        } else {
            paint_shape(painter, view, &obj.shape, stroke);
        }
    }
    for shape in preview {
        paint_shape(painter, view, shape, Stroke::new(2.0, palette.warning));
    }
    if let Some(bounds) = editing::selection_box(state) {
        paint_handles(painter, view, bounds, palette.accent);
    }
    if state.selected_count() == 0
        && let Some((a, b)) = input.preview_box()
    {
        let rect = egui::Rect::from_two_pos(view.to_screen(a.0, a.1), view.to_screen(b.0, b.1));
        let color = palette.accent.gamma_multiply(0.7);
        let corners = [
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
            rect.left_top(),
        ];
        painter.extend(EguiShape::dashed_line(
            &corners,
            Stroke::new(2.0, color),
            5.0,
            5.0,
        ));
    }
}

/// The grid: minor lines (20% of the text colour), major lines every `spacing` mm (40%), and
/// the axes through the origin (80%).
fn paint_grid(painter: &Painter, view: &CanvasView, spacing: f64, fg: Color32, widths: LineWidths) {
    let rect = view.rect();
    let lo = view.screen_to_world(rect.left_bottom());
    let hi = view.screen_to_world(rect.right_top());
    let (lo, hi) = (
        (f64::from(lo.x), f64::from(lo.y)),
        (f64::from(hi.x), f64::from(hi.y)),
    );
    let zoom = f64::from(view.zoom());
    let lines = |step: f64, stroke: Stroke, skip_major: bool| {
        if (step * zoom) < f64::from(MIN_GRID_GAP_PT) {
            return;
        }
        let is_major = |v: f64| ((v / spacing).round() - v / spacing).abs() <= 0.01;
        let mut x = (lo.0 / step).floor() * step;
        while x <= hi.0 {
            if !(skip_major && is_major(x)) {
                painter.line_segment([view.to_screen(x, lo.1), view.to_screen(x, hi.1)], stroke);
            }
            x += step;
        }
        let mut y = (lo.1 / step).floor() * step;
        while y <= hi.1 {
            if !(skip_major && is_major(y)) {
                painter.line_segment([view.to_screen(lo.0, y), view.to_screen(hi.0, y)], stroke);
            }
            y += step;
        }
    };
    lines(
        spacing / 5.0,
        Stroke::new(widths.grid_minor, fg.gamma_multiply(0.2)),
        true,
    );
    lines(
        spacing,
        Stroke::new(widths.grid_major, fg.gamma_multiply(0.4)),
        false,
    );
    let axis = Stroke::new(widths.grid_major, fg.gamma_multiply(0.8));
    if lo.1 <= 0.0 && hi.1 >= 0.0 {
        painter.line_segment([view.to_screen(lo.0, 0.0), view.to_screen(hi.0, 0.0)], axis);
    }
    if lo.0 <= 0.0 && hi.0 >= 0.0 {
        painter.line_segment([view.to_screen(0.0, lo.1), view.to_screen(0.0, hi.1)], axis);
    }
}

/// Strokes `shape`'s outline (a raster image as its box, text as its glyph outlines).
fn paint_shape(painter: &Painter, view: &CanvasView, shape: &Shape, stroke: Stroke) {
    for (outline, closed) in outlines(view, shape) {
        if closed {
            painter.add(EguiShape::closed_line(outline, stroke));
        } else {
            painter.add(EguiShape::line(outline, stroke));
        }
    }
}

/// `shape`'s outlines in screen points, each with whether it is closed: a raster image is
/// its box, text its glyph outlines, anything else the model's path flattened.
fn outlines(view: &CanvasView, shape: &Shape) -> Vec<(Vec<Pos2>, bool)> {
    if let Shape::RasterImage(raster) = shape {
        let (x0, y0, x1, y1) = raster.bounds();
        let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
        return vec![(
            corners.iter().map(|&(x, y)| view.to_screen(x, y)).collect(),
            true,
        )];
    }
    let tolerance = FLATTEN_PT / view.zoom().max(1e-6);
    let mut out = Vec::new();
    let mut points: Vec<Pos2> = Vec::new();
    let to_screen = |p: lyon::math::Point| view.to_screen(f64::from(p.x), f64::from(p.y));
    let path = match shape {
        Shape::Text(text) => text_outline::text_path(text).unwrap_or_else(|| shape.render()),
        _ => shape.render(),
    };
    for event in path.iter().flattened(tolerance) {
        match event {
            lyon::path::Event::Begin { at } => {
                points.clear();
                points.push(to_screen(at));
            }
            lyon::path::Event::Line { to, .. } => points.push(to_screen(to)),
            lyon::path::Event::End { close, .. } => out.push((std::mem::take(&mut points), close)),
            // Flattening leaves only lines.
            lyon::path::Event::Quadratic { to, .. } | lyon::path::Event::Cubic { to, .. } => {
                points.push(to_screen(to));
            }
        }
    }
    out
}

/// The gallery shape waiting to be placed: dashed, in the accent colour at 65%, as GTK.
///
/// # Arguments
/// * `painter` - Painter clipped to the canvas.
/// * `view` - The view to paint with.
/// * `ghost` - The shape at the pointer, if the pointer is over the canvas.
/// * `accent` - The theme's accent colour.
pub fn paint_ghost(painter: &Painter, view: &CanvasView, ghost: Option<&Shape>, accent: Color32) {
    let Some(shape) = ghost else {
        return;
    };
    let stroke = Stroke::new(2.0, accent.gamma_multiply(0.65));
    for (mut outline, closed) in outlines(view, shape) {
        if closed && let Some(first) = outline.first().copied() {
            outline.push(first);
        }
        painter.extend(EguiShape::dashed_line(&outline, stroke, 4.0, 4.0));
    }
}

/// The selection box's corner handles: white squares with an accent border.
fn paint_handles(painter: &Painter, view: &CanvasView, bounds: editing::Bounds, accent: Color32) {
    let size = HANDLE_SIZE_PT as f32;
    for (x, y, _) in editing::handle_corners(bounds) {
        let rect = egui::Rect::from_center_size(view.to_screen(x, y), egui::vec2(size, size));
        painter.rect_filled(rect, 0.0, Color32::WHITE);
        painter.rect_stroke(rect, 0.0, Stroke::new(2.0, accent), StrokeKind::Middle);
    }
}

/// The polyline being drawn: its segments, a rubber band to the pointer, and its points.
fn paint_polyline(painter: &Painter, view: &CanvasView, input: &CanvasInput, accent: Color32) {
    let points = input.polyline();
    if points.is_empty() {
        return;
    }
    let mut line: Vec<Pos2> = points.iter().map(|p| view.to_screen(p.x, p.y)).collect();
    if let Some((x, y)) = input.hover {
        line.push(view.to_screen(x, y));
    }
    painter.add(EguiShape::line(line, Stroke::new(2.0, accent)));
    for p in points {
        painter.circle_filled(view.to_screen(p.x, p.y), 3.0, accent);
    }
}
