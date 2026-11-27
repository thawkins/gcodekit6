//! Surface mesh of a [`HeightMap`]: greedy merging of equal-height cells
//! into quads, side walls between them, and a float budget with block
//! coarsening when a mesh would be too large.

use glam::Vec3;

use super::heightmap::HeightMap;

/// Stock colour (steel grey, opaque).
const STOCK_COLOR: [f32; 4] = [0.44, 0.50, 0.56, 1.0];

/// Floats per mesh vertex: position (3), normal (3), colour (4).
pub const MESH_FLOATS_PER_VERTEX: usize = 10;

/// Largest mesh built, in floats (128 MiB). A map whose mesh would be larger
/// (a finely varying 3D surface on large stock) is meshed from a coarser,
/// block-averaged copy instead; the simulation itself keeps full resolution.
pub const MAX_MESH_FLOATS: usize = 32 << 20;

/// Why a mesh build stopped early.
enum MeshAbort {
    /// The mesh would exceed its float budget.
    OverBudget,
    /// The caller asked to stop.
    Cancelled,
}

/// Collects mesh vertices up to a float budget.
struct MeshBuilder<'a> {
    vertices: Vec<f32>,
    max_floats: usize,
    cancel: &'a dyn Fn() -> bool,
}

impl MeshBuilder<'_> {
    /// Append a quad as two triangles. `corners` must be counter-clockwise
    /// when seen from the side `normal` points to.
    fn push_quad(&mut self, corners: [Vec3; 4], normal: Vec3) -> Result<(), MeshAbort> {
        if self.vertices.len() + 6 * MESH_FLOATS_PER_VERTEX > self.max_floats {
            return Err(MeshAbort::OverBudget);
        }
        for index in [0, 1, 2, 0, 2, 3] {
            let v = corners[index];
            self.vertices
                .extend_from_slice(&[v.x, v.y, v.z, normal.x, normal.y, normal.z]);
            self.vertices.extend_from_slice(&STOCK_COLOR);
        }
        Ok(())
    }

    /// Append a convex polygon as a fan of triangles from its centre.
    /// `outline` must be counter-clockwise when seen from the side `normal`
    /// points to.
    fn push_fan(&mut self, outline: &[Vec3], normal: Vec3) -> Result<(), MeshAbort> {
        let n = outline.len();
        if self.vertices.len() + 3 * n * MESH_FLOATS_PER_VERTEX > self.max_floats {
            return Err(MeshAbort::OverBudget);
        }
        let centre = outline.iter().copied().sum::<Vec3>() / n as f32;
        for k in 0..n {
            for v in [centre, outline[k], outline[(k + 1) % n]] {
                self.vertices
                    .extend_from_slice(&[v.x, v.y, v.z, normal.x, normal.y, normal.z]);
                self.vertices.extend_from_slice(&STOCK_COLOR);
            }
        }
        Ok(())
    }

    /// Fails with [`MeshAbort::Cancelled`] once the caller asks to stop.
    fn check_cancel(&self) -> Result<(), MeshAbort> {
        if (self.cancel)() {
            Err(MeshAbort::Cancelled)
        } else {
            Ok(())
        }
    }

    /// Most quads that still fit in the budget.
    fn quads_left(&self) -> usize {
        (self.max_floats.saturating_sub(self.vertices.len())) / (6 * MESH_FLOATS_PER_VERTEX)
    }
}

/// A merged rectangle of cells: `(col0, row0, col1, row1, key)` with
/// exclusive upper bounds.
type CellRect<K> = (usize, usize, usize, usize, K);

