//! G-code interpreter behind [`Visualizer::parse_gcode`].
//!
//! Turns program text into the [`GCodeCommand`] stream plus the toolpath
//! bounds, final position and spindle intensity. Each line goes through three
//! steps: [`Interpreter::scan_words`] reads its words (updating the modal
//! state), the non-motion cases (dwell, G92, G53) are handled on their own, and
//! [`Interpreter::emit_motion`] draws the move in the active motion mode.
//!
//! Depends on the word tokenizer and arc maths of `gcodekit5-core` (re-exported
//! as [`crate::gcode`]) and on [`Bounds`] for extent tracking.
//!
//! [`Visualizer::parse_gcode`]: super::Visualizer::parse_gcode

use super::viewport::Bounds;
use super::visualizer::{GCodeCommand, Point3D, RotaryPosition};
use crate::gcode::{arc, words};
use tracing::{debug, trace};

/// Modal motion mode (group 1).
///
/// Parsing starts in `Rapid` because GRBL powers up in G0, so axis-only lines
/// before the first explicit G0/G1 are still drawn. `None` is only entered via
/// G80 (motion mode cancel).
#[derive(Debug, Clone, Copy, PartialEq)]
enum MotionMode {
    None,
    Rapid,  // G0
    Linear, // G1
    ArcCW,  // G2
    ArcCCW, // G3
    /// G38.2-G38.5: a probing move. It stops at contact, so it is drawn as
    /// travel and never simulated as a cut.
    Probe,
}

/// Active arc plane (G17/G18/G19)
#[derive(Debug, Clone, Copy, PartialEq)]
enum ArcPlane {
    XY, // G17
    ZX, // G18
    YZ, // G19
}

impl ArcPlane {
    /// Axis indices (first plane axis, second plane axis, linear axis) using
    /// 0 = X, 1 = Y, 2 = Z. Ordering follows GRBL so CW/CCW sense is correct.
    fn axes(self) -> (usize, usize, usize) {
        match self {
            ArcPlane::XY => (0, 1, 2),
            ArcPlane::ZX => (2, 0, 1),
            ArcPlane::YZ => (1, 2, 0),
        }
    }
}

/// Spindle / laser state (M3/M4 on, M5 off).
///
/// `Unknown` is the state before any M3/M4/M5 has been seen; moves are then
/// treated as powered so that programs that never switch the spindle (common in
/// hand-written or test files) keep their existing appearance.
#[derive(Debug, Clone, Copy, PartialEq)]
enum SpindleState {
    Unknown,
    On,
    Off,
}

/// Modal state carried from line to line.
#[derive(Debug, Clone, Copy)]
struct ModalState {
    motion: MotionMode,
    spindle: SpindleState,
    plane: ArcPlane,
    /// G90 (true) or G91 (false).
    absolute: bool,
    /// G20 (true) or G21 (false).
    inches: bool,
}

impl Default for ModalState {
    fn default() -> Self {
        Self {
            motion: MotionMode::Rapid,
            spindle: SpindleState::Unknown,
            plane: ArcPlane::XY,
            absolute: true,
            inches: false,
        }
    }
}

/// Structure for extracted parameters
/// Supports 6-axis coordinates (X, Y, Z linear + A, B, C rotary)
#[derive(Default)]
struct ExtractedParams {
    x: Option<f32>,
    y: Option<f32>,
    z: Option<f32>,
    // Rotary axis coordinates (4th, 5th, 6th axes)
    a: Option<f32>,
    b: Option<f32>,
    c: Option<f32>,
    // Arc center offsets (I, J, K) and radius (R)
    i: Option<f32>,
    j: Option<f32>,
    k: Option<f32>,
    r: Option<f32>,
    // Spindle / laser power and dwell time
    s: Option<f32>,
    p: Option<f32>,
}

impl ExtractedParams {
    /// Check if this line has any linear or rotary axis movement
    fn has_movement(&self) -> bool {
        self.x.is_some()
            || self.y.is_some()
            || self.z.is_some()
            || self.a.is_some()
            || self.b.is_some()
            || self.c.is_some()
    }

