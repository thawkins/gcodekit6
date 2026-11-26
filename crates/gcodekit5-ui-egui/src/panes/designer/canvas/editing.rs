//! Edits the canvas makes to the design: hit tests, snapping, creating shapes from a drag or
//! a polyline, resizing the selection by a corner handle, rotating, marquee selection and the
//! view changes (zoom at a point, fit, reset).
//!
//! Every edit goes through `DesignerState`'s undo stack. Ported from the GTK
//! `designer_canvas/editing.rs`, `input.rs` (`create_shape`, `snap_canvas_point`, the marquee)
//! and `rendering.rs` (`selection_bounds`, `get_resize_handle_at`, `apply_resize`).
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model, undo commands), `gcodekit5-core`
//! (default work area).

use gcodekit5_core::constants as core_constants;
use gcodekit5_designer::DesignerState;
use gcodekit5_designer::commands::{CompositeCommand, DesignerCommand, ResizeShape};
use gcodekit5_designer::model::{
    DesignCircle, DesignEllipse, DesignGear, DesignLine, DesignPath, DesignPolygon,
    DesignRectangle, DesignSprocket, DesignTriangle, DesignerShape, Point, Shape,
};

use super::{CanvasView, DesignerTool, ResizeHandle};

/// Bounds `(min_x, min_y, max_x, max_y)` in millimetres.
pub type Bounds = (f64, f64, f64, f64);

/// How far from a shape, in screen points, a click still hits it (GTK's 6 px).
pub const HIT_TOLERANCE_PT: f64 = 6.0;
/// Side of a resize handle, in screen points (GTK's 8 px).
pub const HANDLE_SIZE_PT: f64 = 8.0;
/// Zoom factor of one Zoom In / Zoom Out step (GTK's 1.2).
pub const ZOOM_STEP: f64 = 1.2;
/// Smallest box side (mm) a drag must have to create a shape (GTK's 1 mm).
const MIN_CREATE_MM: f64 = 1.0;
/// The zoom range (points per mm). The model's viewport accepts only values strictly inside
/// 0.1..50 and ignores others, so a zoom past a limit is clamped just inside it.
pub const ZOOM_LIMITS: (f64, f64) = (0.1 + 1e-6, 50.0 - 1e-6);
/// Smallest side (mm) a resize may leave (GTK's 5 mm).
const MIN_RESIZE_MM: f64 = 5.0;

/// The hit tolerance in millimetres at `zoom` points per millimetre.
///
/// # Arguments
/// * `zoom` - Points per millimetre.
pub fn hit_tolerance_mm(zoom: f64) -> f64 {
    HIT_TOLERANCE_PT / zoom.max(1e-6)
}

/// Rounds `(x, y)` to the snap step when snapping is on (GTK: the step is
/// `snap_threshold_mm`, 1 mm by default).
///
/// # Arguments
/// * `state` - The design, for its snap settings.
/// * `x`, `y` - A world point in millimetres.
pub fn snap(state: &DesignerState, x: f64, y: f64) -> (f64, f64) {
    let step = state.snap_threshold_mm.max(0.0);
    if !state.snap_enabled || step <= f64::EPSILON {
        return (x, y);
    }
    ((x / step).round() * step, (y / step).round() * step)
}

/// Rotates `(x, y)` by `angle` radians about `(cx, cy)`.
fn rotate_xy(x: f64, y: f64, cx: f64, cy: f64, angle: f64) -> (f64, f64) {
    let (s, c) = angle.sin_cos();
    let (dx, dy) = (x - cx, y - cy);
    (cx + dx * c - dy * s, cy + dx * s + dy * c)
}

/// The box around the corners of `(x1, y1, x2, y2)` rotated `degrees` about its centre.
fn rotated_box(b: Bounds, degrees: f64) -> Bounds {
    let (cx, cy) = ((b.0 + b.2) / 2.0, (b.1 + b.3) / 2.0);
    [(b.0, b.1), (b.2, b.1), (b.2, b.3), (b.0, b.3)]
        .into_iter()
        .map(|(x, y)| rotate_xy(x, y, cx, cy, degrees.to_radians()))
        .fold(
            (
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ),
            |a, (x, y)| (a.0.min(x), a.1.min(y), a.2.max(x), a.3.max(y)),
        )
}

