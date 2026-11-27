//! # 3D Stock Removal Simulation
//!
//! Simulates material removal from a rectangular stock block as the toolpath
//! is executed, and builds a surface mesh of what is left.
//!
//! ## Model
//! The cutter is a flat end mill, which removes everything above its tip.
//! Every cut therefore lowers the top of a stock column and never makes an
//! undercut, so the remaining stock is fully described by a [`HeightMap`]:
//! one material height per XY cell. Heights are exact (not quantised to the
//! cell size); a cell is cut when its centre lies under the tool.
//!
//! ## Coordinate convention
//! [`StockSimulator3D`] takes toolpaths in G-code work coordinates (mm).
//! * XY: the stock's lower-left corner is at `stock_origin_xy` (work
//!   `(0, 0)` by default), so the stock covers
//!   `origin.x ..= origin.x + width` and `origin.y ..= origin.y + height`.
//! * Z: the top face of the stock is at `stock_top_z` (Z0 by default, as most
//!   CAM output assumes; see [`StockZeroReference`] for programs zeroed on the
//!   stock bottom), so the material occupies
//!   `stock_top_z - thickness ..= stock_top_z` and material is removed only
//!   where the tool tip is below the stock top.
//!
//! The [`HeightMap`] itself uses stock-local coordinates (lower-left bottom
//! corner at the origin); [`StockSimulator3D::get_mesh`] translates the mesh
//! back into work coordinates.

mod heightmap;
mod mesh;
mod simulator;
mod toolpath;
mod zero_reference;

pub use heightmap::{HeightMap, MAX_STOCK_CELLS};
pub use mesh::{
    MAX_MESH_FLOATS, MESH_FLOATS_PER_VERTEX, generate_surface_mesh, generate_surface_mesh_within,
    grid_face_count_within,
};
pub use simulator::StockSimulator3D;
pub use toolpath::{ToolpathSegment, ToolpathSegmentType, toolpath_from_commands};
pub use zero_reference::{StockXyReference, StockZeroReference, detect_stock_zero_reference};