    /// Arc centre offsets indexed by axis (0 = I/X, 1 = J/Y, 2 = K/Z)
    fn ijk(&self) -> [Option<f32>; 3] {
        [self.i, self.j, self.k]
    }

    /// Convert the linear words (X/Y/Z, I/J/K, R) by `scale`.
    fn scale_linear(&mut self, scale: f32) {
        for v in [
            &mut self.x,
            &mut self.y,
            &mut self.z,
            &mut self.i,
            &mut self.j,
            &mut self.k,
            &mut self.r,
        ] {
            *v = v.map(|val| val * scale);
        }
    }
}

/// The words of one line plus its non-modal G-code flags.
#[derive(Default)]
struct LineWords {
    params: ExtractedParams,
    /// G4.
    dwell: bool,
    /// G92.
    set_position: bool,
    /// G28/G30.
    home_move: bool,
    /// G10, G28.1, G30.1, G43.1: the axis words are not a move.
    axes_not_motion: bool,
    /// G53.
    machine_coords: bool,
}

fn point_axis(p: Point3D, axis: usize) -> f64 {
    match axis {
        0 => p.x as f64,
        1 => p.y as f64,
        _ => p.z as f64,
    }
}

fn point_from_axes(values: [f64; 3]) -> Point3D {
    Point3D::new(values[0] as f32, values[1] as f32, values[2] as f32)
}

/// Emit visualizer commands for a G2/G3 arc.
///
/// XY-plane arcs are emitted as `GCodeCommand::Arc`, split so that no single
/// command sweeps more than 180 degrees (full circles would otherwise have
/// identical start and end points, which renderers draw as nothing). Arcs in
/// the G18/G19 planes are tessellated into feed moves because
/// `GCodeCommand::Arc` is interpreted in the XY plane by all renderers.
///
/// Returns `false` when the arc is undefined (no centre information or a zero
/// radius); the caller then falls back to a straight move.
#[allow(clippy::too_many_arguments)]
fn push_arc_commands(
    commands: &mut Vec<GCodeCommand>,
    bounds: &mut Bounds,
    from: Point3D,
    to: Point3D,
    plane: ArcPlane,
    clockwise: bool,
    ijk: [Option<f32>; 3],
    radius_word: Option<f32>,
    intensity: Option<f32>,
) -> bool {
    let (a0, a1, lin) = plane.axes();
    let (sx, sy) = (point_axis(from, a0), point_axis(from, a1));
    let (ex, ey) = (point_axis(to, a0), point_axis(to, a1));

    let offsets = if ijk[a0].is_some() || ijk[a1].is_some() {
        Some((ijk[a0].unwrap_or(0.0) as f64, ijk[a1].unwrap_or(0.0) as f64))
    } else if let Some(r) = radius_word {
        arc::center_offset_from_radius(ex - sx, ey - sy, r as f64, clockwise)
    } else {
        None
    };
    let Some((oi, oj)) = offsets else {
        return false;
    };

    let radius = (oi * oi + oj * oj).sqrt();
    if !radius.is_finite() || radius < 1e-6 {
        return false;
    }
    let (cx, cy) = (sx + oi, sy + oj);
    let start_angle = (-oj).atan2(-oi);
    let sweep = arc::angular_travel((-oi, -oj), (ex - cx, ey - cy), clockwise);

    let (from_lin, to_lin) = (point_axis(from, lin), point_axis(to, lin));
    let point_at = |t: f64| -> Point3D {
        let angle = start_angle + sweep * t;
        let mut v = [0.0f64; 3];
        v[a0] = cx + radius * angle.cos();
        v[a1] = cy + radius * angle.sin();
        v[lin] = from_lin + (to_lin - from_lin) * t;
        point_from_axes(v)
    };

    if plane == ArcPlane::XY {
        let pieces = if sweep.abs() > std::f64::consts::PI + 1e-9 {
            2
        } else {
            1
        };
        let mut prev = from;
        for piece in 1..=pieces {
            let next = if piece == pieces {
                to
            } else {
                point_at(piece as f64 / pieces as f64)
            };
            commands.push(GCodeCommand::Arc {
                from: prev,
                to: next,
                center: Point3D::new(cx as f32, cy as f32, prev.z),
                clockwise,
                intensity,
            });
            prev = next;
        }

        let (min_x, min_y, max_x, max_y) = arc::arc_bounds((cx, cy), radius, start_angle, sweep);
        bounds.update(min_x as f32, min_y as f32, from.z);
        bounds.update(max_x as f32, max_y as f32, to.z);
    } else {
        // 10 degree steps, at least 2 segments
        let steps = ((sweep.abs() / (std::f64::consts::PI / 18.0)).ceil() as usize).max(2);
        let mut prev = from;
        bounds.update(from.x, from.y, from.z);
        for step in 1..=steps {
            let next = if step == steps {
                to
            } else {
                point_at(step as f64 / steps as f64)
            };
            commands.push(GCodeCommand::Move {
                from: prev,
                to: next,
                rapid: false,
                intensity,
            });
            bounds.update(next.x, next.y, next.z);
            prev = next;
        }
    }

    true
}

