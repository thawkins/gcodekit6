//! Jigsaw Puzzle Maker
//!
//! Generates G-code toolpaths for laser/CNC cutting jigsaw puzzles with interlocking pieces.

use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// The smallest puzzle width or height accepted (mm).
pub const MIN_SIZE_MM: f64 = 50.0;
/// The fewest pieces accepted in either direction.
pub const MIN_PIECES: f64 = 2.0;
/// The most pieces accepted in either direction.
pub const MAX_PIECES: f64 = 20.0;
/// The smallest piece width or height accepted (mm).
pub const MIN_PIECE_MM: f64 = 15.0;
/// The smallest tab size accepted (percent).
pub const MIN_TAB_SIZE_PERCENT: f64 = 10.0;
/// The largest tab size accepted (percent).
pub const MAX_TAB_SIZE_PERCENT: f64 = 30.0;
/// The largest jitter accepted (percent); the smallest is 0.
pub const MAX_JITTER_PERCENT: f64 = 13.0;
/// The largest corner radius accepted (mm); the smallest is 0.
pub const MAX_CORNER_RADIUS_MM: f64 = 10.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PuzzleParameters {
    pub width: f32,
    pub height: f32,
    pub pieces_across: i32,
    pub pieces_down: i32,
    pub seed: u32,
    pub tab_size_percent: f32,
    pub jitter_percent: f32,
    pub corner_radius: f32,
}

impl Default for PuzzleParameters {
    fn default() -> Self {
        Self {
            width: 200.0,
            height: 150.0,
            pieces_across: 4,
            pieces_down: 3,
            seed: 42,
            tab_size_percent: 20.0,
            jitter_percent: 4.0,
            corner_radius: 2.0,
        }
    }
}

#[derive(Debug, Clone)]
struct Point {
    x: f32,
    y: f32,
}

impl Point {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// The largest integer an `f32` holds exactly (2^24); the RNG state must not pass it.
const RNG_EXACT_LIMIT: u64 = 1 << 24;

pub struct JigsawPuzzleMaker {
    params: PuzzleParameters,
    paths: Vec<Vec<Point>>,
    rng_state: f32,
}

impl JigsawPuzzleMaker {
    pub fn new(params: PuzzleParameters) -> Result<Self, String> {
        Self::validate_parameters(&params)?;
        Ok(Self {
            params: params.clone(),
            paths: Vec::new(),
            rng_state: params.seed as f32,
        })
    }

    fn random(&mut self) -> f32 {
        let x = (self.rng_state.sin() * 10000.0).abs();
        self.rng_state += 1.0;
        x - x.floor()
    }

    fn uniform(&mut self, min: f32, max: f32) -> f32 {
        let r = self.random();
        min + r * (max - min)
    }

    fn rbool(&mut self) -> bool {
        self.random() > 0.5
    }

    fn validate_parameters(params: &PuzzleParameters) -> Result<(), String> {
        if params.width < MIN_SIZE_MM as f32 || params.height < MIN_SIZE_MM as f32 {
            return Err("Puzzle dimensions must be at least 50mm".to_string());
        }

        if params.pieces_across < MIN_PIECES as i32 || params.pieces_down < MIN_PIECES as i32 {
            return Err("Must have at least 2x2 pieces".to_string());
        }

        if params.pieces_across > MAX_PIECES as i32 || params.pieces_down > MAX_PIECES as i32 {
            return Err("Maximum 20 pieces in any direction".to_string());
        }

        let piece_width = params.width / params.pieces_across as f32;
        let piece_height = params.height / params.pieces_down as f32;

        if piece_width < MIN_PIECE_MM as f32 || piece_height < MIN_PIECE_MM as f32 {
            return Err("Pieces too small (minimum 15mm per piece)".to_string());
        }

        if params.tab_size_percent < MIN_TAB_SIZE_PERCENT as f32
            || params.tab_size_percent > MAX_TAB_SIZE_PERCENT as f32
        {
            return Err("Tab size must be between 10% and 30%".to_string());
        }

        if params.jitter_percent < 0.0 || params.jitter_percent > MAX_JITTER_PERCENT as f32 {
            return Err("Jitter must be between 0% and 13%".to_string());
        }

        if params.corner_radius < 0.0 || params.corner_radius > MAX_CORNER_RADIUS_MM as f32 {
            return Err("Corner radius must be between 0 and 10mm".to_string());
        }

        // The f32 RNG state stops advancing past 2^24 (`x + 1.0 == x`), so every value it
        // hands out from there on is the same. Each state the puzzle reads must stay exact.
        let max_seed = RNG_EXACT_LIMIT + 1 - Self::rng_draws(params);
        if u64::from(params.seed) > max_seed {
            return Err(format!(
                "Random seed must be at most {max_seed} for this many pieces"
            ));
        }

        Ok(())
    }