/// The box the selection handles of `shape` sit on: its bounds, with a rotated rectangle,
/// ellipse or path boxed as drawn.
///
/// # Arguments
/// * `shape` - The shape.
pub fn selection_bounds(shape: &Shape) -> Bounds {
    match shape {
        Shape::Rectangle(rect) if rect.rotation.abs() > 1e-9 => {
            let (hw, hh) = (rect.width / 2.0, rect.height / 2.0);
            let (cx, cy) = (rect.center.x, rect.center.y);
            rotated_box((cx - hw, cy - hh, cx + hw, cy + hh), rect.rotation)
        }
        Shape::Ellipse(e) if e.rotation.abs() > 1e-9 => {
            let (s, c) = e.rotation.to_radians().sin_cos();
            let hw = ((e.rx * c).powi(2) + (e.ry * s).powi(2)).sqrt();
            let hh = ((e.rx * s).powi(2) + (e.ry * c).powi(2)).sqrt();
            (
                e.center.x - hw,
                e.center.y - hh,
                e.center.x + hw,
                e.center.y + hh,
            )
        }
        Shape::Path(path) if path.rotation.abs() > 1e-9 => {
            rotated_box(path.bounds(), path.rotation)
        }
        other => other.bounds(),
    }
}

/// The box of the current selection: the selected shape's, or the union of all selected
/// shapes' when there are several.
///
/// # Arguments
/// * `state` - The design.
pub fn selection_box(state: &DesignerState) -> Option<Bounds> {
    let mut selected = state.canvas.shapes().filter(|s| s.selected).peekable();
    selected.peek()?;
    let b = selected.map(|o| selection_bounds(&o.shape)).fold(
        (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ),
        |a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)),
    );
    Some(b)
}

/// The corner handle of `bounds` under world point `(x, y)`, if any.
///
/// # Arguments
/// * `x`, `y` - A world point in millimetres.
/// * `bounds` - The selection box.
/// * `zoom` - Points per millimetre, to size the handles.
pub fn handle_at(x: f64, y: f64, bounds: Bounds, zoom: f64) -> Option<ResizeHandle> {
    let half = HANDLE_SIZE_PT / zoom.max(1e-6) / 2.0;
    handle_corners(bounds)
        .into_iter()
        .find(|(cx, cy, _)| (x - cx).abs() <= half && (y - cy).abs() <= half)
        .map(|(_, _, handle)| handle)
}

/// The four corners of `bounds` with their handles.
pub fn handle_corners(b: Bounds) -> [(f64, f64, ResizeHandle); 4] {
    [
        (b.0, b.3, ResizeHandle::TopLeft),
        (b.2, b.3, ResizeHandle::TopRight),
        (b.0, b.1, ResizeHandle::BottomLeft),
        (b.2, b.1, ResizeHandle::BottomRight),
    ]
}

/// The shape `tool` makes from a drag from `start` to `end`, or `None` when the box is too
/// small (GTK's defaults: a hexagon, a 20-tooth module-2 gear, a 15-tooth ANSI 40 sprocket).
///
/// # Arguments
/// * `tool` - A tool that drags a shape.
/// * `start`, `end` - The snapped drag ends, world millimetres.
pub fn shape_from_drag(tool: DesignerTool, start: (f64, f64), end: (f64, f64)) -> Option<Shape> {
    let (w, h) = ((end.0 - start.0).abs(), (end.1 - start.1).abs());
    let center = Point::new((start.0 + end.0) / 2.0, (start.1 + end.1) / 2.0);
    let radius = w.min(h) / 2.0;
    let big = w > MIN_CREATE_MM && h > MIN_CREATE_MM;
    let round = radius > MIN_CREATE_MM;
    match tool {
        DesignerTool::Rectangle if big => Some(Shape::Rectangle(DesignRectangle::new(
            start.0.min(end.0),
            start.1.min(end.1),
            w,
            h,
        ))),
        DesignerTool::Circle if round => Some(Shape::Circle(DesignCircle::new(center, radius))),
        DesignerTool::Line if w.hypot(h) > MIN_CREATE_MM => Some(Shape::Line(DesignLine::new(
            Point::new(start.0, start.1),
            Point::new(end.0, end.1),
        ))),
        DesignerTool::Ellipse if w / 2.0 > MIN_CREATE_MM && h / 2.0 > MIN_CREATE_MM => {
            Some(Shape::Ellipse(DesignEllipse::new(center, w / 2.0, h / 2.0)))
        }
        DesignerTool::Triangle if big => {
            // The right angle sits at the corner the drag started from.
            let corner = match (end.0 >= start.0, end.1 >= start.1) {
                (true, true) => 2,
                (true, false) => 0,
                (false, true) => 3,
                (false, false) => 1,
            };
            Some(Shape::Triangle(DesignTriangle::new_with_corner(
                center, w, h, corner,
            )))
        }
        DesignerTool::Polygon if round => {
            Some(Shape::Polygon(DesignPolygon::new(center, radius, 6)))
        }
        DesignerTool::Gear if round => Some(Shape::Gear(DesignGear::new(center, 2.0, 20))),
        DesignerTool::Sprocket if round => {
            Some(Shape::Sprocket(DesignSprocket::new(center, 12.7, 15)))
        }
        _ => None,
    }
}

