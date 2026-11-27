//! Builds the toolpath's line vertices from the program the machine streams.
//!
//! The program is parsed by the shared [`gcodekit5_visualizer::Visualizer`] (unchanged). Its
//! commands do not say which source line they came from, and the progress colouring needs
//! that, so a marker dwell (`G4 P<MARKER_DWELL>`) is placed after every program line: the
//! dwell changes no modal state, and counting markers numbers each command with its line.
//! A real dwell of exactly [`MARKER_DWELL`] seconds would be miscounted; no program dwells for
//! 12345.678 s.
//!
//! Rapids and moves made with the spindle/laser off
//! are travel (grey), powered feed moves and arcs are cuts coloured by spindle/laser power,
//! and arcs are split into segments of at most 5 degrees and at most the Preferences "Arc
//! Segment Length" long ([`arc_segment_count`]).
//!
//! The same pass takes the inspector statistics of the S values ([`Intensity`]) and
//! builds the run preview's path ([`super::preview::PathBuilder`]), so the
//! preview needs no second parse.
//!
//! Dependencies: `glam`, `bytemuck`, `gcodekit5-visualizer` (parser, arc maths).

use gcodekit5_visualizer::gcode::arc;
use gcodekit5_visualizer::{GCodeCommand, Point3D, Visualizer};
use glam::Vec3;

use super::camera::Aabb;
use super::preview::PathBuilder;

/// The dwell (seconds) of the marker placed after each program line.
pub const MARKER_DWELL: f32 = 12345.678;

/// Colour of travel: rapids and moves with the spindle/laser off (50% grey).
pub const TRAVEL_COLOR: [u8; 4] = [128, 128, 128, 255];

/// The default "Arc Segment Length" (mm). Arcs are drawn in steps of at most 5 degrees
/// whatever the length, so this long a chord binds only on arcs of radius above about 1146
/// mm: the default draws as the visualizer always has (5-degree steps).
pub const DEFAULT_ARC_SEGMENT_LENGTH: f64 = 100.0;

/// The "Arc Segment Length" values honoured (mm); others are clamped into it, and a value
/// that is not a number falls back to [`DEFAULT_ARC_SEGMENT_LENGTH`].
pub const ARC_SEGMENT_LENGTH_RANGE: std::ops::RangeInclusive<f64> = 0.01..=100.0;

/// The longest arc step, in radians (5 degrees).
const ARC_MAX_STEP: f64 = std::f64::consts::PI / 36.0;

/// The fewest and most segments one arc is drawn with, so a tiny segment length cannot
/// explode the vertex count.
const ARC_SEGMENTS: (usize, usize) = (4, 128);

/// `mm` as an "Arc Segment Length" the visualizer honours: clamped into
/// [`ARC_SEGMENT_LENGTH_RANGE`], or the default when it is not a number.
///
/// # Arguments
/// * `mm` - The configured length.
pub fn clamp_arc_segment_length(mm: f64) -> f64 {
    if mm.is_nan() {
        DEFAULT_ARC_SEGMENT_LENGTH
    } else {
        mm.clamp(
            *ARC_SEGMENT_LENGTH_RANGE.start(),
            *ARC_SEGMENT_LENGTH_RANGE.end(),
        )
    }
}

/// The number of segments an arc of `sweep` radians and `radius` mm is drawn with: steps of
/// at most 5 degrees and chords of at most `segment_length` mm (clamped by
/// [`clamp_arc_segment_length`]), between 4 and 128 segments.
///
/// # Arguments
/// * `sweep` - The arc's sweep in radians (either sign).
/// * `radius` - The arc's radius in mm.
/// * `segment_length` - The Preferences "Arc Segment Length" in mm.
pub fn arc_segment_count(sweep: f64, radius: f64, segment_length: f64) -> usize {
    let (min, max) = ARC_SEGMENTS;
    let by_angle = (sweep.abs() / ARC_MAX_STEP).ceil();
    let by_length = (sweep.abs() * radius.abs() / clamp_arc_segment_length(segment_length)).ceil();
    let wanted = by_angle.max(by_length);
    if wanted.is_finite() {
        (wanted as usize).clamp(min, max)
    } else {
        min
    }
}