    /// How many random values `generate` reads: one jitter per inner cut and, per tab,
    /// one flip plus five shape values.
    fn rng_draws(params: &PuzzleParameters) -> u64 {
        let across = u64::try_from(params.pieces_across).unwrap_or(0);
        let down = u64::try_from(params.pieces_down).unwrap_or(0);
        across.saturating_sub(1) * (1 + 6 * down) + down.saturating_sub(1) * (1 + 6 * across)
    }

    pub fn generate(&mut self) -> Result<(), String> {
        self.paths.clear();

        // Reset RNG state for reproducible results
        self.rng_state = self.params.seed as f32;

        let piece_width = self.params.width / self.params.pieces_across as f32;
        let piece_height = self.params.height / self.params.pieces_down as f32;

        // Generate vertical cuts with jitter
        for col in 1..self.params.pieces_across {
            let base_x = col as f32 * piece_width;
            let jitter_range = piece_width * (self.params.jitter_percent / 100.0);
            let x = base_x + self.uniform(-jitter_range, jitter_range);
            self.generate_vertical_cut(x, piece_height);
        }

        // Generate horizontal cuts with jitter
        for row in 1..self.params.pieces_down {
            let base_y = row as f32 * piece_height;
            let jitter_range = piece_height * (self.params.jitter_percent / 100.0);
            let y = base_y + self.uniform(-jitter_range, jitter_range);
            self.generate_horizontal_cut(y, piece_width);
        }

        // Generate outer border with corner radius last
        self.generate_border();

        Ok(())
    }

    fn generate_border(&mut self) {
        let mut path = Vec::new();
        let w = self.params.width;
        let h = self.params.height;
        let r = self.params.corner_radius;

        if r > 0.0 {
            // Rounded corners using arc approximation
            let steps = 4;

            // Bottom edge with bottom-left corner
            path.push(Point::new(r, 0.0));
            path.push(Point::new(w - r, 0.0));

            // Bottom-right corner
            for i in 0..=steps {
                let angle = (i as f32 / steps as f32) * PI / 2.0;
                path.push(Point::new(w - r + r * angle.sin(), r - r * angle.cos()));
            }

            // Right edge
            path.push(Point::new(w, r));
            path.push(Point::new(w, h - r));

            // Top-right corner
            for i in 0..=steps {
                let angle = (i as f32 / steps as f32) * PI / 2.0;
                path.push(Point::new(w - r + r * angle.cos(), h - r + r * angle.sin()));
            }

            // Top edge
            path.push(Point::new(w - r, h));
            path.push(Point::new(r, h));

            // Top-left corner
            for i in 0..=steps {
                let angle = (i as f32 / steps as f32) * PI / 2.0;
                path.push(Point::new(r - r * angle.sin(), h - r + r * angle.cos()));
            }

            // Left edge
            path.push(Point::new(0.0, h - r));
            path.push(Point::new(0.0, r));

            // Bottom-left corner
            for i in 0..=steps {
                let angle = (i as f32 / steps as f32) * PI / 2.0;
                path.push(Point::new(r - r * angle.cos(), r - r * angle.sin()));
            }

            path.push(Point::new(r, 0.0));
        } else {
            // Square corners
            path.push(Point::new(0.0, 0.0));
            path.push(Point::new(w, 0.0));
            path.push(Point::new(w, h));
            path.push(Point::new(0.0, h));
            path.push(Point::new(0.0, 0.0));
        }

        self.paths.push(path);
    }