/// Adds the shape `tool` makes from a drag, as one undo step.
///
/// # Arguments
/// * `state` - The design.
/// * `tool` - A tool that drags a shape.
/// * `start`, `end` - The drag ends, world millimetres (snapped again here, as GTK does).
///
/// # Returns
/// The new shape's id, when the box was big enough.
pub fn create_from_drag(
    state: &mut DesignerState,
    tool: DesignerTool,
    start: (f64, f64),
    end: (f64, f64),
) -> Option<u64> {
    let start = snap(state, start.0, start.1);
    let end = snap(state, end.0, end.1);
    let shape = shape_from_drag(tool, start, end)?;
    Some(state.add_shape_with_undo(shape))
}

/// Adds a path through `points` (two or more), as one undo step.
///
/// # Arguments
/// * `state` - The design.
/// * `points` - The clicked points, world millimetres.
/// * `closed` - Whether the path closes back to its first point.
pub fn create_polyline(state: &mut DesignerState, points: &[Point], closed: bool) -> Option<u64> {
    (points.len() >= 2)
        .then(|| state.add_shape_with_undo(Shape::Path(DesignPath::from_points(points, closed))))
}

/// A resize in progress: what it started from.
#[derive(Clone, Debug)]
pub struct ResizeDrag {
    /// The handle being dragged.
    pub handle: ResizeHandle,
    /// The selection box when the drag started.
    pub bounds: Bounds,
    /// The selected shapes when the drag started, so updates do not compound.
    pub originals: Vec<(u64, Shape)>,
    /// The handle's world position when the drag started.
    pub start: (f64, f64),
}

impl ResizeDrag {
    /// Starts dragging `handle` of the selection box `bounds`.
    ///
    /// # Arguments
    /// * `state` - The design.
    /// * `handle` - The grabbed handle.
    /// * `bounds` - The selection box.
    pub fn new(state: &DesignerState, handle: ResizeHandle, bounds: Bounds) -> Self {
        let start = match handle {
            ResizeHandle::TopLeft => (bounds.0, bounds.3),
            ResizeHandle::TopRight => (bounds.2, bounds.3),
            ResizeHandle::BottomLeft => (bounds.0, bounds.1),
            ResizeHandle::BottomRight => (bounds.2, bounds.1),
        };
        let originals = state
            .canvas
            .shapes()
            .filter(|s| s.selected)
            .map(|s| (s.id, s.shape.clone()))
            .collect();
        Self {
            handle,
            bounds,
            originals,
            start,
        }
    }