/// Interpreter state for one [`super::Visualizer::parse_gcode`] run.
pub(super) struct Interpreter {
    /// Commands emitted so far.
    pub(super) commands: Vec<GCodeCommand>,
    /// Extent of everything drawn so far.
    pub(super) bounds: Bounds,
    /// Current work position (mm).
    pub(super) pos: Point3D,
    /// Current rotary position (degrees).
    pub(super) rotary: RotaryPosition,
    /// Last S word seen.
    pub(super) intensity: f32,
    /// Whether the program carries the GcodeKit5 image engraving marker.
    pub(super) is_raster: bool,
    modal: ModalState,
    /// G0, G1, G2, G3 counts, for the debug summary.
    motion_counts: [usize; 4],
    /// Highest work Z reached so far; used to place G53 retracts.
    max_z_seen: f32,
}

impl Interpreter {
    /// Interpret a whole program.
    ///
    /// # Arguments
    /// * `gcode` - Program text.
    ///
    /// # Returns
    /// The final interpreter state, whose public fields hold the result.
    pub(super) fn run(gcode: &str) -> Self {
        let pos = Point3D::new(0.0, 0.0, 0.0);
        let mut state = Self {
            commands: Vec::new(),
            bounds: Bounds::new(),
            pos,
            rotary: RotaryPosition::default(),
            intensity: 0.0,
            is_raster: false,
            modal: ModalState::default(),
            motion_counts: [0usize; 4],
            max_z_seen: pos.z,
        };
        for (line_num, line) in gcode.lines().enumerate() {
            state.interpret_line(line_num, line);
        }
        debug!(
            "Parse complete: G0={}, G1={}, G2={}, G3={}, total commands={}",
            state.motion_counts[0],
            state.motion_counts[1],
            state.motion_counts[2],
            state.motion_counts[3],
            state.commands.len()
        );
        state
    }

    /// Interpret one program line.
    fn interpret_line(&mut self, line_num: usize, line: &str) {
        let original_line = line.trim();
        // Identify if it is a raster image
        if original_line.contains("; GcodeKit5 Image Engraving") {
            self.is_raster = true;
        }

        let words = words::parse_line(original_line);
        if words.is_empty() {
            return;
        }
        let mut line_words = self.scan_words(&words, line_num);

        // Update intensity if there is an S command
        if let Some(intensity) = line_words.params.s {
            self.intensity = intensity;
        }
        // Moves made while the spindle/laser is off (after M5) carry no
        // intensity, so renderers draw them as travel, not as cuts.
        let move_intensity = if self.modal.spindle == SpindleState::Off {
            None
        } else {
            Some(self.intensity)
        };

        // Convert linear words to mm (G20 applies to the whole line)
        let scale = if self.modal.inches { 25.4f32 } else { 1.0 };
        line_words.params.scale_linear(scale);
        let params = &line_words.params;

        if line_words.dwell {
            self.emit_dwell(params, scale);
            return;
        }
        if line_words.axes_not_motion || !params.has_movement() {
            return;
        }
        if line_words.set_position {
            self.set_position(params);
            return;
        }
        if line_words.machine_coords {
            self.emit_machine_retract(params);
            return;
        }
        self.emit_motion(params, line_words.home_move, move_intensity, line_num);
        self.max_z_seen = self.max_z_seen.max(self.pos.z);
    }