    fn generate_vertical_cut(&mut self, x: f32, piece_height: f32) {
        let mut path = Vec::new();
        let num_tabs = self.params.pieces_down;
        let piece_width = self.params.width / self.params.pieces_across as f32;

        path.push(Point::new(x, 0.0));

        for row in 0..num_tabs {
            let y_start = row as f32 * piece_height;
            let flip = self.rbool();

            // Draw tab using Draradech's Bézier algorithm
            // Save current position
            let start_y = path.last().map(|p| p.y).unwrap_or(0.0);

            // Generate tab path starting from current Y position
            let mut tab_path = Vec::new();
            tab_path.push(Point::new(x, start_y));
            self.draw_tab_vertical(&mut tab_path, x, piece_height, piece_width, flip);

            // Offset tab to correct position and add to main path
            for point in tab_path.iter().skip(1) {
                path.push(Point::new(point.x, y_start + point.y));
            }
        }

        self.paths.push(path);
    }

    fn generate_horizontal_cut(&mut self, y: f32, piece_width: f32) {
        let mut path = Vec::new();
        let num_tabs = self.params.pieces_across;
        let piece_height = self.params.height / self.params.pieces_down as f32;

        path.push(Point::new(0.0, y));

        for col in 0..num_tabs {
            let x_start = col as f32 * piece_width;
            let flip = self.rbool();

            // Draw tab using Draradech's Bézier algorithm
            let start_x = path.last().map(|p| p.x).unwrap_or(0.0);

            let mut tab_path = Vec::new();
            tab_path.push(Point::new(start_x, y));
            self.draw_tab_horizontal(&mut tab_path, y, piece_width, piece_height, flip);

            // Offset tab to correct position and add to main path
            for point in tab_path.iter().skip(1) {
                path.push(Point::new(x_start + point.x, point.y));
            }
        }

        self.paths.push(path);
    }

    fn draw_tab_vertical(
        &mut self,
        path: &mut Vec<Point>,
        x: f32,
        piece_length: f32,
        piece_width: f32,
        flip: bool,
    ) {
        // Draradech algorithm: Uses Bézier curves with random jitter
        // t = tab size (as fraction), j = jitter, a-e = random values
        let t = self.params.tab_size_percent / 200.0; // Divide by 200 to get fraction of piece
        let j = self.params.jitter_percent / 100.0;

        // Generate random values for this tab (matches Draradech's next() function)
        let a = self.uniform(-j, j);
        let b = self.uniform(-j, j);
        let c = self.uniform(-j, j);
        let d = self.uniform(-j, j);
        let e = self.uniform(-j, j);

        let sign = if flip { -1.0 } else { 1.0 };

        // Convert l (length) and w (width) functions - vertical cuts
        let l = |v: f32| piece_length * v;
        let w = |v: f32| x + piece_width * v * sign;

        // 10 control points using Draradech's algorithm (p0-p9)
        // These create 3 cubic Bézier curves
        let p0 = Point::new(w(0.0), l(0.0));
        let p1 = Point::new(w(a), l(0.2));
        let p2 = Point::new(w(-t + c), l(0.5 + b + d));
        let p3 = Point::new(w(t + c), l(0.5 - t + b));
        let p4 = Point::new(w(3.0 * t + c), l(0.5 - 2.0 * t + b - d));
        let p5 = Point::new(w(3.0 * t + c), l(0.5 + 2.0 * t + b - d));
        let p6 = Point::new(w(t + c), l(0.5 + t + b));
        let p7 = Point::new(w(-t + c), l(0.5 + b + d));
        let p8 = Point::new(w(e), l(0.8));
        let p9 = Point::new(w(0.0), l(1.0));

        // Approximate cubic Bézier curves with line segments
        let steps = 10;

        // First cubic Bézier: p0-p1-p2-p3
        for i in 1..=steps {
            let t_val = i as f32 / steps as f32;
            let point = Self::cubic_bezier(p0.clone(), p1.clone(), p2.clone(), p3.clone(), t_val);
            path.push(point);
        }

        // Second cubic Bézier: p3-p4-p5-p6
        for i in 1..=steps {
            let t_val = i as f32 / steps as f32;
            let point = Self::cubic_bezier(p3.clone(), p4.clone(), p5.clone(), p6.clone(), t_val);
            path.push(point);
        }

        // Third cubic Bézier: p6-p7-p8-p9
        for i in 1..=steps {
            let t_val = i as f32 / steps as f32;
            let point = Self::cubic_bezier(p6.clone(), p7.clone(), p8.clone(), p9.clone(), t_val);
            path.push(point);
        }
    }

