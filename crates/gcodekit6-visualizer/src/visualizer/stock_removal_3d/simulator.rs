//! [`StockSimulator3D`]: samples a toolpath along its length and cuts the
//! [`HeightMap`] at each tool position.

use glam::Vec3;

use super::heightmap::HeightMap;
use super::mesh::{MAX_MESH_FLOATS, MESH_FLOATS_PER_VERTEX, generate_surface_mesh_within};
use super::toolpath::{ToolpathSegment, ToolpathSegmentType};
use super::zero_reference::{StockXyReference, StockZeroReference};

/// Report progress at most once per this many tool positions.
const PROGRESS_STRIDE: usize = 64;

/// Tool positions sampled per cell edge length along a segment.
const SAMPLES_PER_CELL: f32 = 2.0;

/// A segment prepared for sampling, in work coordinates.
enum SegmentPath {
    Line {
        start: Vec3,
        end: Vec3,
    },
    Arc {
        center: (f32, f32),
        start: Vec3,
        end: Vec3,
        r0: f32,
        r1: f32,
        a0: f32,
        span: f32,
    },
}

impl SegmentPath {
    /// Prepare `segment`; `None` for an arc without a centre or with zero
    /// radius, which cannot be sampled.
    fn new(segment: &ToolpathSegment) -> Option<Self> {
        let start = Vec3::from(segment.start);
        let end = Vec3::from(segment.end);
        match segment.segment_type {
            ToolpathSegmentType::RapidMove | ToolpathSegmentType::LinearMove => {
                Some(Self::Line { start, end })
            }
            ToolpathSegmentType::ArcCW | ToolpathSegmentType::ArcCCW => {
                let center = segment.center?;
                let (sx, sy) = (start.x - center.0, start.y - center.1);
                let (ex, ey) = (end.x - center.0, end.y - center.1);
                let r0 = sx.hypot(sy);
                if r0.is_nan() || r0 < 0.001 {
                    return None;
                }
                let span = gcodekit6_core::gcode::arc::angular_travel(
                    (f64::from(sx), f64::from(sy)),
                    (f64::from(ex), f64::from(ey)),
                    segment.segment_type == ToolpathSegmentType::ArcCW,
                ) as f32;
                Some(Self::Arc {
                    center,
                    start,
                    end,
                    r0,
                    r1: ex.hypot(ey),
                    a0: sy.atan2(sx),
                    span,
                })
            }
        }
    }

    /// Path length (mm), including Z travel.
    fn length(&self) -> f32 {
        match *self {
            Self::Line { start, end } => start.distance(end),
            Self::Arc {
                start,
                end,
                r0,
                r1,
                span,
                ..
            } => {
                let arc = 0.5 * (r0 + r1) * span.abs();
                arc.hypot(end.z - start.z)
            }
        }
    }

    /// Lowest Z reached along the path.
    fn min_z(&self) -> f32 {
        match *self {
            Self::Line { start, end } | Self::Arc { start, end, .. } => start.z.min(end.z),
        }
    }

    /// Point at parameter `t` in `0 ..= 1`. Arcs interpolate the radius
    /// linearly so they end exactly on `end` even when the programmed start
    /// and end radii differ slightly.
    fn point(&self, t: f32) -> Vec3 {
        match *self {
            Self::Line { start, end } => start.lerp(end, t),
            Self::Arc {
                center,
                start,
                end,
                r0,
                r1,
                a0,
                span,
            } => {
                let r = r0 + (r1 - r0) * t;
                let a = a0 + span * t;
                Vec3::new(
                    center.0 + r * a.cos(),
                    center.1 + r * a.sin(),
                    start.z + (end.z - start.z) * t,
                )
            }
        }
    }
}

/// Material-removal simulator for a rectangular stock block.
///
/// See the module docs for the coordinate convention.
#[derive(Debug, Clone)]
pub struct StockSimulator3D {
    map: HeightMap,
    tool_radius: f32,
    /// Work Z (mm) of the top face of the stock.
    stock_top_z: f32,
    /// Work XY (mm) of the stock's lower-left corner.
    origin_xy: (f32, f32),
}

