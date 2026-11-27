//! Toolpath segment types and data structures.

use super::*;

/// Types of toolpath segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolpathSegmentType {
    RapidMove,
    LinearMove,
    ArcCW,
    ArcCCW,
}

/// A single segment of a toolpath.
#[derive(Debug, Clone)]
pub struct ToolpathSegment {
    pub segment_type: ToolpathSegmentType,
    pub start: Point,
    pub end: Point,
    pub center: Option<Point>,
    pub feed_rate: f64,
    pub spindle_speed: u32,
    /// Z depth for this segment (negative = below stock top)
    pub z_depth: Option<f64>,
    /// Start Z depth for this segment (if different from current Z)
    pub start_z: Option<f64>,
}

impl ToolpathSegment {
    /// Creates a new toolpath segment.
    pub fn new(
        segment_type: ToolpathSegmentType,
        start: Point,
        end: Point,
        feed_rate: f64,
        spindle_speed: u32,
    ) -> Self {
        Self {
            segment_type,
            start,
            end,
            center: None,
            feed_rate,
            spindle_speed,
            z_depth: None,
            start_z: None,
        }
    }

    /// Creates a new arc segment.
    pub fn new_arc(
        segment_type: ToolpathSegmentType,
        start: Point,
        end: Point,
        center: Point,
        feed_rate: f64,
        spindle_speed: u32,
    ) -> Self {
        Self {
            segment_type,
            start,
            end,
            center: Some(center),
            feed_rate,
            spindle_speed,
            z_depth: None,
            start_z: None,
        }
    }

    /// Returns the length of the path the tool travels along this segment, in mm.
    ///
    /// Linear and rapid moves return the straight-line distance. Arcs with a
    /// centre return `radius * |sweep|`, where the sweep follows the arc's
    /// direction (CCW positive); an arc whose start equals its end is a full
    /// circle, as G02/G03 interpret it. An arc without a centre falls back to
    /// the chord.
    pub fn arc_length(&self) -> f64 {
        let chord = self.start.distance_to(&self.end);
        let center = match (self.segment_type, self.center) {
            (ToolpathSegmentType::ArcCW | ToolpathSegmentType::ArcCCW, Some(c)) => c,
            _ => return chord,
        };
        let radius = center.distance_to(&self.start);
        let a0 = (self.start.y - center.y).atan2(self.start.x - center.x);
        let a1 = (self.end.y - center.y).atan2(self.end.x - center.x);
        let ccw_sweep = if self.segment_type == ToolpathSegmentType::ArcCCW {
            a1 - a0
        } else {
            a0 - a1
        };
        let sweep = ccw_sweep.rem_euclid(std::f64::consts::TAU);
        if chord < 1e-9 {
            radius * std::f64::consts::TAU
        } else {
            radius * sweep
        }
    }
}
