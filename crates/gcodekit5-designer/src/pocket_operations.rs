//! Pocket operations for CAM toolpath generation.
//!
//! Implements pocket milling operations with island detection and offset path generation.
//! Supports outline pocket and island preservation. Regions with polygonal holes are
//! handled by the `holes` submodule
//! ([`PocketGenerator::generate_polygon_pocket_with_holes`]).

mod holes;

use super::toolpath::{Toolpath, ToolpathSegment, ToolpathSegmentType};
use crate::geometry::{prepare_polygon, safe_parallel_offset, to_pline};
use crate::model::{DesignCircle as Circle, DesignRectangle as Rectangle, Point};
use crate::ops::clean_polyline;
use cavalier_contours::polyline::Polyline;
use std::f64::consts::PI;

/// The smallest stepover and step-down (mm) the generators use. A smaller (or non-finite)
/// value would make the offset and Z-pass loops run for millions of iterations, so it is
/// raised to this; sane inputs are unchanged.
const MIN_STEP_MM: f64 = 0.01;

/// Duplicate-vertex tolerance (mm) when preparing a polygon pocket boundary.
const POLYGON_DUPLICATE_TOLERANCE: f64 = 0.01;
/// Safety cap on the number of offset levels generated for one region (shared with the
/// with-holes generator).
const MAX_OFFSET_LEVELS: usize = 100_000;

/// The Z step of each pass: `step_down` (at least [`MIN_STEP_MM`]), or the whole depth in
/// one pass when `step_down` is not positive.
fn z_step(step_down: f64, total_depth: f64) -> f64 {
    if step_down > 0.0 {
        step_down.max(MIN_STEP_MM)
    } else {
        total_depth
    }
}

fn add_center_cleanup(
    toolpath: &mut Toolpath,
    center: Point,
    tool_diameter: f64,
    depth: f64,
    feed_rate: f64,
    spindle_speed: u32,
) {
    let radius = (tool_diameter * 0.5).max(0.25);
    // Cross pattern that passes through the exact center to clear any core.
    let cleanup_points = [
        Point::new(center.x + radius, center.y),
        center,
        Point::new(center.x - radius, center.y),
        center,
        Point::new(center.x, center.y + radius),
        Point::new(center.x, center.y - radius),
        Point::new(center.x + radius, center.y),
    ];

    toolpath.add_segment(ToolpathSegment::new(
        ToolpathSegmentType::RapidMove,
        Point::new(0.0, 0.0),
        cleanup_points[0],
        feed_rate,
        spindle_speed,
    ));

    for window in cleanup_points.windows(2) {
        let mut seg = ToolpathSegment::new(
            ToolpathSegmentType::LinearMove,
            window[0],
            window[1],
            feed_rate,
            spindle_speed,
        );
        seg.start_z = Some(depth);
        seg.z_depth = Some(depth);
        toolpath.add_segment(seg);
    }

    // Add a small circular sweep to fully wipe the center.
    let start_pt = Point::new(center.x + radius, center.y);

    // Ensure we start the loop at the right point
    let mut lead_in = ToolpathSegment::new(
        ToolpathSegmentType::LinearMove,
        cleanup_points.last().copied().unwrap_or(start_pt),
        start_pt,
        feed_rate,
        spindle_speed,
    );
    lead_in.start_z = Some(depth);
    lead_in.z_depth = Some(depth);
    toolpath.add_segment(lead_in);

    let mut current = start_pt;
    for target in [
        Point::new(center.x, center.y + radius),
        Point::new(center.x - radius, center.y),
        Point::new(center.x, center.y - radius),
        start_pt,
    ] {
        let mut seg = ToolpathSegment::new_arc(
            ToolpathSegmentType::ArcCCW,
            current,
            target,
            center,
            feed_rate,
            spindle_speed,
        );
        seg.start_z = Some(depth);
        seg.z_depth = Some(depth);
        toolpath.add_segment(seg);
        current = target;
    }
}

fn polygon_centroid(points: &[Point]) -> Option<Point> {
    if points.len() < 3 {
        return None;
    }

    let mut area = 0.0;
    let mut cx = 0.0;
    let mut cy = 0.0;

    for i in 0..points.len() - 1 {
        let p1 = points[i];
        let p2 = points[i + 1];
        let cross = p1.x * p2.y - p2.x * p1.y;
        area += cross;
        cx += (p1.x + p2.x) * cross;
        cy += (p1.y + p2.y) * cross;
    }

    if area.abs() < 1e-6 {
        return None;
    }

    area *= 0.5;
    cx /= 6.0 * area;
    cy /= 6.0 * area;
    Some(Point::new(cx, cy))
}

