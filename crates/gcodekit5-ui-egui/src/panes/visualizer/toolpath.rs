//! Builds the toolpath's line vertices from the program the machine streams.
//!
//! The program is parsed by the shared [`gcodekit5_visualizer::Visualizer`] (unchanged). Its
//! commands do not say which source line they came from, and the progress colouring needs
//! that, so a marker dwell (`G4 P<MARKER_DWELL>`) is placed after every program line: the
//! dwell changes no modal state, and counting markers numbers each command with its line.
//! A real dwell of exactly [`MARKER_DWELL`] seconds would be miscounted; no program dwells for
//! 12345.678 s.
//!
//! As in the GTK `ui/gtk/renderer_3d.rs`, rapids and moves made with the spindle/laser off
//! are travel (grey), powered feed moves and arcs are cuts coloured by spindle/laser power,
//! and arcs are split into segments of at most 5 degrees.
//!
//! Dependencies: `glam`, `bytemuck`, `gcodekit5-visualizer` (parser, arc maths).

use gcodekit5_visualizer::gcode::arc;
use gcodekit5_visualizer::{GCodeCommand, Point3D, Visualizer};
use glam::Vec3;

use super::camera::Aabb;

/// The dwell (seconds) of the marker placed after each program line.
pub const MARKER_DWELL: f32 = 12345.678;

/// Colour of travel: rapids and moves with the spindle/laser off (GTK's 50% grey).
pub const TRAVEL_COLOR: [u8; 4] = [128, 128, 128, 255];

/// One toolpath vertex: position, colour and the program line it belongs to.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ToolpathVertex {
    /// Position in mm.
    pub pos: [f32; 3],
    /// sRGB colour with alpha.
    pub color: [u8; 4],
    /// Index of the program line (as streamed) this segment came from.
    pub line: u32,
}

/// The toolpath of one program, ready to upload.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolpathMesh {
    /// Line-list vertices (two per segment), in program order.
    pub vertices: Vec<ToolpathVertex>,
    /// The bounds of every move, or `None` when the program moves nowhere.
    pub bounds: Option<Aabb>,
    /// The bounds of the cutting moves (GTK's inspector bounds), else `None`.
    pub cut_bounds: Option<Aabb>,
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
}

impl Builder {
    fn segment(&mut self, from: Vec3, to: Vec3, color: [u8; 4], cut: bool) {
        for p in [from, to] {
            self.mesh.vertices.push(ToolpathVertex {
                pos: p.to_array(),
                color,
                line: self.line,
            });
            grow(&mut self.mesh.bounds, p);
            if cut {
                grow(&mut self.mesh.cut_bounds, p);
            }
        }
    }

    /// An XY arc as segments of at most 5 degrees (4 to 128 of them), as GTK draws it.
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
            return;
        };
        let segments = sweep.segment_count(std::f64::consts::PI / 36.0, 4, 128);
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
            prev = next;
        }
    }
}

/// The colour of a cut at spindle/laser power `s`, as the GTK 3D view picks it.
fn cut_color(vis: &Visualizer, s: f32) -> [u8; 4] {
    let (r, g, b) = vis.get_color_for_intensity(s);
    let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    [byte(r), byte(g), byte(b), 255]
}

/// Builds the toolpath of `lines`, the program as the machine streams it (one entry per
/// streamed line).
///
/// # Arguments
/// * `lines` - The program's lines.
pub fn build(lines: &[String]) -> ToolpathMesh {
    let mut vis = Visualizer::new();
    vis.parse_gcode(&marked_program(lines));
    let mut builder = Builder {
        mesh: ToolpathMesh::default(),
        line: 0,
    };
    let v = |p: &Point3D| Vec3::new(p.x, p.y, p.z);
    for command in vis.commands() {
        match command {
            GCodeCommand::Dwell { duration, .. } => {
                if (duration - MARKER_DWELL).abs() < 1e-3 {
                    builder.line = builder.line.saturating_add(1);
                }
            }
            GCodeCommand::Move {
                from,
                to,
                rapid,
                intensity,
            } => match (*rapid, intensity) {
                (false, Some(s)) => builder.segment(v(from), v(to), cut_color(&vis, *s), true),
                (false, None) | (true, _) => builder.segment(v(from), v(to), TRAVEL_COLOR, false),
            },
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
    builder.mesh
}
