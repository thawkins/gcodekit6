//! # Gerber aperture macros
//!
//! Turns an aperture macro (`%AM`) and the parameters of the `%ADD` that instantiates it
//! into the flashed shape, per the Gerber Layer Format Specification (Ucamco 2023),
//! section 4.5. Supported: primitives 1 (circle), 4 (outline), 5 (polygon), 7 (thermal),
//! 20 (vector line) and 21 (center line), comments (0), variable definitions (`$n=...`) and
//! arithmetic expressions (`+`, `-`, `x`, `/`, unary signs, parentheses). Primitives are
//! combined in order: exposure on adds to the shape, exposure off cuts it away. Moire (6)
//! and anything the parser could not read make the macro unsupported (`None`).
//!
//! Depends on `gerber_types` (the parsed macro) and `csgrs` (the 2D geometry).

use csgrs::sketch::Sketch;
use csgrs::traits::CSG;
use gerber_types::{ApertureMacro, MacroBoolean, MacroContent, MacroDecimal, MacroInteger};
use std::collections::HashMap;

/// Macro variables (`$n`); an undefined variable reads as 0 (spec 4.5.4.2).
type Variables = HashMap<u32, f64>;

/// The shape a macro aperture flashes, centred on the flash point (mm).
///
/// # Arguments
/// * `am` - The macro definition.
/// * `args` - The `%ADD` parameters, bound to `$1`, `$2`, ...
/// * `unit_scale` - File unit to mm factor.
///
/// # Returns
/// The shape (possibly empty), or `None` when the macro has a primitive or expression the
/// converter cannot evaluate.
pub(super) fn macro_sketch(
    am: &ApertureMacro,
    args: &[MacroDecimal],
    unit_scale: f64,
) -> Option<Sketch<()>> {
    let empty = Variables::new();
    let mut vars = Variables::new();
    for (i, arg) in args.iter().enumerate() {
        let n = u32::try_from(i + 1).ok()?;
        vars.insert(n, decimal(arg, &empty)?);
    }
    let mut shape: Sketch<()> = Sketch::new();
    for content in &am.content {
        let (exposure, outline) = match content {
            MacroContent::Comment(_) => continue,
            MacroContent::VariableDefinition(def) => {
                let value = eval_expr(&def.expression, &vars)?;
                vars.insert(def.number, value);
                continue;
            }
            MacroContent::Moire(_) => return None,
            MacroContent::Circle(c) => {
                let d = decimal(&c.diameter, &vars)? * unit_scale;
                let center = point(&c.center, &vars, unit_scale)?;
                let angle = c.angle.as_ref().map_or(Some(0.0), |a| decimal(a, &vars))?;
                let pts = circle_points(center, d / 2.0);
                (boolean(&c.exposure, &vars)?, rotated(pts, angle))
            }
            MacroContent::VectorLine(v) => {
                let w = decimal(&v.width, &vars)? * unit_scale;
                let s = point(&v.start, &vars, unit_scale)?;
                let e = point(&v.end, &vars, unit_scale)?;
                let (dx, dy) = (e[0] - s[0], e[1] - s[1]);
                let len = dx.hypot(dy);
                if len < 1e-12 {
                    continue;
                }
                let (nx, ny) = (-dy / len * w / 2.0, dx / len * w / 2.0);
                let pts = vec![
                    [s[0] + nx, s[1] + ny],
                    [e[0] + nx, e[1] + ny],
                    [e[0] - nx, e[1] - ny],
                    [s[0] - nx, s[1] - ny],
                ];
                let angle = decimal(&v.angle, &vars)?;
                (boolean(&v.exposure, &vars)?, rotated(pts, angle))
            }
            MacroContent::CenterLine(c) => {
                let w = decimal(&c.dimensions.0, &vars)? * unit_scale;
                let h = decimal(&c.dimensions.1, &vars)? * unit_scale;
                let [cx, cy] = point(&c.center, &vars, unit_scale)?;
                let (hw, hh) = (w / 2.0, h / 2.0);
                let pts = vec![
                    [cx - hw, cy - hh],
                    [cx + hw, cy - hh],
                    [cx + hw, cy + hh],
                    [cx - hw, cy + hh],
                ];
                let angle = decimal(&c.angle, &vars)?;
                (boolean(&c.exposure, &vars)?, rotated(pts, angle))
            }
            MacroContent::Outline(o) => {
                let pts = o
                    .points
                    .iter()
                    .map(|p| point(p, &vars, unit_scale))
                    .collect::<Option<Vec<_>>>()?;
                let angle = decimal(&o.angle, &vars)?;
                (boolean(&o.exposure, &vars)?, rotated(pts, angle))
            }
            MacroContent::Polygon(p) => {
                let n = integer(&p.vertices, &vars)?.clamp(3, 12);
                let [cx, cy] = point(&p.center, &vars, unit_scale)?;
                let r = decimal(&p.diameter, &vars)? * unit_scale / 2.0;
                // The first vertex is on the positive X axis through the centre.
                let pts = (0..n)
                    .map(|i| {
                        let a = std::f64::consts::TAU * f64::from(i) / f64::from(n);
                        [cx + r * a.cos(), cy + r * a.sin()]
                    })
                    .collect();
                let angle = decimal(&p.angle, &vars)?;
                (boolean(&p.exposure, &vars)?, rotated(pts, angle))
            }
            MacroContent::Thermal(t) => {
                let thermal = thermal_sketch(
                    point(&t.center, &vars, unit_scale)?,
                    decimal(&t.outer_diameter, &vars)? * unit_scale,
                    decimal(&t.inner_diameter, &vars)? * unit_scale,
                    decimal(&t.gap, &vars)? * unit_scale,
                    decimal(&t.angle, &vars)?,
                );
                // A thermal always has exposure on.
                shape = shape.union(&thermal);
                continue;
            }
        };
        let primitive: Sketch<()> = Sketch::polygon(&outline, None);
        shape = if exposure {
            shape.union(&primitive)
        } else {
            shape.difference(&primitive)
        };
    }
    Some(shape)
}