/// One Z pass of a pocket: the depth it cuts to and the depth of the pass before it.
#[derive(Debug, Clone, Copy)]
struct ZPass {
    /// Depth of the previous pass (the start depth for the first pass).
    prev_z: f64,
    /// Depth this pass cuts to.
    current_z: f64,
    /// True for the first pass, which approaches from safe Z.
    is_first: bool,
}

/// The vertices of an offset loop, closed by repeating the first one, or `None` when the
/// loop has fewer than two vertices.
fn closed_loop_points(path: &Polyline) -> Option<Vec<Point>> {
    let mut points: Vec<Point> = path
        .vertex_data
        .iter()
        .map(|v| Point::new(v.x, v.y))
        .collect();
    if points.len() < 2 {
        return None;
    }
    points.push(points[0]);
    Some(points)
}

/// Rotates `p` about the origin by `degrees` (counter-clockwise).
fn rotate_deg(p: Point, degrees: f64) -> Point {
    let cos_a = degrees.to_radians().cos();
    let sin_a = degrees.to_radians().sin();
    Point::new(p.x * cos_a - p.y * sin_a, p.x * sin_a + p.y * cos_a)
}

/// The `(min, max)` Y of `points`, or `None` when there are none.
fn y_extent(points: &[Point]) -> Option<(f64, f64)> {
    let first = points.first()?.y;
    Some(points.iter().fold((first, first), |(min_y, max_y), p| {
        (
            if p.y < min_y { p.y } else { min_y },
            if p.y > max_y { p.y } else { max_y },
        )
    }))
}

/// The X spans of the scanline `y` inside `polygon`, each inset by `tool_radius` at both ends;
/// spans too short to hold the tool are dropped. Spans run left to right, or right to left
/// (in reverse order) when `reverse` is set.
fn raster_spans(polygon: &[Point], y: f64, tool_radius: f64, reverse: bool) -> Vec<(f64, f64)> {
    let mut intersections = Vec::new();
    for (i, &p1) in polygon.iter().enumerate() {
        let p2 = polygon[(i + 1) % polygon.len()];
        if ((p1.y <= y && p2.y > y) || (p2.y <= y && p1.y > y)) && (p2.y - p1.y).abs() > 1e-9 {
            intersections.push(p1.x + (y - p1.y) * (p2.x - p1.x) / (p2.y - p1.y));
        }
    }
    intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mut spans: Vec<(f64, f64)> = intersections
        .as_chunks::<2>()
        .0
        .iter()
        .map(|[x_start, x_end]| (x_start + tool_radius, x_end - tool_radius))
        .filter(|(start_x, end_x)| start_x < end_x)
        .collect();
    if reverse {
        spans.reverse();
        for span in &mut spans {
            *span = (span.1, span.0);
        }
    }
    spans
}

/// Strategy for pocket milling.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, Default)]
pub enum PocketStrategy {
    /// Zig-Zag or Raster milling.
    Raster { angle: f64, bidirectional: bool },
    /// Contour-parallel (offset) milling.
    #[default]
    ContourParallel,
    /// Adaptive clearing (trochoidal-like).
    Adaptive,
}

/// Represents a pocket operation configuration.
#[derive(Debug, Clone)]
pub struct PocketOperation {
    pub id: String,
    pub depth: f64,
    pub start_depth: f64,
    pub tool_diameter: f64,
    pub stepover: f64,
    pub feed_rate: f64,
    pub spindle_speed: u32,
    pub climb_milling: bool,
    pub strategy: PocketStrategy,
    pub ramp_angle: f64,
    pub raster_fill_ratio: f64,
}

impl PocketOperation {
    /// Creates a new pocket operation with default parameters.
    pub fn new(id: String, depth: f64, tool_diameter: f64) -> Self {
        debug_assert!(depth.is_finite(), "depth must be finite, got {depth}");
        debug_assert!(
            tool_diameter.is_finite() && tool_diameter > 0.0,
            "tool_diameter must be positive and finite, got {tool_diameter}"
        );
        Self {
            id,
            depth,
            start_depth: 0.0,
            tool_diameter,
            stepover: tool_diameter / 2.0,
            feed_rate: 100.0,
            spindle_speed: 10000,
            climb_milling: false,
            strategy: PocketStrategy::ContourParallel,
            ramp_angle: 0.0,
            raster_fill_ratio: 0.5,
        }
    }

