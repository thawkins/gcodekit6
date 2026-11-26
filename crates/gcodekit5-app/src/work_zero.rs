//! Work zero commands: the work coordinate systems G54-G59, the lines that select one, set
//! the current position as its zero (`G10 L20`) and move to work zero, and the reading of
//! the active system from the controller's parser state (GRBL's `[GC:...]` reply to `$G`).
//!
//! Plain functions without I/O, shared by the front ends; ported from the GTK
//! `machine_control` view, which built these lines inline.
//!
//! Dependencies: none beyond [`crate::machine::Axis`].

use crate::machine::Axis;

/// The line that asks GRBL for its G-code parser state; it answers `[GC:...]`, then `ok`.
pub const PARSER_STATE_QUERY: &str = "$G";

/// A work coordinate system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Wcs {
    /// G54, the default system (`P1`).
    #[default]
    G54,
    /// G55 (`P2`).
    G55,
    /// G56 (`P3`).
    G56,
    /// G57 (`P4`).
    G57,
    /// G58 (`P5`).
    G58,
    /// G59 (`P6`).
    G59,
}

impl Wcs {
    /// Every system, G54 first.
    pub const ALL: [Wcs; 6] = [Wcs::G54, Wcs::G55, Wcs::G56, Wcs::G57, Wcs::G58, Wcs::G59];

    /// The G-code word that selects the system (`G55`); also the line that selects it.
    pub fn code(self) -> &'static str {
        match self {
            Wcs::G54 => "G54",
            Wcs::G55 => "G55",
            Wcs::G56 => "G56",
            Wcs::G57 => "G57",
            Wcs::G58 => "G58",
            Wcs::G59 => "G59",
        }
    }

    /// The `P` number that names the system in `G10 L20` (G54 is `P1`).
    pub fn p_number(self) -> u8 {
        match self {
            Wcs::G54 => 1,
            Wcs::G55 => 2,
            Wcs::G56 => 3,
            Wcs::G57 => 4,
            Wcs::G58 => 5,
            Wcs::G59 => 6,
        }
    }

    /// The system a `G` word's number selects (54-59), if any.
    fn from_number(number: u32) -> Option<Wcs> {
        let index = number.checked_sub(54)?;
        Wcs::ALL.get(usize::try_from(index).ok()?).copied()
    }
}

/// The line that makes the current position 0 on `axes` in `wcs`: `G10 L20 P2 X0`.
///
/// # Arguments
/// * `wcs` - The system whose offset is set.
/// * `axes` - The axes to zero, in the order written.
pub fn zero_axes_command(wcs: Wcs, axes: &[Axis]) -> String {
    axes.iter()
        .fold(format!("G10 L20 P{}", wcs.p_number()), |mut line, axis| {
            line.push(' ');
            line.push(axis.letter());
            line.push('0');
            line
        })
}

/// The line that rapids to work zero in absolute mode: `G90 G0 X0 Y0`, with ` Z0` when
/// `include_z` (a Z0 rapid can drive the tool into the work, so it is opt-in). `G90` is
/// explicit: in `G91` the move would be a zero-length relative one.
///
/// # Arguments
/// * `include_z` - Whether Z moves to 0 too.
pub fn go_to_work_zero_command(include_z: bool) -> &'static str {
    if include_z {
        "G90 G0 X0 Y0 Z0"
    } else {
        "G90 G0 X0 Y0"
    }
}

/// The active system of a GRBL parser-state reply (`[GC:G0 G54 G17 ...]`), if the line is
/// one and names a system.
///
/// # Arguments
/// * `line` - One line received from the controller.
pub fn parser_state_wcs(line: &str) -> Option<Wcs> {
    let inner = line.trim().strip_prefix("[GC:")?.strip_suffix(']')?;
    wcs_in_line(inner)
}

/// The last system a G-code line selects (`G55`, `G0G59X0`), if any. Comments (`(...)`,
/// `; ...`) and `$` lines select none; `G54.1` is not a system selection.
///
/// # Arguments
/// * `line` - A G-code line, as sent to the controller.
pub fn wcs_in_line(line: &str) -> Option<Wcs> {
    if line.trim_start().starts_with('$') {
        return None;
    }
    let mut found = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '(' => {
                for c in chars.by_ref() {
                    if c == ')' {
                        break;
                    }
                }
            }
            ';' => break,
            'G' | 'g' => {
                let mut number = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() || d == '.' {
                        number.push(d);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if let Some(wcs) = number.parse::<u32>().ok().and_then(Wcs::from_number) {
                    found = Some(wcs);
                }
            }
            _ => {}
        }
    }
    found
}
