//! Work zero commands: the work coordinate systems G54-G59, the lines that select one, set
//! the current position as its zero (`G10 L20`) and move to work zero, and the reading of
//! the active system from the controller's parser state (GRBL's `[GC:...]` reply to `$G`).
//!
//! Plain functions without I/O, shared by the front ends; ported from the GTK
//! `machine_control` view, which built these lines inline.
//!
//! Dependencies: `gcodekit5-devicedb` (controller type), [`crate::machine::Axis`].

use gcodekit5_devicedb::ControllerType;

use crate::machine::Axis;

/// The line that asks GRBL for its G-code parser state; it answers `[GC:...]`, then `ok`.
pub const PARSER_STATE_QUERY: &str = "$G";

/// Whether a controller of the device profile's `controller` type answers `$G` with a GRBL
/// `[GC:...]` parser state (Grbl, grblHAL, FluidNC). Without a profile GRBL is assumed, the
/// app's default controller.
///
/// # Arguments
/// * `controller` - The active device profile's controller type, if a profile is active.
pub fn reports_grbl_parser_state(controller: Option<&ControllerType>) -> bool {
    match controller {
        None | Some(ControllerType::Grbl | ControllerType::GrblHal | ControllerType::FluidNC) => {
            true
        }
        Some(
            ControllerType::TinyG
            | ControllerType::G2Core
            | ControllerType::Smoothieware
            | ControllerType::Marlin
            | ControllerType::Repetier,
        ) => false,
    }
}

/// How the work zero commands work on a controller type, as the GTK front end sent them
/// (`G10 L20 P<n>` for Zero, `G54`-`G59` for the selector, `G90 G0 X0 Y0` for Go to Work Zero,
/// whatever the firmware) where the firmware takes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkZeroSupport {
    /// GRBL family: the active system is read from the parser state (`$G`).
    #[default]
    Reported,
    /// Smoothieware: `G10 L20 P<n>` and G54-G59 work, but there is no `$G`; the active system
    /// is the last one selected (G54 at first), as GTK tracked it.
    Selected,
    /// Repetier: no work coordinate systems; Zero's `G10 L20` is sent as `G92` (the connection
    /// translates it), and the selector is unavailable.
    PositionOnly,
    /// TinyG and g2core (JSON protocols the app does not stream) and Marlin (whose `G10` is a
    /// filament retract): no work zero.
    Unavailable,
}

/// How work zero works on `controller` (see [`WorkZeroSupport`]). Without a profile GRBL is
/// assumed, the app's default controller.
///
/// # Arguments
/// * `controller` - The active device profile's controller type, if a profile is active.
pub fn work_zero_support(controller: Option<&ControllerType>) -> WorkZeroSupport {
    match controller {
        None | Some(ControllerType::Grbl | ControllerType::GrblHal | ControllerType::FluidNC) => {
            WorkZeroSupport::Reported
        }
        Some(ControllerType::Smoothieware) => WorkZeroSupport::Selected,
        Some(ControllerType::Repetier) => WorkZeroSupport::PositionOnly,
        Some(ControllerType::TinyG | ControllerType::G2Core | ControllerType::Marlin) => {
            WorkZeroSupport::Unavailable
        }
    }
}

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

/// The line that sets the rotary `axis` of `wcs` so its current position reads
/// `calibration_deg` (Preferences > Machine > rotary calibration; 0 for a plain zero):
/// `G10 L20 P2 A12.5`, as GTK's A/B/C Zero buttons send it. The value is rounded to 3
/// decimals, with trailing zeros dropped (`0.30000000000000004` is sent as `0.3`).
///
/// # Arguments
/// * `wcs` - The system whose offset is set.
/// * `axis` - The rotary axis (A, B or C).
/// * `calibration_deg` - The value, in degrees, the current position takes.
///
/// # Returns
/// `None` for a calibration that is not a finite number: nothing may be sent then.
pub fn zero_rotary_command(wcs: Wcs, axis: Axis, calibration_deg: f64) -> Option<String> {
    if !calibration_deg.is_finite() {
        return None;
    }
    let text = format!("{calibration_deg:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    let text = if text == "-0" { "0" } else { text };
    Some(format!(
        "G10 L20 P{} {}{text}",
        wcs.p_number(),
        axis.letter()
    ))
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

/// A work coordinate system a line selects or a controller reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WcsWord {
    /// One of G54-G59.
    Known(Wcs),
    /// A system outside G54-G59 (`G59.1`, `G54.1 P7`, ...), or a parser state that names
    /// none: a `G10 L20 P<n>` built for G54-G59 would act on another system than the active
    /// one.
    Other,
}

/// The active system of a GRBL parser-state reply (`[GC:G0 G54 G17 ...]`), if the line is
/// one: [`WcsWord::Other`] when it names a system outside G54-G59 (grblHAL's `G59.1`) or
/// none.
///
/// # Arguments
/// * `line` - One line received from the controller.
pub fn parser_state_wcs(line: &str) -> Option<WcsWord> {
    let inner = line.trim().strip_prefix("[GC:")?.strip_suffix(']')?;
    Some(wcs_in_line(inner).unwrap_or(WcsWord::Other))
}

/// The last system a G-code line selects (`G55`, `G0G59X0`), if any: [`WcsWord::Other`] for
/// a `G54.x`-`G59.x` word (`G59.1`, `G54.1`). Comments (`(...)`, `; ...`) and `$` lines
/// select none.
///
/// # Arguments
/// * `line` - A G-code line, as sent to the controller.
pub fn wcs_in_line(line: &str) -> Option<WcsWord> {
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
                if let Some(word) = wcs_word(&number) {
                    found = Some(word);
                }
            }
            _ => {}
        }
    }
    found
}

/// The system a `G` word's number (`55`, `055`, `59.1`) selects, if it is in 54-59.
fn wcs_word(number: &str) -> Option<WcsWord> {
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    let wcs = Wcs::from_number(whole.parse::<u32>().ok()?)?;
    if fraction.trim_end_matches('0').is_empty() {
        Some(WcsWord::Known(wcs))
    } else {
        Some(WcsWord::Other)
    }
}