    /// Sets the start depth (top of stock).
    pub fn set_start_depth(&mut self, start_depth: f64) {
        self.start_depth = start_depth;
    }

    /// Sets the cutting parameters for this pocket operation.
    pub fn set_parameters(&mut self, stepover: f64, feed_rate: f64, spindle_speed: u32) {
        debug_assert!(
            stepover.is_finite() && stepover > 0.0,
            "stepover must be positive and finite, got {stepover}"
        );
        debug_assert!(
            feed_rate.is_finite() && feed_rate > 0.0,
            "feed_rate must be positive and finite, got {feed_rate}"
        );
        self.stepover = stepover;
        self.feed_rate = feed_rate;
        self.spindle_speed = spindle_speed;
    }

    /// Enables or disables climb milling.
    pub fn set_climb_milling(&mut self, enable: bool) {
        self.climb_milling = enable;
    }

    /// Sets the pocket strategy.
    pub fn set_strategy(&mut self, strategy: PocketStrategy) {
        self.strategy = strategy;
    }

    /// Sets the ramp angle in degrees.
    pub fn set_ramp_angle(&mut self, angle: f64) {
        debug_assert!(
            angle.is_finite() && (0.0..=90.0).contains(&angle),
            "ramp_angle must be 0..90 degrees, got {angle}"
        );
        self.ramp_angle = angle.clamp(0.0, 90.0);
    }

    /// Sets raster fill percentage where 100 keeps full strokes and 0 removes them.
    pub fn set_raster_fill_percent(&mut self, percent: f64) {
        self.raster_fill_ratio = (percent / 100.0).clamp(0.0, 1.0);
    }

    /// Calculates the offset distance for the given pass number.
    pub fn calculate_offset(&self, pass: u32) -> f64 {
        self.stepover * pass as f64
    }
}

/// Represents an island within a pocket.
#[derive(Debug, Clone)]
pub struct Island {
    pub center: Point,
    pub radius: f64,
}

impl Island {
    /// Creates a new island.
    pub fn new(center: Point, radius: f64) -> Self {
        Self { center, radius }
    }

    /// Checks if a point is inside the island.
    pub fn contains_point(&self, point: &Point) -> bool {
        self.center.distance_to(point) <= self.radius
    }
}

/// Generates pocket toolpaths with island detection.
pub struct PocketGenerator {
    pub operation: PocketOperation,
    pub islands: Vec<Island>,
}

impl PocketGenerator {
    /// The stepover the generators step by: the operation's, at least [`MIN_STEP_MM`].
    fn stepover(&self) -> f64 {
        self.operation.stepover.max(MIN_STEP_MM)
    }

    /// Creates a new pocket generator.
    pub fn new(operation: PocketOperation) -> Self {
        Self {
            operation,
            islands: Vec::new(),
        }
    }

    /// Adds an island to the pocket.
    pub fn add_island(&mut self, island: Island) {
        self.islands.push(island);
    }

    /// Adds a circular island.
    pub fn add_circular_island(&mut self, center: Point, radius: f64) {
        self.add_island(Island::new(center, radius));
    }

    /// Clears all islands.
    pub fn clear_islands(&mut self) {
        self.islands.clear();
    }

    /// Checks if a point is in any island.
    fn is_in_island(&self, point: &Point) -> bool {
        self.islands
            .iter()
            .any(|island| island.contains_point(point))
    }

