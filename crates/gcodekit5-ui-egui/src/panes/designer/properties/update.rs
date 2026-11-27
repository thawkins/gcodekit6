//! What the properties panel shows for the current selection,
//! as a value read from the design each frame.
//!
//! One selected shape shows its own properties; several show their common box (centre and
//! size), a relative rotation, and the CAM and effect values of the first one. The
//! laser override of several shapes shows each value they share, and nothing ("---") where
//! they differ.
//!
//! Dependencies: `gcodekit5-designer` (model).

use gcodekit5_designer::DesignerState;
use gcodekit5_designer::canvas::DrawingObject;
use gcodekit5_designer::designer_state::{MachineMode, ToolSettings};
use gcodekit5_designer::model::{
    DesignerShape, LaserParams, ParametricPathSource, RasterImage, Shape,
};

/// A shape's rotation (degrees) counts as rotated above this.
const ROTATED_DEG: f64 = 0.01;

/// The laser values of the selection after resolving "use global values"; each is `None`
/// where the shapes differ.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaserSummary {
    /// Every shape uses the global values.
    pub all_use_global: bool,
    /// Some shape has its own values.
    pub any_override: bool,
    /// Feed rate, mm/min.
    pub feed_rate: Option<f64>,
    /// Power, percent.
    pub power_percent: Option<f64>,
    /// Passes.
    pub passes: Option<u32>,
}

/// `params` with the global values put in where it uses them: (feed, power %, passes).
///
/// # Arguments
/// * `params` - A shape's laser parameters.
/// * `global` - The toolbox's tool settings.
pub fn resolve_laser(params: &LaserParams, global: &ToolSettings) -> (f64, f64, u32) {
    if params.use_global {
        (
            global.feed_rate,
            f64::from(global.spindle_speed) / 10.0,
            (global.step_down as u32).max(1),
        )
    } else {
        (params.feed_rate, params.power_percent, params.passes)
    }
}

impl LaserSummary {
    /// The summary of `params`, or `None` when there are none.
    ///
    /// # Arguments
    /// * `params` - The laser parameters of the selected shapes.
    /// * `global` - The toolbox's tool settings.
    pub fn from_params(params: &[LaserParams], global: &ToolSettings) -> Option<Self> {
        let resolved: Vec<(f64, f64, u32)> =
            params.iter().map(|p| resolve_laser(p, global)).collect();
        let first = *resolved.first()?;
        let same_f64 = |get: fn(&(f64, f64, u32)) -> f64| {
            resolved
                .iter()
                .all(|r| (get(r) - get(&first)).abs() < 1e-9)
                .then_some(get(&first))
        };
        Some(Self {
            all_use_global: params.iter().all(|p| p.use_global),
            any_override: params.iter().any(|p| !p.use_global),
            feed_rate: same_f64(|r| r.0),
            power_percent: same_f64(|r| r.1),
            passes: resolved.iter().all(|r| r.2 == first.2).then_some(first.2),
        })
    }
}

/// The laser parameters embedded in `shape` (a raster image has none).
pub fn shape_laser_params(shape: &Shape) -> Option<LaserParams> {
    match shape {
        Shape::Rectangle(s) => Some(s.laser_params),
        Shape::Circle(s) => Some(s.laser_params),
        Shape::Ellipse(s) => Some(s.laser_params),
        Shape::Line(s) => Some(s.laser_params),
        Shape::Path(s) => Some(s.laser_params),
        Shape::Polygon(s) => Some(s.laser_params),
        Shape::Triangle(s) => Some(s.laser_params),
        Shape::Text(s) => Some(s.laser_params),
        Shape::Gear(s) => Some(s.laser_params),
        Shape::Sprocket(s) => Some(s.laser_params),
        Shape::RasterImage(_) => None,
    }
}

/// The timing pulley parameters of `shape` (pitch, teeth, belt width, hole radius), if it is
/// one.
pub fn timing_pulley(shape: &Shape) -> Option<(f64, usize, f64, f64)> {
    match shape {
        Shape::Path(p) => match p.parametric_source {
            Some(ParametricPathSource::TimingPulley {
                pitch,
                teeth,
                belt_width,
                hole_radius,
            }) => Some((pitch, teeth, belt_width, hole_radius)),
            _ => None,
        },
        _ => None,
    }
}

/// The selection as the panel shows it.
#[derive(Clone, Debug)]
pub struct Summary {
    /// The selected shapes, in drawing order.
    pub ids: Vec<u64>,
    /// The first selected shape; its CAM and effect values are shown.
    pub first: DrawingObject,
    /// Centre of the selection's box, mm.
    pub center: (f64, f64),
    /// Width and height shown, mm.
    pub size: (f64, f64),
    /// The rotation shown, degrees (always 0 for several shapes: it is relative).
    pub rotation: f64,
    /// Whether the machine is a 2D laser.
    pub laser_mode: bool,
    /// Whether the one selected shape is a raster image.
    pub raster: bool,
    /// Whether some selected shape is not text (the geometry operations apply).
    pub any_not_text: bool,
    /// The laser override values of the selection, if it has any.
    pub laser: Option<LaserSummary>,
}

