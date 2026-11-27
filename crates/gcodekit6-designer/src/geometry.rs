//! Shared 2D geometry helpers for the designer.
//!
//! One owner for the small polygon primitives that the shapes, pocket generators,
//! toolpath generator and G-code optimiser all need: point-to-segment distance,
//! even-odd point-in-polygon, closed-polyline preparation for offsetting, and a
//! panic-safe polyline offset.
//!
//! Tolerances stay with the caller: every helper whose callers used different
//! thresholds takes that threshold as a parameter, so each call site keeps its
//! exact numeric behaviour.
//!
//! Dependencies: `cavalier_contours` for [`Polyline`] and its offset algorithm.

use std::panic;

use cavalier_contours::polyline::{PlineSource, PlineSourceMut, PlineVertex, Polyline};

use crate::model::Point;

/// Distance (mm) from `p` to the segment `a`-`b`.
///
/// # Arguments
/// * `p` - The query point.
/// * `a`, `b` - The segment end points.
/// * `min_len_sq` - Squared segment length below which the segment is treated as the
///   single point `a`. A zero-length segment is always treated that way; pass `0.0`
///   for no additional threshold.
///
/// # Returns
/// The Euclidean distance from `p` to the nearest point of the segment.
pub fn point_segment_distance(p: &Point, a: &Point, b: &Point, min_len_sq: f64) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len_sq = dx * dx + dy * dy;
    if len_sq == 0.0 || len_sq < min_len_sq {
        return p.distance_to(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq).clamp(0.0, 1.0);
    p.distance_to(&Point::new(a.x + t * dx, a.y + t * dy))
}

/// Returns true if a ray cast from `p` towards +X crosses the edge `a`-`b`.
///
/// The edge's x-intercept is computed from `a`, so callers that need bit-identical
/// results keep their own choice of which end point is `a`.
///
/// # Arguments
/// * `p` - The ray origin.
/// * `a`, `b` - The edge end points.
pub fn ray_crosses_edge(p: &Point, a: &Point, b: &Point) -> bool {
    (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x
}

/// Even-odd ray-casting test: returns true if `p` lies inside the closed polygon `poly`.
///
/// # Arguments
/// * `p` - The query point.
/// * `poly` - The polygon vertices; the closing edge is implicit. An empty slice
///   contains nothing.
pub fn point_in_polygon(p: &Point, poly: &[Point]) -> bool {
    let Some(mut j) = poly.len().checked_sub(1) else {
        return false;
    };
    let mut inside = false;
    for i in 0..poly.len() {
        if ray_crosses_edge(p, &poly[i], &poly[j]) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Prepares a polygon for offsetting: removes duplicates and enforces CW orientation.
///
/// Degenerate input (fewer than three vertices or zero area) is still returned as a
/// closed polyline; use [`to_pline`] to reject it instead.
///
/// # Arguments
/// * `vertices` - The polygon vertices, optionally with a repeated closing vertex.
/// * `tolerance` - Distance (mm) within which consecutive vertices are duplicates. A
///   single closing vertex closer than this to the first vertex is dropped.
///
/// # Returns
/// A closed, clockwise (or degenerate) polyline with zero bulges.
pub fn prepare_polygon(vertices: &[Point], tolerance: f64) -> Polyline {
    let mut clean_vertices = dedupe_consecutive(vertices, tolerance);

    if clean_vertices.len() > 1
        && let (Some(first), Some(last)) = (clean_vertices.first(), clean_vertices.last())
        && last.distance_to(first) < tolerance
    {
        clean_vertices.pop();
    }

    if shoelace_sum(&clean_vertices) > 0.0 {
        clean_vertices.reverse();
    }
    closed_polyline(clean_vertices)
}

/// Builds a closed polyline from `points` with the requested orientation.
///
/// # Arguments
/// * `points` - The loop vertices, optionally with repeated closing vertices.
/// * `ccw` - True for counter-clockwise output, false for clockwise.
/// * `tolerance` - Distance (mm) within which consecutive vertices are duplicates.
///   Every trailing vertex within this distance of the first vertex is dropped.
///
/// # Returns
/// `None` for degenerate input (fewer than three distinct vertices or zero area).
pub fn to_pline(points: &[Point], ccw: bool, tolerance: f64) -> Option<Polyline> {
    let mut clean = dedupe_consecutive(points, tolerance);
    while clean.len() > 1
        && clean
            .first()
            .zip(clean.last())
            .is_some_and(|(f, l)| f.distance_to(l) <= tolerance)
    {
        clean.pop();
    }
    if clean.len() < 3 {
        return None;
    }

    let signed_area = shoelace_sum(&clean) * 0.5;
    if signed_area.abs() < 1e-9 {
        return None;
    }
    if (signed_area > 0.0) != ccw {
        clean.reverse();
    }
    Some(closed_polyline(clean))
}

/// Offsets `pline` by `distance`, catching a panic inside cavalier_contours.
///
/// # Arguments
/// * `pline` - The polyline to offset.
/// * `distance` - The signed offset distance in mm.
///
/// # Returns
/// The offset polylines, or an empty `Vec` (logged at warn) when the offset
/// algorithm panics.
pub fn safe_parallel_offset(pline: &Polyline, distance: f64) -> Vec<Polyline> {
    panic::catch_unwind(panic::AssertUnwindSafe(|| pline.parallel_offset(distance))).unwrap_or_else(
        |_| {
            tracing::warn!(distance, "Polyline offset failed; using an empty result");
            Vec::new()
        },
    )
}

/// Drops every vertex within `tolerance` of the previously kept vertex.
fn dedupe_consecutive(points: &[Point], tolerance: f64) -> Vec<Point> {
    let mut clean: Vec<Point> = Vec::with_capacity(points.len());
    for p in points {
        if clean
            .last()
            .is_none_or(|last: &Point| last.distance_to(p) > tolerance)
        {
            clean.push(*p);
        }
    }
    clean
}

/// Twice the signed area of the closed loop `points` (positive when counter-clockwise).
fn shoelace_sum(points: &[Point]) -> f64 {
    (0..points.len())
        .map(|i| {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
}

/// Builds a closed, zero-bulge polyline through `points`.
fn closed_polyline(points: Vec<Point>) -> Polyline {
    let mut pline = Polyline::new();
    for p in points {
        pline.add_vertex(PlineVertex::new(p.x, p.y, 0.0));
    }
    pline.set_is_closed(true);
    pline
}