    fn add_helical_ramp(&self, toolpath: &mut Toolpath, center: Point, start_z: f64, end_z: f64) {
        let radius = self.operation.tool_diameter / 4.0;
        let ramp_angle_rad = self.operation.ramp_angle.to_radians();
        let z_drop_per_rev = 2.0 * PI * radius * ramp_angle_rad.tan();

        if z_drop_per_rev < 0.001 {
            // Angle too shallow or radius too small, just plunge
            // Rapid to center
            toolpath.add_segment(ToolpathSegment::new(
                ToolpathSegmentType::RapidMove,
                Point::new(0.0, 0.0),
                center,
                self.operation.feed_rate,
                self.operation.spindle_speed,
            ));
            // Plunge
            let mut plunge = ToolpathSegment::new(
                ToolpathSegmentType::LinearMove,
                center,
                center,
                self.operation.feed_rate / 2.0,
                self.operation.spindle_speed,
            );
            plunge.start_z = Some(start_z);
            plunge.z_depth = Some(end_z);
            toolpath.add_segment(plunge);
            return;
        }

        let total_drop = start_z - end_z; // Positive
        let revs = (total_drop / z_drop_per_rev).ceil() as u32;
        let actual_drop_per_rev = total_drop / revs as f64;

        // Start point of helix
        let start_pt = Point::new(center.x + radius, center.y);

        // Rapid to start of helix at start_z
        let mut rapid = ToolpathSegment::new(
            ToolpathSegmentType::RapidMove,
            Point::new(0.0, 0.0),
            start_pt,
            self.operation.feed_rate,
            self.operation.spindle_speed,
        );
        rapid.z_depth = Some(start_z);
        toolpath.add_segment(rapid);

        let mut current_z = start_z;
        let mut current_pt = start_pt;

        for _ in 0..revs {
            let next_z = current_z - actual_drop_per_rev;

            // Full circle arc (broken into 2 halves or handled by G-code gen?)
            // G-code gen handles full circle if start != end? No.
            // If start == end, it might be full circle.
            // But here we are spiraling, so Z changes.
            // Let's do 4 quadrants to be safe and consistent.

            let p1 = Point::new(center.x, center.y + radius);
            let p2 = Point::new(center.x - radius, center.y);
            let p3 = Point::new(center.x, center.y - radius);
            let p4 = Point::new(center.x + radius, center.y);

            let drop_per_quad = actual_drop_per_rev / 4.0;

            let points = [p1, p2, p3, p4];
            for (i, &target) in points.iter().enumerate() {
                let target_z = current_z - drop_per_quad * (i as f64 + 1.0);

                let mut arc = ToolpathSegment::new_arc(
                    ToolpathSegmentType::ArcCCW,
                    current_pt,
                    target,
                    center,
                    self.operation.feed_rate,
                    self.operation.spindle_speed,
                );
                arc.start_z = Some(current_z - drop_per_quad * i as f64);
                arc.z_depth = Some(target_z);
                toolpath.add_segment(arc);
                current_pt = target;
            }
            current_z = next_z;
        }

        // Move to center at bottom
        let mut to_center = ToolpathSegment::new(
            ToolpathSegmentType::LinearMove,
            current_pt,
            center,
            self.operation.feed_rate,
            self.operation.spindle_speed,
        );
        to_center.start_z = Some(end_z);
        to_center.z_depth = Some(end_z);
        toolpath.add_segment(to_center);
    }