    /// Read the words of one line, applying modal G/M codes to the modal
    /// state and collecting the axis/parameter words and non-modal flags.
    fn scan_words(&mut self, words: &[words::Word], line_num: usize) -> LineWords {
        let mut line = LineWords::default();
        let modal = &mut self.modal;
        for word in words {
            let value = word.value as f32;
            let params = &mut line.params;
            match word.letter {
                'G' => {
                    let (major, minor) = words::code_number(word.value);
                    trace!("Line {}: G{}.{} command", line_num, major, minor);
                    match (major, minor) {
                        (0, 0) => modal.motion = MotionMode::Rapid,
                        (1, 0) => modal.motion = MotionMode::Linear,
                        (2, 0) => modal.motion = MotionMode::ArcCW,
                        (3, 0) => modal.motion = MotionMode::ArcCCW,
                        (38, 2..=5) => modal.motion = MotionMode::Probe,
                        (4, 0) => line.dwell = true,
                        (10, _) | (28, 1) | (30, 1) | (43, 1) => line.axes_not_motion = true,
                        (17, 0) => modal.plane = ArcPlane::XY,
                        (18, 0) => modal.plane = ArcPlane::ZX,
                        (19, 0) => modal.plane = ArcPlane::YZ,
                        (20, 0) => modal.inches = true,
                        (21, 0) => modal.inches = false,
                        (28, 0) | (30, 0) => line.home_move = true,
                        (53, 0) => line.machine_coords = true,
                        (80, 0) => modal.motion = MotionMode::None,
                        (90, 0) => modal.absolute = true,
                        (91, 0) => modal.absolute = false,
                        (92, 0) => line.set_position = true,
                        _ => {}
                    }
                }
                'M' => match words::code_number(word.value) {
                    (3, 0) | (4, 0) => modal.spindle = SpindleState::On,
                    (5, 0) => modal.spindle = SpindleState::Off,
                    _ => {}
                },
                'X' => params.x = Some(value),
                'Y' => params.y = Some(value),
                'Z' => params.z = Some(value),
                'A' => params.a = Some(value),
                'B' => params.b = Some(value),
                'C' => params.c = Some(value),
                'I' => params.i = Some(value),
                'J' => params.j = Some(value),
                'K' => params.k = Some(value),
                'R' => params.r = Some(value),
                'S' => params.s = Some(value),
                'P' => params.p = Some(value),
                _ => {}
            }
        }
        line
    }

    /// G4: emit a dwell at the current position.
    fn emit_dwell(&mut self, params: &ExtractedParams, scale: f32) {
        // G4 P<seconds> (GRBL/LinuxCNC); X/S accepted for other dialects.
        // Axis words on a dwell line are not a move.
        let duration = params
            .p
            .or(params.x.map(|x| x / scale))
            .or(params.s)
            .unwrap_or(0.0);
        if duration > 0.0 {
            self.commands.push(GCodeCommand::Dwell {
                pos: self.pos,
                duration,
            });
        }
    }

    /// G92: the given axes take the new values without moving.
    fn set_position(&mut self, params: &ExtractedParams) {
        let (pos, rotary) = (self.pos, self.rotary);
        self.pos = Point3D::new(
            params.x.unwrap_or(pos.x),
            params.y.unwrap_or(pos.y),
            params.z.unwrap_or(pos.z),
        );
        self.rotary = RotaryPosition::new(
            params.a.unwrap_or(rotary.a),
            params.b.unwrap_or(rotary.b),
            params.c.unwrap_or(rotary.c),
        );
    }

