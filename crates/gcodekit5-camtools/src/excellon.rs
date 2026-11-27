//! # Excellon Drill Files
//!
//! Parses Excellon (NC drill) files, as exported next to the Gerber layers by KiCad, Eagle
//! and Altium, and drills them for the Gerber page's Drill Holes layer.
//!
//! Supported: the `M48` ... `%`/`M95` header; `METRIC`/`INCH` (and `M71`/`M72`) with `LZ`
//! (leading zeros kept, trailing suppressed) or `TZ` (trailing zeros kept, leading
//! suppressed); the number format from a `METRIC,TZ,000.000` pattern or a `FILE_FORMAT=3:3` /
//! `FORMAT={3:3/...}` comment (default 3.3 metric, 2.4 inch, `TZ`); coordinates with a
//! decimal point (taken as written); the tool table `T<n>C<diameter>`; tool selection;
//! absolute (`G90`) and incremental (`G91`) `X`/`Y` hits with an omitted coordinate kept from
//! the previous hit; `G85` slots and routed slots (`G00` move, `M15` down, `G01` line(s),
//! `M16`/`M17` up); `M30`/`M00` end. Repeat codes (`R`) and other commands are skipped.
//!
//! Drilling: one program for every tool. The tools are drilled smallest diameter first; each
//! tool starts with the spindle stopped and an `M0` pause naming the drill to load (GRBL has
//! no `M6`). Each hit is one plain plunge (`G1` to the drill depth at the feed rate, then
//! back to safe Z); GRBL has no `G83` peck cycle. A slot is routed: one plunge at its start,
//! a `G1` to its end, then back to safe Z. Positions get the same X/Y offset as the Gerber
//! layers, so the holes line up with the isolation cut.
//!
//! Dependencies: `anyhow` (errors), `tracing` (skipped commands), [`crate::gerber`]
//! (the page's parameters).

use std::collections::BTreeMap;
use std::fmt::Write;

use anyhow::Result;
use tracing::debug;

use crate::gerber::GerberParameters;

/// One drilled feature, in mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrillFeature {
    /// A single hole at (`x`, `y`).
    Hit {
        /// X position (mm).
        x: f64,
        /// Y position (mm).
        y: f64,
    },
    /// A slot from `start` to `end`, routed with the tool.
    Slot {
        /// Start position `(x, y)` (mm).
        start: (f64, f64),
        /// End position `(x, y)` (mm).
        end: (f64, f64),
    },
}

/// One feature and the tool number it is drilled with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrillOperation {
    /// Tool number (`T<n>`); 0 for features before any tool was selected.
    pub tool: u32,
    /// The hole or slot.
    pub feature: DrillFeature,
}

/// The features of one tool, as drilled together.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolGroup {
    /// Tool number.
    pub tool: u32,
    /// Tool diameter (mm), if the tool table defines it.
    pub diameter: Option<f64>,
    /// The tool's features in file order.
    pub features: Vec<DrillFeature>,
}

/// A parsed Excellon drill file. All lengths are in mm.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExcellonFile {
    /// Tool diameters (mm) by tool number.
    pub tools: BTreeMap<u32, f64>,
    /// Every hole and slot in file order.
    pub operations: Vec<DrillOperation>,
}

impl ExcellonFile {
    /// The features grouped by tool, smallest diameter first (tools without a diameter last,
    /// equal diameters by tool number).
    pub fn tool_groups(&self) -> Vec<ToolGroup> {
        let mut by_tool: BTreeMap<u32, Vec<DrillFeature>> = BTreeMap::new();
        for op in &self.operations {
            by_tool.entry(op.tool).or_default().push(op.feature);
        }
        let mut groups: Vec<ToolGroup> = by_tool
            .into_iter()
            .map(|(tool, features)| ToolGroup {
                tool,
                diameter: self.tools.get(&tool).copied(),
                features,
            })
            .collect();
        groups.sort_by(|a, b| match (a.diameter, b.diameter) {
            (Some(da), Some(db)) => da.total_cmp(&db).then(a.tool.cmp(&b.tool)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.tool.cmp(&b.tool),
        });
        groups
    }
}

/// Whether `content` starts like an Excellon drill file: its first line that is neither
/// blank nor a `;` comment is `M48`.
///
/// # Arguments
/// * `content` - The file's text.
pub fn looks_like_excellon(content: &str) -> bool {
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with(';'))
        .is_some_and(|line| line.starts_with("M48"))
}

