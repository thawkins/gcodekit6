//! Toolpath segments for the stock simulation, built from the
//! visualizer's [`GCodeCommand`] stream.

use crate::visualizer::visualizer::GCodeCommand;

/// Kind of motion a [`ToolpathSegment`] describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolpathSegmentType {
    /// G0 rapid move (still cuts if it passes through the stock).
    RapidMove,
    /// G1 feed move.
    LinearMove,
    /// G2 clockwise arc in the XY plane.
    ArcCW,
    /// G3 counter-clockwise arc in the XY plane.
    ArcCCW,
}

/// One tool motion in work coordinates (mm).
#[derive(Debug, Clone, PartialEq)]
pub struct ToolpathSegment {
    /// Motion kind.
    pub segment_type: ToolpathSegmentType,
    /// Start point (X, Y, Z).
    pub start: (f32, f32, f32),
    /// End point (X, Y, Z).
    pub end: (f32, f32, f32),
    /// Arc centre (X, Y); `None` for straight moves.
    pub center: Option<(f32, f32)>,
}

/// Convert parsed visualizer commands into simulator segments.
///
/// Dwells do not move the tool and are dropped. Arcs outside the XY plane are
/// already tessellated into straight moves by the parser.
///
/// The parser starts from an assumed `(0, 0, 0)`, so the real start position
/// is unknown. Rapids are therefore dropped until every axis has moved at
/// least once; otherwise the opening `G0 Z10` of a bottom-zeroed program would
/// cut a false hole at XY0. Later rapids are kept and cut where they pass
/// through the stock. Feed moves are always kept.
///
/// # Arguments
/// * `commands` - Commands produced by [`crate::visualizer::Visualizer`].
///
/// # Returns
/// The simulator segments, in program order.
pub fn toolpath_from_commands(commands: &[GCodeCommand]) -> Vec<ToolpathSegment> {
    let mut known = [false; 3];
    let mut segments = Vec::with_capacity(commands.len());
    for cmd in commands {
        let (from, to, segment_type, center) = match cmd {
            GCodeCommand::Move {
                from, to, rapid, ..
            } => {
                let kind = if *rapid {
                    ToolpathSegmentType::RapidMove
                } else {
                    ToolpathSegmentType::LinearMove
                };
                (from, to, kind, None)
            }
            GCodeCommand::Arc {
                from,
                to,
                center,
                clockwise,
                ..
            } => {
                let kind = if *clockwise {
                    ToolpathSegmentType::ArcCW
                } else {
                    ToolpathSegmentType::ArcCCW
                };
                (from, to, kind, Some((center.x, center.y)))
            }
            GCodeCommand::Dwell { .. } => continue,
        };
        let start_known = known.iter().all(|&k| k);
        for (axis, (a, b)) in known
            .iter_mut()
            .zip([(from.x, to.x), (from.y, to.y), (from.z, to.z)])
        {
            if a != b {
                *axis = true;
            }
        }
        if segment_type == ToolpathSegmentType::RapidMove && !start_known {
            continue;
        }
        segments.push(ToolpathSegment {
            segment_type,
            start: (from.x, from.y, from.z),
            end: (to.x, to.y, to.z),
            center,
        });
    }
    segments
}