/// The bit of [`ToolpathVertex`]'s tag set on travel (rapids and moves with the
/// spindle/laser off), so the shader can hide travel or cuts (the sidebar's Show Rapid Moves
/// and Show Cutting Moves) without a second buffer. The other bits are the program line.
pub const TRAVEL_BIT: u32 = 1 << 31;

/// One toolpath vertex: position, colour, and a tag holding the program line it belongs to
/// and whether it is travel ([`TRAVEL_BIT`]).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ToolpathVertex {
    /// Position in mm.
    pub pos: [f32; 3],
    /// sRGB colour with alpha.
    pub color: [u8; 4],
    /// The program line, with [`TRAVEL_BIT`] set on travel.
    tag: u32,
}

impl ToolpathVertex {
    /// Index of the program line (as streamed) this segment came from.
    pub fn line(&self) -> u32 {
        self.tag & !TRAVEL_BIT
    }

    /// Whether the segment is travel: a rapid, or a move with the spindle/laser off.
    pub fn is_travel(&self) -> bool {
        self.tag & TRAVEL_BIT != 0
    }
}

/// The inspector statistics of the spindle/laser power: over every move and arc that
/// carries an S value, one value per command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Intensity {
    /// The lowest S.
    pub min: f32,
    /// The highest S.
    pub max: f32,
    /// The mean S.
    pub avg: f32,
}

/// Sums S values for [`Intensity`].
#[derive(Debug, Default)]
struct IntensitySum {
    min: f32,
    max: f32,
    sum: f64,
    count: u64,
}

impl IntensitySum {
    fn add(&mut self, s: f32) {
        if self.count == 0 {
            (self.min, self.max) = (s, s);
        } else {
            self.min = self.min.min(s);
            self.max = self.max.max(s);
        }
        self.sum += f64::from(s);
        self.count += 1;
    }

    fn finish(&self) -> Option<Intensity> {
        (self.count > 0).then_some(Intensity {
            min: self.min,
            max: self.max,
            avg: (self.sum / self.count as f64) as f32,
        })
    }
}

/// The toolpath of one program, ready to upload.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolpathMesh {
    /// Line-list vertices (two per segment), in program order.
    pub vertices: Vec<ToolpathVertex>,
    /// The bounds of every move, or `None` when the program moves nowhere.
    pub bounds: Option<Aabb>,
    /// The bounds of the cutting moves (the inspector bounds), else `None`.
    pub cut_bounds: Option<Aabb>,
    /// The run preview's path.
    pub preview: Vec<Vec3>,
    /// The S-value statistics, or `None` when no move sets a power.
    pub intensity: Option<Intensity>,
}

/// The program text with a marker dwell after every line.
fn marked_program(lines: &[String]) -> String {
    let marker = format!("G4 P{MARKER_DWELL}");
    let size = lines.iter().map(|l| l.len() + marker.len() + 2).sum();
    let mut text = String::with_capacity(size);
    for line in lines {
        text.push_str(line);
        text.push('\n');
        text.push_str(&marker);
        text.push('\n');
    }
    text
}

/// Grows `bounds` to hold `p`.
fn grow(bounds: &mut Option<Aabb>, p: Vec3) {
    *bounds = Some(match bounds {
        Some(b) => Aabb::new(b.min.min(p), b.max.max(p)),
        None => Aabb::new(p, p),
    });
}

/// Collects segments into the vertex list and the bounds.
struct Builder {
    mesh: ToolpathMesh,
    line: u32,
    path: PathBuilder,
    /// The longest arc chord (mm).
    arc_segment_length: f64,
}

impl Builder {
    fn segment(&mut self, from: Vec3, to: Vec3, color: [u8; 4], cut: bool) {
        // A program of 2^31 lines is far beyond any buffer the GPU takes.
        let line = self.line.min(!TRAVEL_BIT);
        let tag = if cut { line } else { line | TRAVEL_BIT };
        for p in [from, to] {
            self.mesh.vertices.push(ToolpathVertex {
                pos: p.to_array(),
                color,
                tag,
            });
            grow(&mut self.mesh.bounds, p);
            if cut {
                grow(&mut self.mesh.cut_bounds, p);
            }
        }
    }