/// Which zeros a number without a decimal point leaves out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zeros {
    /// `LZ`: leading zeros kept, trailing zeros suppressed.
    Leading,
    /// `TZ`: trailing zeros kept, leading zeros suppressed.
    Trailing,
}

/// How the file writes numbers.
#[derive(Debug, Clone, Copy)]
struct NumberFormat {
    /// mm per file unit.
    scale: f64,
    /// Digits before the implied decimal point.
    int_digits: i32,
    /// Digits after the implied decimal point.
    dec_digits: i32,
    /// Whether the digit counts came from the file (a unit change then keeps them).
    explicit: bool,
    zeros: Zeros,
}

impl NumberFormat {
    /// Switches to metric (`metric`) or inch units, with that unit's default digits unless
    /// the file gave its own.
    fn set_units(&mut self, metric: bool) {
        self.scale = if metric { 1.0 } else { 25.4 };
        if !self.explicit {
            (self.int_digits, self.dec_digits) = if metric { (3, 3) } else { (2, 4) };
        }
    }

    /// The value of a coordinate or diameter `text` in mm, or `None` if it is not a number.
    fn value(&self, text: &str) -> Option<f64> {
        if text.contains('.') {
            return text.parse::<f64>().ok().map(|v| v * self.scale);
        }
        let (negative, digits) = match text.as_bytes().first() {
            Some(b'-') => (true, &text[1..]),
            Some(b'+') => (false, &text[1..]),
            _ => (false, text),
        };
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let raw: f64 = digits.parse().ok()?;
        let exponent = match self.zeros {
            Zeros::Trailing => -self.dec_digits,
            // The number is padded on the right to the full width.
            Zeros::Leading => self.int_digits - i32::try_from(digits.len()).ok()?,
        };
        let value = raw * 10f64.powi(exponent) * self.scale;
        Some(if negative { -value } else { value })
    }
}

impl Default for NumberFormat {
    fn default() -> Self {
        Self {
            scale: 1.0,
            int_digits: 3,
            dec_digits: 3,
            explicit: false,
            zeros: Zeros::Trailing,
        }
    }
}