    /// G53 (non-modal): draw the machine-coordinate move as a retract.
    fn emit_machine_retract(&mut self, params: &ExtractedParams) {
        // The axis words are machine coordinates, whose relation to the work
        // coordinates (the WCS offset) is unknown here. Generators use G53 only
        // to retract toward the top of travel (e.g. `G53 G0 Z-1` after homing),
        // so draw it as a travel move that never goes below the highest work Z
        // reached so far, with X/Y unchanged. Plotting it at work Z-1 would show
        // (and simulate) a false cut.
        let pos = self.pos;
        let to = Point3D::new(
            pos.x,
            pos.y,
            if params.z.is_some() {
                pos.z.max(self.max_z_seen)
            } else {
                pos.z
            },
        );
        self.motion_counts[0] += 1;
        self.push_line(to, true, None);
    }

    /// Draw the line's move in the active motion mode (rapid for G28/G30,
    /// whose axis words are an intermediate point reached at rapid rate; the
    /// final stored home position is unknown here).
    fn emit_motion(
        &mut self,
        params: &ExtractedParams,
        home_move: bool,
        move_intensity: Option<f32>,
        line_num: usize,
    ) {
        let absolute = self.modal.absolute;
        let resolve = |current: f32, value: Option<f32>| -> f32 {
            match value {
                Some(v) if absolute => v,
                Some(v) => current + v,
                None => current,
            }
        };
        let (pos, rotary) = (self.pos, self.rotary);
        let target = Point3D::new(
            resolve(pos.x, params.x),
            resolve(pos.y, params.y),
            resolve(pos.z, params.z),
        );
        self.rotary = RotaryPosition::new(
            resolve(rotary.a, params.a),
            resolve(rotary.b, params.b),
            resolve(rotary.c, params.c),
        );
        self.bounds.update_rotary(
            params.a.map(|_| self.rotary.a),
            params.b.map(|_| self.rotary.b),
            params.c.map(|_| self.rotary.c),
        );

        let mode = if home_move {
            MotionMode::Rapid
        } else {
            self.modal.motion
        };
        match mode {
            MotionMode::Rapid | MotionMode::Linear => {
                let rapid = mode == MotionMode::Rapid;
                self.motion_counts[if rapid { 0 } else { 1 }] += 1;
                self.push_line(target, rapid, move_intensity);
            }
            MotionMode::ArcCW | MotionMode::ArcCCW => {
                let clockwise = mode == MotionMode::ArcCW;
                self.motion_counts[if clockwise { 2 } else { 3 }] += 1;
                let drawn = push_arc_commands(
                    &mut self.commands,
                    &mut self.bounds,
                    pos,
                    target,
                    self.modal.plane,
                    clockwise,
                    params.ijk(),
                    params.r,
                    move_intensity,
                );
                if drawn {
                    self.pos = target;
                } else {
                    debug!(
                        "Arc without usable I/J/K or R at line {}, treating as linear move",
                        line_num
                    );
                    self.push_line(target, false, move_intensity);
                }
            }
            MotionMode::Probe => self.push_line(target, true, None),
            MotionMode::None => {
                // There is no defined movement mode, ignore it
                debug!("Movement without motion mode at line {}", line_num);
            }
        }
    }

    /// Emit a straight move from the current position to `to`, extend the
    /// bounds by both ends and make `to` the current position.
    fn push_line(&mut self, to: Point3D, rapid: bool, intensity: Option<f32>) {
        let from = self.pos;
        self.commands.push(GCodeCommand::Move {
            from,
            to,
            rapid,
            intensity,
        });
        self.bounds.update(from.x, from.y, from.z);
        self.bounds.update(to.x, to.y, to.z);
        self.pos = to;
    }
}