/// Merge the cells of a `cols x rows` grid into rectangles of equal key.
///
/// Greedy meshing: from each unvisited cell with a key, grow a run along X,
/// then grow it along Y while the whole run matches. Cells whose key is
/// `None` are skipped. Stops with [`MeshAbort::OverBudget`] past `max_rects`
/// rectangles, and checks `builder` for cancellation once per row.
fn greedy_rects<K: PartialEq + Copy>(
    cols: usize,
    rows: usize,
    max_rects: usize,
    builder: &MeshBuilder<'_>,
    key: impl Fn(usize, usize) -> Option<K>,
) -> Result<Vec<CellRect<K>>, MeshAbort> {
    let mut visited = vec![false; cols * rows];
    let mut rects = Vec::new();
    for row in 0..rows {
        builder.check_cancel()?;
        for col in 0..cols {
            if visited[row * cols + col] {
                continue;
            }
            let Some(k) = key(col, row) else {
                continue;
            };
            if rects.len() >= max_rects {
                return Err(MeshAbort::OverBudget);
            }
            let matches = |c: usize, r: usize| !visited[r * cols + c] && key(c, r) == Some(k);
            let mut col1 = col + 1;
            while col1 < cols && matches(col1, row) {
                col1 += 1;
            }
            let mut row1 = row + 1;
            while row1 < rows && (col..col1).all(|c| matches(c, row1)) {
                row1 += 1;
            }
            for r in row..row1 {
                visited[r * cols + col..r * cols + col1].fill(true);
            }
            rects.push((col, row, col1, row1, k));
        }
    }
    Ok(rects)
}

/// A corner of a mesh quad on the height map's grid: column edge, row edge and
/// height (mm above the stock bottom).
type GridPoint = (usize, usize, f32);

/// A face of the stock before it is emitted: four grid corners,
/// counter-clockwise seen from the side `normal` points to.
struct GridQuad {
    corners: [GridPoint; 4],
    normal: Vec3,
}

/// Every quad corner, indexed by the grid line it lies on, so an edge can find
/// the corners of other faces lying inside it (T-junctions).
#[derive(Default)]
struct CornerIndex {
    /// Column edges of the corners on each `(row edge, height)` line along X.
    along_x: std::collections::HashMap<(usize, u32), Vec<usize>>,
    /// Row edges of the corners on each `(column edge, height)` line along Y.
    along_y: std::collections::HashMap<(usize, u32), Vec<usize>>,
    /// Heights of the corners on each `(column edge, row edge)` vertical line.
    along_z: std::collections::HashMap<(usize, usize), Vec<f32>>,
}

impl CornerIndex {
    fn new(quads: &[GridQuad]) -> Self {
        let mut index = Self::default();
        for &(c, r, z) in quads.iter().flat_map(|q| q.corners.iter()) {
            index.along_x.entry((r, z.to_bits())).or_default().push(c);
            index.along_y.entry((c, z.to_bits())).or_default().push(r);
            index.along_z.entry((c, r)).or_default().push(z);
        }
        for v in index.along_x.values_mut().chain(index.along_y.values_mut()) {
            v.sort_unstable();
            v.dedup();
        }
        for v in index.along_z.values_mut() {
            v.sort_unstable_by(f32::total_cmp);
            v.dedup();
        }
        index
    }

    /// Corners of any face lying strictly inside the edge `a -> b`, in order
    /// from `a`. Edges are axis-aligned on the grid. Each line's corners are
    /// sorted, so the ones inside are found by binary search, not a scan of
    /// the whole line.
    fn inside(&self, a: GridPoint, b: GridPoint) -> Vec<GridPoint> {
        let mut points: Vec<GridPoint> = if a.0 == b.0 && a.1 == b.1 {
            let (lo, hi) = (a.2.min(b.2), a.2.max(b.2));
            self.along_z.get(&(a.0, a.1)).map_or_else(Vec::new, |zs| {
                strictly_between(zs, |&z| z <= lo, |&z| z < hi)
                    .iter()
                    .map(|&z| (a.0, a.1, z))
                    .collect()
            })
        } else if a.1 == b.1 {
            let (lo, hi) = (a.0.min(b.0), a.0.max(b.0));
            self.along_x
                .get(&(a.1, a.2.to_bits()))
                .map_or_else(Vec::new, |cs| {
                    strictly_between(cs, |&c| c <= lo, |&c| c < hi)
                        .iter()
                        .map(|&c| (c, a.1, a.2))
                        .collect()
                })
        } else {
            let (lo, hi) = (a.1.min(b.1), a.1.max(b.1));
            self.along_y
                .get(&(a.0, a.2.to_bits()))
                .map_or_else(Vec::new, |rs| {
                    strictly_between(rs, |&r| r <= lo, |&r| r < hi)
                        .iter()
                        .map(|&r| (a.0, r, a.2))
                        .collect()
                })
        };
        // Sorted ascending; reverse when the edge runs the other way.
        if (a.0, a.1, a.2) > (b.0, b.1, b.2) {
            points.reverse();
        }
        points
    }
}