    /// Generates a pocket toolpath for a rectangular outline.
    pub fn generate_rectangular_pocket(&self, rect: &Rectangle, step_down: f64) -> Vec<Toolpath> {
        if let PocketStrategy::ContourParallel = self.operation.strategy {
            let mut toolpaths = Vec::new();

            let half_tool = self.operation.tool_diameter / 2.0;
            let max_offset = rect.width.min(rect.height) / 2.0;

            // Calculate Z passes
            let total_depth = self.operation.depth.abs();
            let z_step = z_step(step_down, total_depth);
            let z_passes = (total_depth / z_step).ceil() as u32;
            let mut prev_z = self.operation.start_depth;
            let mut is_first_z_pass = true;

            for z_pass in 1..=z_passes {
                let current_z =
                    self.operation.start_depth - (z_step * z_pass as f64).min(total_depth);
                let mut toolpath = Toolpath::new(self.operation.tool_diameter, current_z);

                // Start from the outside (boundary) and work inwards
                let mut current_offset = half_tool;
                let mut is_first_stepin = true;

                while current_offset < max_offset {
                    let inset_x = (rect.center.x - rect.width / 2.0) + current_offset;
                    let inset_y = (rect.center.y - rect.height / 2.0) + current_offset;
                    let inset_width = (rect.width - 2.0 * current_offset).max(0.0);
                    let inset_height = (rect.height - 2.0 * current_offset).max(0.0);

                    if inset_width <= 0.0 || inset_height <= 0.0 {
                        break;
                    }

                    let points = [
                        Point::new(inset_x, inset_y),
                        Point::new(inset_x + inset_width, inset_y),
                        Point::new(inset_x + inset_width, inset_y + inset_height),
                        Point::new(inset_x, inset_y + inset_height),
                        Point::new(inset_x, inset_y),
                    ];

                    // Move to start of this contour loop
                    if let Some(first_point) = points.first() {
                        if self.operation.ramp_angle > 0.0 {
                            self.add_helical_ramp(&mut toolpath, *first_point, prev_z, current_z);
                        } else if is_first_z_pass && is_first_stepin {
                            // Very first step-in of first Z pass: use RapidMove to approach from safe Z
                            toolpath.add_segment(ToolpathSegment::new(
                                ToolpathSegmentType::RapidMove,
                                Point::new(0.0, 0.0),
                                *first_point,
                                self.operation.feed_rate,
                                self.operation.spindle_speed,
                            ));
                            is_first_stepin = false;
                        } else {
                            // Subsequent step-ins or Z passes: use LinearMove at current depth
                            let last_pt = toolpath
                                .segments
                                .last()
                                .map(|s| s.end)
                                .unwrap_or(*first_point);
                            toolpath.add_segment(ToolpathSegment::new(
                                ToolpathSegmentType::LinearMove,
                                last_pt,
                                *first_point,
                                self.operation.feed_rate,
                                self.operation.spindle_speed,
                            ));
                            is_first_stepin = false;
                        }
                    }

                    for window in points.windows(2) {
                        let start = window[0];
                        let end = window[1];

                        if !self.is_in_island(&start) && !self.is_in_island(&end) {
                            let segment = ToolpathSegment::new(
                                ToolpathSegmentType::LinearMove,
                                start,
                                end,
                                self.operation.feed_rate,
                                self.operation.spindle_speed,
                            );
                            toolpath.add_segment(segment);
                        }
                    }

                    current_offset += self.stepover();
                }
                toolpaths.push(toolpath);
                prev_z = current_z;
                is_first_z_pass = false;
            }

            toolpaths
        } else {
            // Convert to polygon and use generic generator
            let vertices = vec![
                Point::new(
                    rect.center.x - rect.width / 2.0,
                    rect.center.y - rect.height / 2.0,
                ),
                Point::new(
                    (rect.center.x - rect.width / 2.0) + rect.width,
                    rect.center.y - rect.height / 2.0,
                ),
                Point::new(
                    (rect.center.x - rect.width / 2.0) + rect.width,
                    (rect.center.y - rect.height / 2.0) + rect.height,
                ),
                Point::new(
                    rect.center.x - rect.width / 2.0,
                    (rect.center.y - rect.height / 2.0) + rect.height,
                ),
            ];
            self.generate_polygon_pocket(&vertices, step_down)
        }
    }