impl Summary {
    /// The summary of `state`'s selection, or `None` when nothing is selected.
    ///
    /// # Arguments
    /// * `state` - The design.
    pub fn of(state: &DesignerState) -> Option<Self> {
        let selected: Vec<&DrawingObject> = state
            .canvas
            .shape_store
            .draw_order_iter()
            .filter_map(|id| state.canvas.get_shape(id))
            .filter(|o| o.selected)
            .collect();
        let first = light_clone(selected.first()?);
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for obj in &selected {
            let (a, b, c, d) = obj.shape.bounds();
            (x0, y0, x1, y1) = (x0.min(a), y0.min(b), x1.max(c), y1.max(d));
        }
        let single = (selected.len() == 1).then_some(&first.shape);
        let (size, rotation) = match single {
            Some(shape) => shape_size_and_rotation(shape),
            None => ((x1 - x0, y1 - y0), 0.0),
        };
        let raster = matches!(single, Some(Shape::RasterImage(_)));
        let params: Vec<LaserParams> = selected
            .iter()
            .filter_map(|o| shape_laser_params(&o.shape))
            .collect();
        Some(Self {
            ids: selected.iter().map(|o| o.id).collect(),
            center: ((x0 + x1) / 2.0, (y0 + y1) / 2.0),
            size,
            rotation,
            laser_mode: state.machine_mode() == MachineMode::Laser2D,
            raster,
            any_not_text: selected.iter().any(|o| !matches!(o.shape, Shape::Text(_))),
            laser: LaserSummary::from_params(&params, &state.tool_settings),
            first,
        })
    }

    /// The shape, when exactly one is selected (the first shape's copy, so the selection is
    /// copied once per frame, not twice).
    pub fn single(&self) -> Option<&Shape> {
        (self.ids.len() == 1).then_some(&self.first.shape)
    }

    /// Whether the one selected shape is rotated: its size fields are then read-only and
    /// its aspect ratio locked (the "Rotated object" notice).
    pub fn rotated(&self) -> bool {
        self.single().is_some() && self.rotation.abs() > ROTATED_DEG
    }

    /// Whether the one selected shape is a timing pulley.
    pub fn timing_pulley(&self) -> Option<(f64, usize, f64, f64)> {
        self.single().and_then(timing_pulley)
    }
}

/// A copy of `shape` without a raster image's pixels (the panel never reads them, and
/// copying them every frame would be slow).
fn light_shape(shape: &Shape) -> Shape {
    match shape {
        Shape::RasterImage(r) => Shape::RasterImage(RasterImage {
            image_data: Vec::new(),
            original_path: r.original_path.clone(),
            scan_direction: r.scan_direction.clone(),
            dithering: r.dithering.clone(),
            ..*r
        }),
        other => other.clone(),
    }
}

/// A copy of `obj` with [`light_shape`].
fn light_clone(obj: &DrawingObject) -> DrawingObject {
    DrawingObject {
        id: obj.id,
        group_id: obj.group_id,
        name: obj.name.clone(),
        shape: light_shape(&obj.shape),
        selected: obj.selected,
        operation_type: obj.operation_type,
        use_custom_values: obj.use_custom_values,
        pocket_depth: obj.pocket_depth,
        start_depth: obj.start_depth,
        step_down: obj.step_down,
        step_in: obj.step_in,
        ramp_angle: obj.ramp_angle,
        pocket_strategy: obj.pocket_strategy,
        raster_fill_ratio: obj.raster_fill_ratio,
        offset: obj.offset,
        fillet: obj.fillet,
        chamfer: obj.chamfer,
        lock_aspect_ratio: obj.lock_aspect_ratio,
        use_global_laser: obj.use_global_laser,
        laser_params: obj.laser_params,
    }
}

/// The size and rotation shown for one shape: the shape's own width and height where
/// it has them, otherwise its bounds (with no rotation lock).
fn shape_size_and_rotation(shape: &Shape) -> ((f64, f64), f64) {
    let (x0, y0, x1, y1) = shape.bounds();
    let bounds = (x1 - x0, y1 - y0);
    match shape {
        Shape::Rectangle(r) => ((r.width, r.height), r.rotation),
        Shape::Triangle(t) => ((t.width, t.height), t.rotation),
        Shape::Ellipse(e) => ((e.rx * 2.0, e.ry * 2.0), e.rotation),
        Shape::Circle(c) => ((c.radius * 2.0, c.radius * 2.0), 0.0),
        Shape::Polygon(p) => ((p.radius * 2.0, p.radius * 2.0), p.rotation),
        Shape::Path(p) => (bounds, p.rotation),
        Shape::RasterImage(r) => ((r.width_mm, r.height_mm), r.rotation),
        Shape::Line(_) | Shape::Text(_) | Shape::Gear(_) | Shape::Sprocket(_) => (bounds, 0.0),
    }
}

/// The rotation field's value for one shape, degrees (the shape's own angle).
pub fn shape_rotation(shape: &Shape) -> f64 {
    match shape {
        Shape::Line(l) => l.rotation,
        Shape::Text(t) => t.rotation,
        other => shape_size_and_rotation(other).1,
    }
}
