//! [`HeightMap`]: the remaining stock as one material height per XY cell,
//! and the flat end mill cut applied to it.

/// Largest number of XY cells the simulation allocates (4 bytes each):
/// 2^27 cells, 512 MiB. A full 1220 x 2440 mm sheet at 0.19 mm needs
/// about 82 M cells.
pub const MAX_STOCK_CELLS: u64 = 1 << 27;

/// Heights below this (mm) count as cut through.
const CUT_THROUGH_EPSILON: f32 = 1e-4;

/// Number of cells covering `size` mm at `resolution` mm per cell. A size that
/// is a whole number of cells (up to float error) gets no extra cell.
fn cell_count(size: f32, resolution: f32) -> u64 {
    ((size / resolution) - 1e-4).ceil().max(1.0) as u64
}

/// Material height per XY cell of a rectangular stock block.
///
/// Cell `(col, row)` covers `col * resolution ..` in X and `row * resolution
/// ..` in Y, clipped to the stock size, so the last column and row may be
/// narrower than `resolution`. Heights are measured from the stock bottom and
/// lie in `0 ..= thickness`.
#[derive(Debug, Clone)]
pub struct HeightMap {
    pub(super) cols: usize,
    pub(super) rows: usize,
    pub(super) resolution: f32,
    pub(super) size_x: f32,
    pub(super) size_y: f32,
    pub(super) thickness: f32,
    heights: Vec<f32>,
}

impl HeightMap {
    /// Checks that a stock of `width x height x thickness` mm can be simulated
    /// at `resolution` mm per cell.
    ///
    /// # Errors
    /// A message when a dimension or the resolution is not a positive finite
    /// number, or the map would exceed [`MAX_STOCK_CELLS`].
    pub fn check_dimensions(
        width: f32,
        height: f32,
        thickness: f32,
        resolution: f32,
    ) -> Result<(), String> {
        for (name, value) in [
            ("width", width),
            ("height", height),
            ("thickness", thickness),
            ("resolution", resolution),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(format!(
                    "Stock {name} must be a positive number (got {value})"
                ));
            }
        }
        let total = cell_count(width, resolution).saturating_mul(cell_count(height, resolution));
        if total > MAX_STOCK_CELLS {
            return Err(format!(
                "Stock {width} x {height} mm is too large to simulate at {resolution} mm \
                 resolution ({total} cells; the limit is {MAX_STOCK_CELLS})"
            ));
        }
        Ok(())
    }

    /// Create an uncut stock block.
    ///
    /// # Arguments
    /// * `width`, `height`, `thickness` - Stock size in mm (X, Y, Z).
    /// * `resolution` - Cell edge length in mm.
    ///
    /// # Errors
    /// See [`HeightMap::check_dimensions`].
    pub fn new(width: f32, height: f32, thickness: f32, resolution: f32) -> Result<Self, String> {
        Self::check_dimensions(width, height, thickness, resolution)?;
        let cols = cell_count(width, resolution) as usize;
        let rows = cell_count(height, resolution) as usize;
        Ok(Self {
            cols,
            rows,
            resolution,
            size_x: width,
            size_y: height,
            thickness,
            heights: vec![thickness; cols * rows],
        })
    }

    /// Number of cells along X.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Number of cells along Y.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Cell edge length (mm).
    pub fn resolution(&self) -> f32 {
        self.resolution
    }

    /// Uncut stock thickness (mm).
    pub fn thickness(&self) -> f32 {
        self.thickness
    }

    /// All cell heights, row-major (`row * cols + col`).
    pub fn heights(&self) -> &[f32] {
        &self.heights
    }

    /// Material height (mm above the stock bottom) of a cell, or `None`
    /// outside the map.
    pub fn height_at(&self, col: usize, row: usize) -> Option<f32> {
        (col < self.cols && row < self.rows).then(|| self.heights[row * self.cols + col])
    }

    /// Lower edge (mm) of cell `index` along an axis of `size` mm, clipped to
    /// the stock.
    pub(super) fn edge(&self, index: usize, size: f32) -> f32 {
        (index as f32 * self.resolution).min(size)
    }

    /// Range of cell indices whose span overlaps `lo ..= hi` (mm) along an
    /// axis with `len` cells, or `None` when it misses the stock.
    fn index_span(&self, lo: f32, hi: f32, len: usize) -> Option<std::ops::RangeInclusive<usize>> {
        let lo = (lo / self.resolution).floor();
        let hi = (hi / self.resolution).floor();
        if lo.is_nan() || hi.is_nan() || hi < 0.0 || lo >= len as f32 {
            return None;
        }
        Some(lo.max(0.0) as usize..=(hi as usize).min(len - 1))
    }

    /// Cut with a flat end mill: every cell whose centre lies within `radius`
    /// of `(x, y)` is lowered to `tip_z`.
    ///
    /// # Arguments
    /// * `x`, `y` - Tool axis position in stock-local mm.
    /// * `tip_z` - Tool tip height above the stock bottom (mm).
    /// * `radius` - Tool radius (mm).
    pub fn cut_flat(&mut self, x: f32, y: f32, tip_z: f32, radius: f32) {
        if tip_z.is_nan() || tip_z >= self.thickness {
            return;
        }
        let new_height = tip_z.max(0.0);
        let (Some(cols), Some(rows)) = (
            self.index_span(x - radius, x + radius, self.cols),
            self.index_span(y - radius, y + radius, self.rows),
        ) else {
            return;
        };
        let r_sq = radius * radius;
        for row in rows {
            let cy = 0.5 * (self.edge(row, self.size_y) + self.edge(row + 1, self.size_y));
            let dy = cy - y;
            for col in cols.clone() {
                let cx = 0.5 * (self.edge(col, self.size_x) + self.edge(col + 1, self.size_x));
                let dx = cx - x;
                if dx * dx + dy * dy <= r_sq {
                    let h = &mut self.heights[row * self.cols + col];
                    if *h > new_height {
                        *h = new_height;
                    }
                }
            }
        }
    }

    /// Copy with each `factor x factor` block of cells averaged into one cell,
    /// for meshing maps too detailed to mesh at full resolution.
    pub(super) fn coarsened(&self, factor: usize) -> Self {
        let cols = self.cols.div_ceil(factor);
        let rows = self.rows.div_ceil(factor);
        let mut heights = Vec::with_capacity(cols * rows);
        for row in 0..rows {
            let (r0, r1) = (row * factor, ((row + 1) * factor).min(self.rows));
            for col in 0..cols {
                let (c0, c1) = (col * factor, ((col + 1) * factor).min(self.cols));
                let sum: f32 = (r0..r1)
                    .map(|r| {
                        self.heights[r * self.cols + c0..r * self.cols + c1]
                            .iter()
                            .sum::<f32>()
                    })
                    .sum();
                heights.push(sum / ((r1 - r0) * (c1 - c0)) as f32);
            }
        }
        Self {
            cols,
            rows,
            resolution: self.resolution * factor as f32,
            size_x: self.size_x,
            size_y: self.size_y,
            thickness: self.thickness,
            heights,
        }
    }

    /// Height with cut-through cells snapped to zero; `0.0` outside the map.
    pub(super) fn solid_height(&self, col: isize, row: isize) -> f32 {
        if col < 0 || row < 0 {
            return 0.0;
        }
        match self.height_at(col as usize, row as usize) {
            Some(h) if h > CUT_THROUGH_EPSILON => h,
            _ => 0.0,
        }
    }
}