/// Thermal primitive (code 7): a ring with two gaps crossing at its centre, rotated about
/// the macro origin by `angle` degrees.
fn thermal_sketch(
    center: [f64; 2],
    outer_d: f64,
    inner_d: f64,
    gap: f64,
    angle: f64,
) -> Sketch<()> {
    let poly = |pts: Vec<[f64; 2]>| -> Sketch<()> { Sketch::polygon(&rotated(pts, angle), None) };
    let [cx, cy] = center;
    let (ro, hg) = (outer_d / 2.0, gap / 2.0);
    let mut ring = poly(circle_points(center, ro));
    if inner_d > 0.0 {
        ring = ring.difference(&poly(circle_points(center, inner_d / 2.0)));
    }
    let horizontal = vec![
        [cx - ro, cy - hg],
        [cx + ro, cy - hg],
        [cx + ro, cy + hg],
        [cx - ro, cy + hg],
    ];
    let vertical = vec![
        [cx - hg, cy - ro],
        [cx + hg, cy - ro],
        [cx + hg, cy + ro],
        [cx - hg, cy + ro],
    ];
    ring.difference(&poly(horizontal))
        .difference(&poly(vertical))
}

/// Points of a circle of `radius` around `center`, as fine as the converter's arcs.
fn circle_points(center: [f64; 2], radius: f64) -> Vec<[f64; 2]> {
    let n = super::arc_segments(radius, std::f64::consts::TAU).max(32);
    (0..n)
        .map(|i| {
            let a = std::f64::consts::TAU * i as f64 / n as f64;
            [center[0] + radius * a.cos(), center[1] + radius * a.sin()]
        })
        .collect()
}

/// Rotates `pts` counter-clockwise by `degrees` about the macro origin.
fn rotated(pts: Vec<[f64; 2]>, degrees: f64) -> Vec<[f64; 2]> {
    if degrees == 0.0 {
        return pts;
    }
    let (s, c) = degrees.to_radians().sin_cos();
    pts.into_iter()
        .map(|[x, y]| [x * c - y * s, x * s + y * c])
        .collect()
}

/// A macro coordinate pair in mm.
fn point(p: &(MacroDecimal, MacroDecimal), vars: &Variables, unit_scale: f64) -> Option<[f64; 2]> {
    Some([
        decimal(&p.0, vars)? * unit_scale,
        decimal(&p.1, vars)? * unit_scale,
    ])
}