impl StockSimulator3D {
    /// Create a simulator whose stock top face is at work Z0 and whose
    /// lower-left corner is at work X0 Y0.
    ///
    /// # Arguments
    /// * `width`, `height`, `thickness` - Stock size in mm (X, Y, Z).
    /// * `resolution` - Cell edge length in mm.
    /// * `tool_radius` - Flat end mill radius in mm.
    ///
    /// # Errors
    /// A message when the stock is invalid or too large (see
    /// [`HeightMap::check_dimensions`]) or the tool radius is not a positive
    /// finite number.
    pub fn new(
        width: f32,
        height: f32,
        thickness: f32,
        resolution: f32,
        tool_radius: f32,
    ) -> Result<Self, String> {
        Self::validate(width, height, thickness, resolution, tool_radius)?;
        Ok(Self {
            map: HeightMap::new(width, height, thickness, resolution)?,
            tool_radius,
            stock_top_z: 0.0,
            origin_xy: (0.0, 0.0),
        })
    }

    /// Check the arguments of [`StockSimulator3D::new`] without allocating,
    /// so a UI can reject bad input before handing the (possibly large)
    /// allocation to a worker thread.
    ///
    /// # Errors
    /// The message [`StockSimulator3D::new`] would return.
    pub fn validate(
        width: f32,
        height: f32,
        thickness: f32,
        resolution: f32,
        tool_radius: f32,
    ) -> Result<(), String> {
        if !tool_radius.is_finite() || tool_radius <= 0.0 {
            return Err(format!(
                "Tool diameter must be a positive number (got {})",
                tool_radius * 2.0
            ));
        }
        HeightMap::check_dimensions(width, height, thickness, resolution)
    }

    /// Set the work Z (mm) of the top face of the stock (default 0).
    ///
    /// # Arguments
    /// * `stock_top_z` - Work Z of the stock top; use `thickness` for programs
    ///   that zero on the machine bed / bottom of the stock. Non-finite values
    ///   are ignored.
    ///
    /// # Returns
    /// The simulator, for chaining.
    pub fn with_stock_top_z(mut self, stock_top_z: f32) -> Self {
        if stock_top_z.is_finite() {
            self.stock_top_z = stock_top_z;
        }
        self
    }

    /// Place the stock for a program zeroed on the given stock face.
    ///
    /// # Arguments
    /// * `reference` - Which stock face is work Z0.
    ///
    /// # Returns
    /// The simulator, for chaining.
    pub fn with_stock_zero_reference(self, reference: StockZeroReference) -> Self {
        let top = reference.stock_top_z(self.map.thickness);
        self.with_stock_top_z(top)
    }

    /// Set the work XY (mm) of the stock's lower-left corner (default 0, 0).
    ///
    /// # Arguments
    /// * `x`, `y` - Work coordinates of the corner; use `-width / 2`,
    ///   `-height / 2` for a program zeroed on the stock centre. Non-finite
    ///   values are ignored.
    ///
    /// # Returns
    /// The simulator, for chaining.
    pub fn with_stock_origin_xy(mut self, x: f32, y: f32) -> Self {
        if x.is_finite() && y.is_finite() {
            self.origin_xy = (x, y);
        }
        self
    }

    /// Place the stock for a program whose work X0 Y0 is at `reference`.
    ///
    /// # Returns
    /// The simulator, for chaining.
    pub fn with_stock_xy_reference(self, reference: StockXyReference) -> Self {
        let (x, y) = reference.stock_origin_xy(self.map.size_x, self.map.size_y);
        self.with_stock_origin_xy(x, y)
    }

    /// Work Z (mm) of the top face of the stock.
    pub fn stock_top_z(&self) -> f32 {
        self.stock_top_z
    }

    /// Work XY (mm) of the stock's lower-left corner.
    pub fn stock_origin_xy(&self) -> (f32, f32) {
        self.origin_xy
    }