    /// An XY arc as segments of at most 5 degrees and the arc segment length
    /// ([`arc_segment_count`]).
    fn arc(
        &mut self,
        from: Point3D,
        to: Point3D,
        center: Point3D,
        cw: bool,
        color: [u8; 4],
        cut: bool,
    ) {
        let v = |p: Point3D| Vec3::new(p.x, p.y, p.z);
        let center_xy = (f64::from(center.x), f64::from(center.y));
        let Some(sweep) = arc::arc_sweep(
            center_xy,
            (f64::from(from.x), f64::from(from.y)),
            (f64::from(to.x), f64::from(to.y)),
            cw,
        ) else {
            // Zero radius: a straight move.
            self.segment(v(from), v(to), color, cut);
            self.path.line(v(from), v(to), 2);
            return;
        };
        let segments = arc_segment_count(sweep.sweep, sweep.radius, self.arc_segment_length);
        let mut prev = v(from);
        for i in 1..=segments {
            let next = if i == segments {
                v(to)
            } else {
                let t = i as f64 / segments as f64;
                let (x, y) = sweep.point_at(center_xy, t);
                Vec3::new(x as f32, y as f32, from.z + (to.z - from.z) * t as f32)
            };
            self.segment(prev, next, color, cut);
            self.path.line(prev, next, 1);
            prev = next;
        }
    }
}

/// The colour of a cut at spindle/laser power `s`.
fn cut_color(vis: &Visualizer, s: f32) -> [u8; 4] {
    let (r, g, b) = vis.get_color_for_intensity(s);
    let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    [byte(r), byte(g), byte(b), 255]
}

/// Builds the toolpath of `lines`, the program as the machine streams it (one entry per
/// streamed line), with the default arc segment length.
///
/// # Arguments
/// * `lines` - The program's lines.
pub fn build(lines: &[String]) -> ToolpathMesh {
    build_with(lines, DEFAULT_ARC_SEGMENT_LENGTH)
}

/// Builds the toolpath of `lines` like [`build`], drawing arcs with chords of at most
/// `arc_segment_length` mm (see [`arc_segment_count`]).
///
/// # Arguments
/// * `lines` - The program's lines.
/// * `arc_segment_length` - The Preferences "Arc Segment Length" in mm.
pub fn build_with(lines: &[String], arc_segment_length: f64) -> ToolpathMesh {
    let mut vis = Visualizer::new();
    vis.parse_gcode(&marked_program(lines));
    let mut builder = Builder {
        mesh: ToolpathMesh::default(),
        line: 0,
        path: PathBuilder::default(),
        arc_segment_length,
    };
    let v = |p: &Point3D| Vec3::new(p.x, p.y, p.z);
    let mut intensity = IntensitySum::default();
    for command in vis.commands() {
        if let GCodeCommand::Move {
            intensity: Some(s), ..
        }
        | GCodeCommand::Arc {
            intensity: Some(s), ..
        } = command
        {
            intensity.add(*s);
        }
        match command {
            GCodeCommand::Dwell { duration, pos } => {
                if (duration - MARKER_DWELL).abs() < 1e-3 {
                    builder.line = builder.line.saturating_add(1);
                } else {
                    builder.path.dwell(v(pos), *duration);
                }
            }
            GCodeCommand::Move {
                from,
                to,
                rapid,
                intensity,
            } => {
                builder.path.line(v(from), v(to), 2);
                match (*rapid, intensity) {
                    (false, Some(s)) => builder.segment(v(from), v(to), cut_color(&vis, *s), true),
                    (false, None) | (true, _) => {
                        builder.segment(v(from), v(to), TRAVEL_COLOR, false)
                    }
                }
            }
            GCodeCommand::Arc {
                from,
                to,
                center,
                clockwise,
                intensity,
            } => {
                let color = intensity.map_or(TRAVEL_COLOR, |s| cut_color(&vis, s));
                let cut = intensity.is_some();
                builder.arc(*from, *to, *center, *clockwise, color, cut);
            }
        }
    }
    let mut mesh = builder.mesh;
    mesh.preview = builder.path.finish();
    mesh.intensity = intensity.finish();
    mesh
}