/// The value of a macro decimal parameter.
fn decimal(d: &MacroDecimal, vars: &Variables) -> Option<f64> {
    match d {
        MacroDecimal::Value(v) => Some(*v),
        MacroDecimal::Variable(n) => Some(vars.get(n).copied().unwrap_or(0.0)),
        MacroDecimal::Expression(e) => eval_expr(e, vars),
    }
}

/// The value of a macro exposure parameter (1 = on, 0 = off).
fn boolean(b: &MacroBoolean, vars: &Variables) -> Option<bool> {
    match b {
        MacroBoolean::Value(v) => Some(*v),
        MacroBoolean::Variable(n) => Some(vars.get(n).copied().unwrap_or(0.0) != 0.0),
        MacroBoolean::Expression(e) => eval_expr(e, vars).map(|v| v != 0.0),
    }
}

/// The value of a macro integer parameter (a vertex count).
fn integer(i: &MacroInteger, vars: &Variables) -> Option<u32> {
    let v = match i {
        MacroInteger::Value(v) => return Some(*v),
        MacroInteger::Variable(n) => vars.get(n).copied().unwrap_or(0.0),
        MacroInteger::Expression(e) => eval_expr(e, vars)?,
    };
    (v.is_finite() && v >= 0.0 && v <= f64::from(u32::MAX)).then(|| v.round() as u32)
}

/// Evaluates a macro arithmetic expression (spec 4.5.4.3).
///
/// Grammar, by recursive descent: `expr := term (('+'|'-') term)*`,
/// `term := factor (('x'|'X'|'/') factor)*`,
/// `factor := ('+'|'-') factor | number | '$'n | '(' expr ')'`.
///
/// # Arguments
/// * `expr` - The expression text.
/// * `vars` - The variables defined so far.
///
/// # Returns
/// The finite value, or `None` for a malformed expression or a division by zero.
pub(super) fn eval_expr(expr: &str, vars: &HashMap<u32, f64>) -> Option<f64> {
    let tokens: Vec<char> = expr.chars().filter(|c| !c.is_whitespace()).collect();
    let mut parser = ExprParser {
        tokens: &tokens,
        pos: 0,
        vars,
    };
    let value = parser.expr()?;
    (parser.pos == tokens.len() && value.is_finite()).then_some(value)
}

/// Cursor over the characters of one macro expression.
struct ExprParser<'a> {
    tokens: &'a [char],
    pos: usize,
    vars: &'a HashMap<u32, f64>,
}

impl ExprParser<'_> {
    fn peek(&self) -> Option<char> {
        self.tokens.get(self.pos).copied()
    }

    fn expr(&mut self) -> Option<f64> {
        let mut value = self.term()?;
        while let Some(op @ ('+' | '-')) = self.peek() {
            self.pos += 1;
            let rhs = self.term()?;
            value = if op == '+' { value + rhs } else { value - rhs };
        }
        Some(value)
    }

    fn term(&mut self) -> Option<f64> {
        let mut value = self.factor()?;
        while let Some(op @ ('x' | 'X' | '/')) = self.peek() {
            self.pos += 1;
            let rhs = self.factor()?;
            value = if op == '/' { value / rhs } else { value * rhs };
        }
        Some(value)
    }

    fn factor(&mut self) -> Option<f64> {
        match self.peek()? {
            '+' => {
                self.pos += 1;
                self.factor()
            }
            '-' => {
                self.pos += 1;
                self.factor().map(|v| -v)
            }
            '(' => {
                self.pos += 1;
                let value = self.expr()?;
                (self.peek()? == ')').then(|| {
                    self.pos += 1;
                    value
                })
            }
            '$' => {
                self.pos += 1;
                let digits = self.take_while(|c| c.is_ascii_digit());
                let n: u32 = digits.parse().ok()?;
                Some(self.vars.get(&n).copied().unwrap_or(0.0))
            }
            c if c.is_ascii_digit() || c == '.' => self
                .take_while(|c| c.is_ascii_digit() || c == '.')
                .parse()
                .ok(),
            _ => None,
        }
    }

    fn take_while(&mut self, pred: impl Fn(char) -> bool) -> String {
        let start = self.pos;
        while self.peek().is_some_and(&pred) {
            self.pos += 1;
        }
        self.tokens[start..self.pos].iter().collect()
    }
}