    fn draw_tab_horizontal(
        &mut self,
        path: &mut Vec<Point>,
        y: f32,
        piece_length: f32,
        piece_width: f32,
        flip: bool,
    ) {
        // Draradech algorithm for horizontal tabs
        let t = self.params.tab_size_percent / 200.0;
        let j = self.params.jitter_percent / 100.0;

        let a = self.uniform(-j, j);
        let b = self.uniform(-j, j);
        let c = self.uniform(-j, j);
        let d = self.uniform(-j, j);
        let e = self.uniform(-j, j);

        let sign = if flip { -1.0 } else { 1.0 };

        // For horizontal cuts, swap l and w
        let l = |v: f32| piece_length * v;
        let w = |v: f32| y + piece_width * v * sign;

        let p0 = Point::new(l(0.0), w(0.0));
        let p1 = Point::new(l(0.2), w(a));
        let p2 = Point::new(l(0.5 + b + d), w(-t + c));
        let p3 = Point::new(l(0.5 - t + b), w(t + c));
        let p4 = Point::new(l(0.5 - 2.0 * t + b - d), w(3.0 * t + c));
        let p5 = Point::new(l(0.5 + 2.0 * t + b - d), w(3.0 * t + c));
        let p6 = Point::new(l(0.5 + t + b), w(t + c));
        let p7 = Point::new(l(0.5 + b + d), w(-t + c));
        let p8 = Point::new(l(0.8), w(e));
        let p9 = Point::new(l(1.0), w(0.0));

        let steps = 10;

        for i in 1..=steps {
            let t_val = i as f32 / steps as f32;
            let point = Self::cubic_bezier(p0.clone(), p1.clone(), p2.clone(), p3.clone(), t_val);
            path.push(point);
        }

        for i in 1..=steps {
            let t_val = i as f32 / steps as f32;
            let point = Self::cubic_bezier(p3.clone(), p4.clone(), p5.clone(), p6.clone(), t_val);
            path.push(point);
        }

        for i in 1..=steps {
            let t_val = i as f32 / steps as f32;
            let point = Self::cubic_bezier(p6.clone(), p7.clone(), p8.clone(), p9.clone(), t_val);
            path.push(point);
        }
    }

    fn cubic_bezier(p0: Point, p1: Point, p2: Point, p3: Point, t: f32) -> Point {
        // Cubic Bézier curve formula: B(t) = (1-t)³P0 + 3(1-t)²tP1 + 3(1-t)t²P2 + t³P3
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;

        Point::new(
            mt3 * p0.x + 3.0 * mt2 * t * p1.x + 3.0 * mt * t2 * p2.x + t3 * p3.x,
            mt3 * p0.y + 3.0 * mt2 * t * p1.y + 3.0 * mt * t2 * p2.y + t3 * p3.y,
        )
    }

    /// Return the generated paths as Vec of Vec of (f64, f64) points for UI/Designer consumption.
    pub fn paths_as_f64(&self) -> Vec<Vec<(f64, f64)>> {
        self.paths
            .iter()
            .map(|p| p.iter().map(|pt| (pt.x as f64, pt.y as f64)).collect())
            .collect()
    }
}