/// The run of the sorted `values` above the low end and below the high end of an edge:
/// `at_or_below_lo` holds for the values up to and including the low end, `below_hi` for the
/// values short of the high end.
fn strictly_between<T>(
    values: &[T],
    at_or_below_lo: impl Fn(&T) -> bool,
    below_hi: impl Fn(&T) -> bool,
) -> &[T] {
    let first = values.partition_point(at_or_below_lo);
    let end = values.partition_point(below_hi).max(first);
    &values[first..end]
}

/// Fails with [`MeshAbort::OverBudget`] once `quads` holds more faces than `budget`: each
/// face becomes at least one quad of the mesh, so the list need not grow further. This is
/// the bound `greedy_rects` keeps too: at most `budget` (`quads_left`) faces, exactly
/// `budget` being allowed.
fn check_faces(quads: &[GridQuad], budget: usize) -> Result<(), MeshAbort> {
    if quads.len() > budget {
        Err(MeshAbort::OverBudget)
    } else {
        Ok(())
    }
}

/// The faces of `map` at its own resolution: merged tops, bottoms and walls. Stops with
/// [`MeshAbort::OverBudget`] as soon as there are more faces than quads left in `builder`'s
/// budget, so a map near the budget does not hold a face list larger than the mesh.
fn grid_quads(map: &HeightMap, builder: &MeshBuilder<'_>) -> Result<Vec<GridQuad>, MeshAbort> {
    let (cols, rows) = (map.cols, map.rows);
    let solid = |c: usize, r: usize| map.solid_height(c as isize, r as isize);
    let budget = builder.quads_left();
    let mut quads = Vec::new();

    // Top faces, merged by equal height.
    let tops = greedy_rects(cols, rows, budget, builder, |c, r| {
        let h = solid(c, r);
        (h > 0.0).then_some(h)
    })?;
    for (c0, r0, c1, r1, h) in tops {
        quads.push(GridQuad {
            corners: [(c0, r0, h), (c1, r0, h), (c1, r1, h), (c0, r1, h)],
            normal: Vec3::Z,
        });
    }

    // Bottom faces under every column that still has material.
    let bottoms = greedy_rects(cols, rows, budget - quads.len(), builder, |c, r| {
        (solid(c, r) > 0.0).then_some(())
    })?;
    for (c0, r0, c1, r1, ()) in bottoms {
        quads.push(GridQuad {
            corners: [(c1, r0, 0.0), (c0, r0, 0.0), (c0, r1, 0.0), (c1, r1, 0.0)],
            normal: Vec3::NEG_Z,
        });
    }

    // Walls facing X: boundary `b` lies between columns b-1 and b. The quad
    // corners are the same either way; only the normal (and so the side that
    // is front-facing) depends on which column is taller.
    for b in 0..=cols {
        builder.check_cancel()?;
        let pair = |r: usize| {
            (
                map.solid_height(b as isize - 1, r as isize),
                map.solid_height(b as isize, r as isize),
            )
        };
        let mut row = 0;
        while row < rows {
            let (left, right) = pair(row);
            let mut end = row + 1;
            while end < rows && pair(end) == (left, right) {
                end += 1;
            }
            if left != right {
                quads.push(GridQuad {
                    corners: [
                        (b, row, left),
                        (b, row, right),
                        (b, end, right),
                        (b, end, left),
                    ],
                    normal: if left > right { Vec3::X } else { Vec3::NEG_X },
                });
                check_faces(&quads, budget)?;
            }
            row = end;
        }
    }

    // Walls facing Y: boundary `b` lies between rows b-1 and b.
    for b in 0..=rows {
        builder.check_cancel()?;
        let pair = |c: usize| {
            (
                map.solid_height(c as isize, b as isize - 1),
                map.solid_height(c as isize, b as isize),
            )
        };
        let mut col = 0;
        while col < cols {
            let (below, above) = pair(col);
            let mut end = col + 1;
            while end < cols && pair(end) == (below, above) {
                end += 1;
            }
            if below != above {
                quads.push(GridQuad {
                    corners: [
                        (col, b, below),
                        (end, b, below),
                        (end, b, above),
                        (col, b, above),
                    ],
                    normal: if below > above { Vec3::Y } else { Vec3::NEG_Y },
                });
                check_faces(&quads, budget)?;
            }
            col = end;
        }
    }
    Ok(quads)
}