    /// Generates a pocket toolpath for a circular outline.
    pub fn generate_circular_pocket(&self, circle: &Circle, step_down: f64) -> Vec<Toolpath> {
        if let PocketStrategy::ContourParallel = self.operation.strategy {
            let mut toolpaths = Vec::new();

            let half_tool = self.operation.tool_diameter / 2.0;
            let max_offset = circle.radius;

            // Calculate Z passes
            let total_depth = self.operation.depth.abs();
            let z_step = z_step(step_down, total_depth);
            let z_passes = (total_depth / z_step).ceil() as u32;
            let mut prev_z = self.operation.start_depth;
            let mut is_first_z_pass = true;

            for z_pass in 1..=z_passes {
                let current_z =
                    self.operation.start_depth - (z_step * z_pass as f64).min(total_depth);
                let mut toolpath = Toolpath::new(self.operation.tool_diameter, current_z);

                let segments = 36;
                let mut current_offset = half_tool;
                let mut is_first_stepin = true;

                while current_offset < max_offset {
                    let inset_radius = circle.radius - current_offset;
                    if inset_radius <= 0.0 {
                        break;
                    }

                    // Calculate start point of this contour
                    let start_angle: f64 = 0.0;
                    let start_x = circle.center.x + inset_radius * start_angle.cos();
                    let start_y = circle.center.y + inset_radius * start_angle.sin();
                    let start_pt = Point::new(start_x, start_y);

                    if self.operation.ramp_angle > 0.0 {
                        self.add_helical_ramp(&mut toolpath, start_pt, prev_z, current_z);
                    } else if is_first_z_pass && is_first_stepin {
                        // Very first step-in of first Z pass: use RapidMove to approach from safe Z
                        toolpath.add_segment(ToolpathSegment::new(
                            ToolpathSegmentType::RapidMove,
                            Point::new(0.0, 0.0),
                            start_pt,
                            self.operation.feed_rate,
                            self.operation.spindle_speed,
                        ));
                        is_first_stepin = false;
                    } else {
                        // Subsequent step-ins or subsequent Z passes: use LinearMove
                        // (the G-code generator will handle the Z plunge based on toolpath.depth)
                        let last_pt = toolpath.segments.last().map(|s| s.end).unwrap_or(start_pt);
                        toolpath.add_segment(ToolpathSegment::new(
                            ToolpathSegmentType::LinearMove,
                            last_pt,
                            start_pt,
                            self.operation.feed_rate,
                            self.operation.spindle_speed,
                        ));
                        is_first_stepin = false;
                    }

                    for i in 0..segments {
                        let angle1 = (i as f64 / segments as f64) * 2.0 * PI;
                        let angle2 = ((i + 1) as f64 / segments as f64) * 2.0 * PI;

                        let x1 = circle.center.x + inset_radius * angle1.cos();
                        let y1 = circle.center.y + inset_radius * angle1.sin();
                        let x2 = circle.center.x + inset_radius * angle2.cos();
                        let y2 = circle.center.y + inset_radius * angle2.sin();

                        let start = Point::new(x1, y1);
                        let end = Point::new(x2, y2);

                        if !self.is_in_island(&start) && !self.is_in_island(&end) {
                            let segment = ToolpathSegment::new(
                                ToolpathSegmentType::LinearMove,
                                start,
                                end,
                                self.operation.feed_rate,
                                self.operation.spindle_speed,
                            );
                            toolpath.add_segment(segment);
                        }
                    }

                    current_offset += self.stepover();
                }
                // Final center cleanup to eliminate residual core (only if raster fill enabled)
                if self.operation.raster_fill_ratio > 0.0 {
                    add_center_cleanup(
                        &mut toolpath,
                        circle.center,
                        self.operation.tool_diameter,
                        current_z,
                        self.operation.feed_rate,
                        self.operation.spindle_speed,
                    );
                }
                toolpaths.push(toolpath);
                prev_z = current_z;
                is_first_z_pass = false;
            }

            toolpaths
        } else {
            // Convert to polygon (approximate circle)
            let segments = 64;
            let mut vertices = Vec::with_capacity(segments);
            for i in 0..segments {
                let angle = (i as f64 / segments as f64) * 2.0 * PI;
                let x = circle.center.x + circle.radius * angle.cos();
                let y = circle.center.y + circle.radius * angle.sin();
                vertices.push(Point::new(x, y));
            }
            self.generate_polygon_pocket(&vertices, step_down)
        }
    }

    /// Generates a pocket toolpath for a polygon defined by vertices.
    pub fn generate_polygon_pocket(&self, vertices: &[Point], step_down: f64) -> Vec<Toolpath> {
        match self.operation.strategy {
            PocketStrategy::Raster {
                angle,
                bidirectional,
            } => self.generate_raster_pocket(vertices, step_down, angle, bidirectional),
            PocketStrategy::ContourParallel => {
                self.generate_contour_parallel_pocket(vertices, step_down)
            }
            PocketStrategy::Adaptive => self.generate_adaptive_pocket(vertices, step_down),
        }
    }

    fn generate_contour_parallel_pocket(
        &self,
        vertices: &[Point],
        step_down: f64,
    ) -> Vec<Toolpath> {
        if vertices.is_empty() {
            return Vec::new();
        }
        let levels = self.polygon_offset_levels(vertices);

        self.toolpath_per_z_pass(step_down, |toolpath, pass| {
            let mut last_loop_centroid = None;
            let mut is_first_stepin = true;
            // Outside-in: every loop of every offset level, entering each one separately.
            for points in levels.iter().flatten().filter_map(closed_loop_points) {
                last_loop_centroid = polygon_centroid(&points);
                self.add_entry_move(toolpath, points[0], pass, pass.is_first && is_first_stepin);
                is_first_stepin = false;
                self.add_loop_cut(toolpath, &points);
            }
            self.add_center_cleanup_if_enabled(toolpath, last_loop_centroid, pass.current_z);
        })
    }