    /// The material height map, in stock-local coordinates.
    pub fn height_map(&self) -> &HeightMap {
        &self.map
    }

    /// Run the whole toolpath without progress reporting.
    pub fn simulate_toolpath(&mut self, toolpath: &[ToolpathSegment]) {
        let _ = self.simulate_toolpath_with_progress(toolpath, |_| true);
    }

    /// Cut with the tool tip at `tip` (work coordinates).
    fn remove_at_tool_tip(&mut self, tip: Vec3) {
        let stock_bottom_z = self.stock_top_z - self.map.thickness;
        self.map.cut_flat(
            tip.x - self.origin_xy.0,
            tip.y - self.origin_xy.1,
            tip.z - stock_bottom_z,
            self.tool_radius,
        );
    }

    /// Simulate material removal, reporting progress by distance travelled.
    ///
    /// Every move cuts, rapids included: a rapid through the stock removes
    /// material just as it would on the machine.
    ///
    /// # Arguments
    /// * `toolpath` - Segments in work coordinates.
    /// * `on_progress` - Called with the completed fraction (`0.0 ..= 1.0`);
    ///   return `false` to cancel.
    ///
    /// # Returns
    /// `false` if the run was cancelled.
    pub fn simulate_toolpath_with_progress<F>(
        &mut self,
        toolpath: &[ToolpathSegment],
        mut on_progress: F,
    ) -> bool
    where
        F: FnMut(f32) -> bool,
    {
        let paths: Vec<Option<(SegmentPath, f32)>> = toolpath
            .iter()
            .map(|segment| {
                let path = SegmentPath::new(segment)?;
                let length = path.length();
                length.is_finite().then_some((path, length))
            })
            .collect();
        let total: f32 = paths.iter().flatten().map(|(_, length)| length).sum();
        let total = total.max(0.001);
        let step = self.map.resolution / SAMPLES_PER_CELL;
        let mut done = 0.0;
        let mut since_report = 0;

        if !on_progress(0.0) {
            return false;
        }
        for (path, length) in paths.iter().flatten() {
            // A segment that stays at or above the stock top cannot cut.
            if path.min_z() < self.stock_top_z {
                let steps = ((length / step).ceil() as usize).max(1);
                for i in 0..=steps {
                    let t = i as f32 / steps as f32;
                    self.remove_at_tool_tip(path.point(t));
                    since_report += 1;
                    if since_report >= PROGRESS_STRIDE {
                        since_report = 0;
                        if !on_progress(((done + length * t) / total).min(1.0)) {
                            return false;
                        }
                    }
                }
            }
            done += length;
        }
        on_progress(1.0)
    }

    /// Surface mesh of the remaining stock in work coordinates.
    ///
    /// Same layout as [`super::generate_surface_mesh`] ([`MESH_FLOATS_PER_VERTEX`]
    /// floats per vertex), translated so the stock lower-left corner is at
    /// `stock_origin_xy` and its top at `stock_top_z`.
    pub fn get_mesh(&self) -> Vec<f32> {
        self.get_mesh_cancellable(&|| false).unwrap_or_default()
    }

    /// [`StockSimulator3D::get_mesh`] that stops early when `cancel` returns
    /// `true`.
    ///
    /// # Returns
    /// The mesh, or `None` when cancelled.
    pub fn get_mesh_cancellable(&self, cancel: &dyn Fn() -> bool) -> Option<Vec<f32>> {
        let mut mesh = generate_surface_mesh_within(&self.map, MAX_MESH_FLOATS, cancel)?;
        let offset = Vec3::new(
            self.origin_xy.0,
            self.origin_xy.1,
            self.stock_top_z - self.map.thickness,
        );
        if offset != Vec3::ZERO {
            for vertex in mesh.as_chunks_mut::<MESH_FLOATS_PER_VERTEX>().0 {
                vertex[0] += offset.x;
                vertex[1] += offset.y;
                vertex[2] += offset.z;
            }
        }
        Some(mesh)
    }
}