    /// Scales the selected shapes so the handle follows `current`, from their shapes when the
    /// drag started. With `keep_aspect` (Shift, or every selected shape's aspect lock) the
    /// dominant axis leads. A resize that would leave a side under 5 mm, or flip the box,
    /// is ignored.
    ///
    /// # Arguments
    /// * `state` - The design.
    /// * `current` - The snapped pointer position, world millimetres.
    /// * `shift` - Whether Shift is held.
    pub fn apply(&self, state: &mut DesignerState, current: (f64, f64), shift: bool) {
        let (x0, y0, x1, y1) = self.bounds;
        let (ow, oh) = (x1 - x0, y1 - y0);
        let mut dx = current.0 - self.start.0;
        let mut dy = current.1 - self.start.1;
        let left = matches!(
            self.handle,
            ResizeHandle::TopLeft | ResizeHandle::BottomLeft
        );
        let top = matches!(self.handle, ResizeHandle::TopLeft | ResizeHandle::TopRight);
        let all_locked = self.originals.iter().all(|(id, _)| {
            state
                .canvas
                .get_shape(*id)
                .is_some_and(|o| o.lock_aspect_ratio)
        }) && !self.originals.is_empty();
        if shift || all_locked {
            let ratio = if oh.abs() > 0.001 { ow / oh } else { 1.0 };
            let natural_w = if left { ow - dx } else { ow + dx };
            let natural_h = if top { oh + dy } else { oh - dy };
            let (new_w, new_h) = if (natural_w / ow).abs() > (natural_h / oh).abs() {
                (natural_w, natural_w / ratio)
            } else {
                (natural_h * ratio, natural_h)
            };
            dx = if left { ow - new_w } else { new_w - ow };
            dy = if top { new_h - oh } else { oh - new_h };
        }
        let new_w = if left { ow - dx } else { ow + dx };
        let new_h = if top { oh + dy } else { oh - dy };
        if new_w < MIN_RESIZE_MM || new_h < MIN_RESIZE_MM {
            return;
        }
        let sx = if ow.abs() > 1e-6 { new_w / ow } else { 1.0 };
        let sy = if oh.abs() > 1e-6 { new_h / oh } else { 1.0 };
        // The opposite corner stays put.
        let anchor = Point::new(if left { x1 } else { x0 }, if top { y0 } else { y1 });
        for (id, original) in &self.originals {
            if let Some(obj) = state.canvas.shape_store.get_mut(*id) {
                let mut shape = original.clone();
                scale_shape(&mut shape, sx, sy, anchor);
                obj.shape = shape;
            }
        }
    }

    /// Ends the resize: one undo step that takes every resized shape from its original to its
    /// current geometry (keeping the spatial index in step).
    ///
    /// # Arguments
    /// * `state` - The design.
    pub fn commit(self, state: &mut DesignerState) {
        let mut commands = Vec::new();
        for (id, original) in self.originals {
            let Some(obj) = state.canvas.shape_store.get_mut(id) else {
                continue;
            };
            // Put the original back: the command moves the shape and its index entry
            // from there to the resized geometry.
            let resized = std::mem::replace(&mut obj.shape, original.clone());
            if resized.bounds() == original.bounds() {
                continue;
            }
            commands.push(DesignerCommand::ResizeShape(ResizeShape {
                id,
                handle: 0,
                dx: 0.0,
                dy: 0.0,
                old_shape: Some(original),
                new_shape: Some(resized),
            }));
        }
        if !commands.is_empty() {
            state.push_command(DesignerCommand::CompositeCommand(CompositeCommand {
                commands,
                name: "Resize Shapes".to_string(),
            }));
        }
    }
}

/// Scales `(x, y)` by `(sx, sy)` about `anchor`.
fn scale_point(p: &mut Point, sx: f64, sy: f64, anchor: Point) {
    p.x = anchor.x + (p.x - anchor.x) * sx;
    p.y = anchor.y + (p.y - anchor.y) * sy;
}

/// Scales `shape` by `(sx, sy)` about `anchor` as GTK's resize does: round shapes scale by the
/// smaller factor, a path is re-placed so its box corner follows the anchor.
fn scale_shape(shape: &mut Shape, sx: f64, sy: f64, anchor: Point) {
    let s = sx.abs().min(sy.abs());
    match shape {
        Shape::Rectangle(rect) => {
            scale_point(&mut rect.center, sx, sy, anchor);
            rect.width *= sx.abs();
            rect.height *= sy.abs();
            if !rect.is_slot {
                rect.corner_radius *= s;
            }
        }
        Shape::Circle(circle) => {
            scale_point(&mut circle.center, sx, sy, anchor);
            circle.radius *= s;
        }
        Shape::Ellipse(ellipse) => {
            scale_point(&mut ellipse.center, sx, sy, anchor);
            ellipse.rx *= sx.abs();
            ellipse.ry *= sy.abs();
        }
        Shape::Line(line) => {
            scale_point(&mut line.start, sx, sy, anchor);
            scale_point(&mut line.end, sx, sy, anchor);
        }
        Shape::Path(path) => {
            let (ox, oy, _, _) = path.bounds();
            path.scale(sx, sy, anchor);
            if path.rotation.abs() <= 1e-9 {
                let (px, py, _, _) = path.bounds();
                path.translate(
                    anchor.x + (ox - anchor.x) * sx - px,
                    anchor.y + (oy - anchor.y) * sy - py,
                );
            }
        }
        Shape::Text(text) => text.scale(sx, sy, anchor),
        Shape::Triangle(triangle) => {
            scale_point(&mut triangle.center, sx, sy, anchor);
            triangle.width *= sx.abs();
            triangle.height *= sy.abs();
        }
        Shape::Polygon(polygon) => {
            scale_point(&mut polygon.center, sx, sy, anchor);
            polygon.radius *= s;
        }
        Shape::Gear(gear) => {
            scale_point(&mut gear.center, sx, sy, anchor);
            gear.module *= s;
        }
        Shape::Sprocket(sprocket) => {
            scale_point(&mut sprocket.center, sx, sy, anchor);
            sprocket.pitch *= s;
            sprocket.roller_diameter *= s;
        }
        Shape::RasterImage(raster) => {
            scale_point(&mut raster.center, sx, sy, anchor);
            raster.width_mm *= sx.abs();
            raster.height_mm *= sy.abs();
        }
    }
}