    fn generate_adaptive_pocket(&self, vertices: &[Point], step_down: f64) -> Vec<Toolpath> {
        if vertices.is_empty() {
            return Vec::new();
        }
        // Inside-out: the innermost level first.
        let mut levels = self.polygon_offset_levels(vertices);
        levels.reverse();

        let innermost = levels.first().and_then(|level| level.first());
        let innermost_centroid = innermost
            .and_then(closed_loop_points)
            .and_then(|points| polygon_centroid(&points));
        let entry_point = innermost
            .and_then(|path| path.vertex_data.first())
            .map(|v| Point::new(v.x, v.y));

        self.toolpath_per_z_pass(step_down, |toolpath, pass| {
            if let Some(start_pt) = entry_point {
                self.add_entry_move(toolpath, start_pt, pass, pass.is_first);
            }
            for points in levels.iter().flatten().filter_map(closed_loop_points) {
                // The tool stays at cutting depth; traverse only when not already at the start.
                let start_pt = points[0];
                if let Some(last_pt) = toolpath.segments.last().map(|s| s.end)
                    && last_pt.distance_to(&start_pt) > 0.1
                {
                    self.add_feed_move(toolpath, last_pt, start_pt);
                }
                self.add_loop_cut(toolpath, &points);
            }
            self.add_center_cleanup_if_enabled(toolpath, innermost_centroid, pass.current_z);
        })
    }

    fn generate_raster_pocket(
        &self,
        vertices: &[Point],
        step_down: f64,
        angle: f64,
        bidirectional: bool,
    ) -> Vec<Toolpath> {
        // Work in a frame rotated so the raster lines run along X.
        let rotated_vertices: Vec<Point> =
            vertices.iter().map(|&p| rotate_deg(p, -angle)).collect();
        let Some((min_y, max_y)) = y_extent(&rotated_vertices) else {
            return Vec::new();
        };
        let tool_radius = self.operation.tool_diameter / 2.0;

        self.toolpath_per_z_pass(step_down, |toolpath, pass| {
            if rotated_vertices.len() < 3 {
                return;
            }
            let mut current_y = min_y + tool_radius;
            let limit_y = max_y - tool_radius;
            let mut forward = true;

            while current_y <= limit_y {
                let reverse = bidirectional && !forward;
                for (start_x, end_x) in
                    raster_spans(&rotated_vertices, current_y, tool_radius, reverse)
                {
                    let start_pt = rotate_deg(Point::new(start_x, current_y), angle);
                    let end_pt = rotate_deg(Point::new(end_x, current_y), angle);
                    self.add_raster_span(toolpath, start_pt, end_pt, pass);
                }
                current_y += self.stepover();
                forward = !forward;
            }
        })
    }

    /// Computes the Z level of each pass: `step_down` steps from the start depth down to the
    /// full depth, the last pass clamped to it.
    fn z_levels(&self, step_down: f64) -> Vec<f64> {
        let total_depth = self.operation.depth.abs();
        let z_step = z_step(step_down, total_depth);
        if z_step <= 0.0 {
            return Vec::new();
        }
        let z_passes = (total_depth / z_step).ceil() as u32;
        (1..=z_passes)
            .map(|z_pass| self.operation.start_depth - (z_step * z_pass as f64).min(total_depth))
            .collect()
    }

    /// Builds one toolpath per Z pass, letting `cut` fill each one at its depth.
    fn toolpath_per_z_pass(
        &self,
        step_down: f64,
        mut cut: impl FnMut(&mut Toolpath, ZPass),
    ) -> Vec<Toolpath> {
        let mut prev_z = self.operation.start_depth;
        self.z_levels(step_down)
            .into_iter()
            .enumerate()
            .map(|(index, current_z)| {
                let pass = ZPass {
                    prev_z,
                    current_z,
                    is_first: index == 0,
                };
                let mut toolpath = Toolpath::new(self.operation.tool_diameter, current_z);
                cut(&mut toolpath, pass);
                prev_z = current_z;
                toolpath
            })
            .collect()
    }