/// The number of faces (merged tops, bottoms and walls) `map` is meshed from at its own
/// resolution, or `None` when there are more than a mesh of `max_floats` floats can hold.
/// Exposed for tests of the face list's budget.
///
/// # Arguments
/// * `map` - Height map to mesh.
/// * `max_floats` - Largest mesh to build, in floats.
#[doc(hidden)]
pub fn grid_face_count_within(map: &HeightMap, max_floats: usize) -> Option<usize> {
    let builder = MeshBuilder {
        vertices: Vec::new(),
        max_floats,
        cancel: &|| false,
    };
    grid_quads(map, &builder).ok().map(|quads| quads.len())
}

/// Build the mesh of `map` at its own resolution, within `builder`'s budget.
///
/// Merged faces meet neighbours whose corners can lie inside their edges
/// (T-junctions), which rasterise with pinhole cracks. Each such edge is split
/// at those corners, and a face with split edges is drawn as a fan from its
/// centre, so every pair of touching faces shares its vertices.
fn build_mesh(map: &HeightMap, builder: &mut MeshBuilder<'_>) -> Result<(), MeshAbort> {
    let quads = grid_quads(map, builder)?;
    builder.check_cancel()?;
    let index = CornerIndex::new(&quads);
    let position =
        |(c, r, z): GridPoint| Vec3::new(map.edge(c, map.size_x), map.edge(r, map.size_y), z);
    for (n, quad) in quads.iter().enumerate() {
        if n % 4096 == 0 {
            builder.check_cancel()?;
        }
        let mut outline = Vec::with_capacity(4);
        for k in 0..4 {
            let (a, b) = (quad.corners[k], quad.corners[(k + 1) % 4]);
            outline.push(position(a));
            outline.extend(index.inside(a, b).into_iter().map(position));
        }
        if outline.len() == 4 {
            builder.push_quad(
                [outline[0], outline[1], outline[2], outline[3]],
                quad.normal,
            )?;
        } else {
            builder.push_fan(&outline, quad.normal)?;
        }
    }
    Ok(())
}

/// Surface mesh of the remaining stock in stock-local coordinates.
///
/// Flat regions are merged into large quads (see `greedy_rects`), and walls
/// are emitted only where neighbouring columns differ in height, merged along
/// their length. Layout: [`MESH_FLOATS_PER_VERTEX`] floats per vertex, three
/// vertices per triangle, counter-clockwise front faces. A mesh that would
/// exceed [`MAX_MESH_FLOATS`] is built from a coarser copy of the map.
pub fn generate_surface_mesh(map: &HeightMap) -> Vec<f32> {
    generate_surface_mesh_within(map, MAX_MESH_FLOATS, &|| false).unwrap_or_default()
}

/// [`generate_surface_mesh`] with an explicit float budget and cancellation.
///
/// The map is meshed at full resolution if the mesh fits in `max_floats`;
/// otherwise it is averaged over 2x2, 4x4, ... cell blocks until it fits.
///
/// # Arguments
/// * `map` - Height map to mesh.
/// * `max_floats` - Largest mesh to build, in floats.
/// * `cancel` - Polled during meshing; return `true` to stop.
///
/// # Returns
/// The mesh, or `None` when cancelled (or when even a single-cell map does
/// not fit in `max_floats`).
pub fn generate_surface_mesh_within(
    map: &HeightMap,
    max_floats: usize,
    cancel: &dyn Fn() -> bool,
) -> Option<Vec<f32>> {
    let mut factor = 1;
    loop {
        let coarse;
        let current = if factor == 1 {
            map
        } else {
            coarse = map.coarsened(factor);
            &coarse
        };
        let mut builder = MeshBuilder {
            vertices: Vec::new(),
            max_floats,
            cancel,
        };
        match build_mesh(current, &mut builder) {
            Ok(()) => return Some(builder.vertices),
            Err(MeshAbort::Cancelled) => return None,
            Err(MeshAbort::OverBudget) => {
                if current.cols <= 1 && current.rows <= 1 {
                    return None;
                }
                factor *= 2;
                tracing::debug!(factor, "Stock mesh over budget; meshing a coarser map");
            }
        }
    }
}