/// The number written after the first `letter` of `line`, if any.
fn field(line: &str, letter: char) -> Option<&str> {
    let start = line.find(letter)? + 1;
    let rest = &line[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
        .unwrap_or(rest.len());
    Some(&rest[..end])
}

/// Reads `int:dec` digit counts from a `FILE_FORMAT=3:3` or `FORMAT={3:3/...}` comment.
fn comment_format(comment: &str) -> Option<(i32, i32)> {
    let upper = comment.to_ascii_uppercase();
    let at = upper.find("FORMAT")? + "FORMAT".len();
    let rest = upper[at..].trim_start_matches(['=', '{', ' ']);
    let mut parts = rest.splitn(2, ':');
    let int = parts.next()?.trim().parse().ok()?;
    let dec: String = parts
        .next()?
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    Some((int, dec.parse().ok()?))
}

/// Parser state while reading the body.
struct Reader {
    format: NumberFormat,
    file: ExcellonFile,
    tool: u32,
    position: (f64, f64),
    incremental: bool,
    /// Route mode: the tool is down (`M15`) and `G01` moves cut slots.
    tool_down: bool,
}

impl Reader {
    /// The position `line`'s `X`/`Y` words move to (an omitted word keeps that axis).
    fn target(&self, line: &str) -> (f64, f64) {
        let axis =
            |letter, current: f64| match field(line, letter).and_then(|t| self.format.value(t)) {
                Some(v) if self.incremental => current + v,
                Some(v) => v,
                None => current,
            };
        (axis('X', self.position.0), axis('Y', self.position.1))
    }

    fn push(&mut self, feature: DrillFeature) {
        self.file.operations.push(DrillOperation {
            tool: self.tool,
            feature,
        });
    }

    /// A `T<n>` word: selects tool `n`, and defines its diameter if a `C` word follows.
    fn tool_word(&mut self, line: &str) {
        let Some(number) = field(line, 'T').and_then(|t| t.parse::<u32>().ok()) else {
            return;
        };
        if let Some(diameter) = field(line, 'C').and_then(|t| self.format.value(t)) {
            self.file.tools.insert(number, diameter);
        }
        self.tool = number;
    }

    /// A unit line (`METRIC,TZ,000.000` or `INCH,LZ`).
    fn units_line(&mut self, line: &str) {
        let mut tokens = line.split(',').map(str::trim);
        let metric = tokens.next().is_some_and(|t| t.starts_with("METRIC"));
        self.format.set_units(metric);
        for token in tokens {
            match token {
                "LZ" => self.format.zeros = Zeros::Leading,
                "TZ" => self.format.zeros = Zeros::Trailing,
                pattern
                    if pattern.contains('.') && pattern.bytes().all(|b| b == b'0' || b == b'.') =>
                {
                    if let Some((int, dec)) = pattern.split_once('.') {
                        self.format.int_digits = i32::try_from(int.len()).unwrap_or(3);
                        self.format.dec_digits = i32::try_from(dec.len()).unwrap_or(3);
                        self.format.explicit = true;
                    }
                }
                other => debug!(token = other, "Excellon unit option skipped"),
            }
        }
    }

    /// One body line.
    fn body_line(&mut self, line: &str) {
        if let Some((from, to)) = line.split_once("G85") {
            // Slot: `X..Y..G85X..Y..`.
            let start = self.target(from);
            self.position = start;
            let end = self.target(to);
            self.position = end;
            self.push(DrillFeature::Slot { start, end });
            return;
        }
        match line {
            "G90" => self.incremental = false,
            "G91" => self.incremental = true,
            "M71" => self.format.set_units(true),
            "M72" => self.format.set_units(false),
            "M15" => self.tool_down = true,
            "M16" | "M17" => self.tool_down = false,
            "G05" | "G5" => self.tool_down = false,
            _ if line.starts_with('T') => self.tool_word(line),
            _ if line.starts_with("G00") || line.starts_with("G0X") || line.starts_with("G0Y") => {
                self.position = self.target(line);
            }
            _ if line.starts_with("G01") || line.starts_with("G1X") || line.starts_with("G1Y") => {
                let end = self.target(line);
                if self.tool_down {
                    self.push(DrillFeature::Slot {
                        start: self.position,
                        end,
                    });
                }
                self.position = end;
            }
            _ if line.starts_with('X') || line.starts_with('Y') => {
                if line.contains('R') {
                    debug!(line, "Excellon repeat code skipped");
                    return;
                }
                let (x, y) = self.target(line);
                self.position = (x, y);
                self.push(DrillFeature::Hit { x, y });
            }
            other => debug!(line = other, "Excellon command skipped"),
        }
    }
}

/// Parses an Excellon drill file.
///
/// # Arguments
/// * `content` - The drill file's text.
///
/// # Returns
/// The tool table and every hole and slot, in mm.
///
/// # Errors
/// A file that does not start with an `M48` header, or one with no holes or slots.
pub fn parse(content: &str) -> Result<ExcellonFile> {
    if !looks_like_excellon(content) {
        anyhow::bail!("The Drill Holes file is not an Excellon drill file (no M48 header)");
    }
    let mut reader = Reader {
        format: NumberFormat::default(),
        file: ExcellonFile::default(),
        tool: 0,
        position: (0.0, 0.0),
        incremental: false,
        tool_down: false,
    };
    let mut in_header = false;
    for raw in content.lines() {
        let line = raw.trim();
        if let Some(comment) = line.strip_prefix(';') {
            if in_header && let Some((int, dec)) = comment_format(comment) {
                reader.format.int_digits = int;
                reader.format.dec_digits = dec;
                reader.format.explicit = true;
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }
        if in_header {
            match line {
                "%" | "M95" => in_header = false,
                _ if line.starts_with("METRIC") || line.starts_with("INCH") => {
                    reader.units_line(line);
                }
                "M71" => reader.format.set_units(true),
                "M72" => reader.format.set_units(false),
                _ if line.starts_with('T') => reader.tool_word(line),
                other => debug!(line = other, "Excellon header line skipped"),
            }
            continue;
        }
        match line {
            "M48" => in_header = true,
            "M30" | "M00" | "M0" => break,
            _ => reader.body_line(line),
        }
    }
    if reader.file.operations.is_empty() {
        anyhow::bail!("The drill file has no holes to drill");
    }
    Ok(reader.file)
}

/// Appends the drilling moves of `drill` to `gcode`, one tool after the other.
///
/// The program around it (work zero, safe Z, spindle on, end) is the Gerber converter's.
///
/// # Arguments
/// * `params` - The page's parameters: `alignment_hole_depth` is the drill depth,
///   `feed_rate` the plunge feed, `safe_z`, `spindle_speed` and `offset_x`/`offset_y`.
/// * `drill` - The parsed drill file.
/// * `gcode` - The program to append to.
///
/// # Errors
/// A device without a Z axis, a drill depth that is not below the surface, or a feed rate
/// that is not above 0.
pub fn append_drill_program(
    params: &GerberParameters,
    drill: &ExcellonFile,
    gcode: &mut String,
) -> Result<()> {
    if params.num_axes < 3 {
        anyhow::bail!("The Drill Holes layer needs a Z axis");
    }
    let depth = params.alignment_hole_depth;
    if !(depth.is_finite() && depth < 0.0) {
        anyhow::bail!("Drill depth must be below the surface (negative Z)");
    }
    if !(params.feed_rate.is_finite() && params.feed_rate > 0.0) {
        anyhow::bail!("Feed rate must be greater than 0");
    }
    let safe_z = params.safe_z;
    let at = |x: f64, y: f64| (x as f32 + params.offset_x, y as f32 + params.offset_y);
    for group in drill.tool_groups() {
        let size = group
            .diameter
            .map_or_else(|| "unknown size".to_string(), |d| format!("{d:.3} mm"));
        writeln!(
            gcode,
            "; Drill T{}: {size}, {} feature(s)",
            group.tool,
            group.features.len()
        )?;
        writeln!(gcode, "G0 Z{safe_z:.3}")?;
        writeln!(gcode, "M5")?;
        writeln!(
            gcode,
            "M0 ; Tool change: load the {size} drill (T{}), then resume",
            group.tool
        )?;
        writeln!(gcode, "M3 S{:.1}", params.spindle_speed)?;
        for feature in &group.features {
            let (start, end) = match *feature {
                DrillFeature::Hit { x, y } => ((x, y), None),
                DrillFeature::Slot { start, end } => (start, Some(end)),
            };
            let (sx, sy) = at(start.0, start.1);
            writeln!(gcode, "G0 X{sx:.3} Y{sy:.3}")?;
            writeln!(gcode, "G1 Z{depth:.3} F{:.1}", params.feed_rate)?;
            if let Some((ex, ey)) = end {
                let (ex, ey) = at(ex, ey);
                writeln!(gcode, "G1 X{ex:.3} Y{ey:.3}")?;
            }
            writeln!(gcode, "G0 Z{safe_z:.3}")?;
        }
    }
    Ok(())
}