    /// Generates successive inward offsets of a polygon boundary, starting one tool radius
    /// in and stepping by the stepover, until an offset vanishes (outermost level first).
    fn polygon_offset_levels(&self, vertices: &[Point]) -> Vec<Vec<Polyline>> {
        // A polygon with fewer than 3 distinct vertices or no area has nothing to pocket. Its
        // inward offset never comes back empty, so without this the loop below never ended.
        if to_pline(vertices, true, POLYGON_DUPLICATE_TOLERANCE).is_none() {
            return Vec::new();
        }
        let polyline = clean_polyline(prepare_polygon(vertices, POLYGON_DUPLICATE_TOLERANCE));
        let mut levels = Vec::new();
        let mut current_offset = self.operation.tool_diameter / 2.0;
        while levels.len() < MAX_OFFSET_LEVELS {
            let offsets = safe_parallel_offset(&polyline, -current_offset);
            if offsets.is_empty() {
                return levels;
            }
            levels.push(offsets);
            current_offset += self.stepover();
        }
        levels
    }

    /// Moves the tool onto `start` at the pass depth: a helical ramp when ramping is enabled,
    /// a rapid from safe Z when `rapid` is set, otherwise a feed move from the toolpath's last
    /// point (or from `start` itself when the toolpath is empty).
    fn add_entry_move(&self, toolpath: &mut Toolpath, start: Point, pass: ZPass, rapid: bool) {
        if self.operation.ramp_angle > 0.0 {
            self.add_helical_ramp(toolpath, start, pass.prev_z, pass.current_z);
        } else if rapid {
            toolpath.add_segment(ToolpathSegment::new(
                ToolpathSegmentType::RapidMove,
                Point::new(0.0, 0.0),
                start,
                self.operation.feed_rate,
                self.operation.spindle_speed,
            ));
        } else {
            let last_pt = toolpath.segments.last().map(|s| s.end).unwrap_or(start);
            self.add_feed_move(toolpath, last_pt, start);
        }
    }

    /// Appends a linear feed move from `from` to `to`.
    fn add_feed_move(&self, toolpath: &mut Toolpath, from: Point, to: Point) {
        toolpath.add_segment(ToolpathSegment::new(
            ToolpathSegmentType::LinearMove,
            from,
            to,
            self.operation.feed_rate,
            self.operation.spindle_speed,
        ));
    }

    /// Appends a feed move along each edge of the closed loop `points`.
    fn add_loop_cut(&self, toolpath: &mut Toolpath, points: &[Point]) {
        for window in points.windows(2) {
            self.add_feed_move(toolpath, window[0], window[1]);
        }
    }

    /// Clears the residual core at `center` when raster fill is enabled and a centre exists.
    fn add_center_cleanup_if_enabled(
        &self,
        toolpath: &mut Toolpath,
        center: Option<Point>,
        z: f64,
    ) {
        if let Some(center_pt) = center
            && self.operation.raster_fill_ratio > 0.0
        {
            add_center_cleanup(
                toolpath,
                center_pt,
                self.operation.tool_diameter,
                z,
                self.operation.feed_rate,
                self.operation.spindle_speed,
            );
        }
    }

    /// Cuts one raster span unless either end lies in an island: the first span of a pass
    /// enters with [`Self::add_entry_move`], later spans link from the previous end at depth.
    fn add_raster_span(&self, toolpath: &mut Toolpath, start: Point, end: Point, pass: ZPass) {
        if self.is_in_island(&start) || self.is_in_island(&end) {
            return;
        }
        match toolpath.segments.last().map(|s| s.end) {
            Some(last_pt) => self.add_feed_move(toolpath, last_pt, start),
            None => self.add_entry_move(toolpath, start, pass, pass.is_first),
        }
        self.add_feed_move(toolpath, start, end);
    }

    /// Generates offset paths for the pocket boundary.
    pub fn generate_offset_paths(&self, rect: &Rectangle, offset_count: u32) -> Vec<Vec<Point>> {
        let mut paths = Vec::new();

        for offset_idx in 1..=offset_count {
            let offset = self.operation.calculate_offset(offset_idx);
            if offset > (rect.width.min(rect.height) / 2.0) {
                break;
            }

            let inset_x = (rect.center.x - rect.width / 2.0) + offset;
            let inset_y = (rect.center.y - rect.height / 2.0) + offset;
            let inset_width = (rect.width - 2.0 * offset).max(0.0);
            let inset_height = (rect.height - 2.0 * offset).max(0.0);

            if inset_width <= 0.0 || inset_height <= 0.0 {
                break;
            }

            let path = vec![
                Point::new(inset_x, inset_y),
                Point::new(inset_x + inset_width, inset_y),
                Point::new(inset_x + inset_width, inset_y + inset_height),
                Point::new(inset_x, inset_y + inset_height),
            ];

            paths.push(path);
        }

        paths
    }
}