/// Rotates the selection by `delta` degrees, as one undo step: a single shape to its current
/// angle plus `delta` (normalised to -180..180), several shapes about their common centre.
///
/// # Arguments
/// * `state` - The design.
/// * `delta` - Degrees, counter-clockwise positive.
pub fn rotate_selected_by(state: &mut DesignerState, delta: f64) {
    if state.selected_count() > 1 {
        state.set_selected_rotation(delta);
        return;
    }
    let Some(current) = state
        .canvas
        .shapes()
        .find(|s| s.selected)
        .map(|s| s.shape.rotation())
    else {
        return;
    };
    let mut target = (current + delta).rem_euclid(360.0);
    if target > 180.0 {
        target -= 360.0;
    }
    state.set_selected_rotation(target);
}

/// Zooms by `factor` keeping the world point under screen point `p` where it is.
///
/// # Arguments
/// * `state` - The design, whose viewport changes.
/// * `view` - This frame's view.
/// * `p` - The screen point to keep fixed.
/// * `factor` - The zoom factor (> 1 zooms in).
pub fn zoom_at(state: &mut DesignerState, view: &CanvasView, p: egui::Pos2, factor: f64) {
    let world = view.screen_to_world(p);
    let viewport = state.canvas.viewport_mut();
    viewport.set_zoom((viewport.zoom() * factor).clamp(ZOOM_LIMITS.0, ZOOM_LIMITS.1));
    let zoom = viewport.zoom();
    let rect = view.rect();
    let pan_x = f64::from(p.x - rect.left()) - f64::from(world.x) * zoom;
    let pan_y = f64::from(rect.bottom() - p.y) - f64::from(world.y) * zoom;
    viewport.set_pan(pan_x, pan_y);
}

/// Fits every shape into the view (5% padding), or the default work area when there are
/// none (GTK: the device's area, falling back to 250 x 250 mm).
///
/// # Arguments
/// * `state` - The design.
pub fn zoom_fit(state: &mut DesignerState) {
    let bounds = state
        .canvas
        .shapes()
        .map(|o| o.get_total_bounds())
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
        .unwrap_or(work_area());
    state.canvas.fit_to_bounds(
        bounds.0,
        bounds.1,
        bounds.2,
        bounds.3,
        core_constants::VIEW_PADDING,
    );
}

/// Fits the default work area into the view (GTK's start-up view without a device).
///
/// # Arguments
/// * `state` - The design.
pub fn fit_work_area(state: &mut DesignerState) {
    let (x0, y0, x1, y1) = work_area();
    state
        .canvas
        .fit_to_bounds(x0, y0, x1, y1, core_constants::VIEW_PADDING);
}

/// The default work area: GTK's area without a device profile.
pub fn work_area() -> Bounds {
    (
        0.0,
        0.0,
        core_constants::DEFAULT_WORK_WIDTH_MM,
        core_constants::DEFAULT_WORK_HEIGHT_MM,
    )
}

/// GTK's Cut: copies the selection to the design's clipboard, then deletes it as one undo
/// step.
///
/// # Arguments
/// * `state` - The design.
pub fn cut_selected(state: &mut DesignerState) {
    state.copy_selected();
    state.delete_selected();
}
