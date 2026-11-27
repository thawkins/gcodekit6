//! G-code visualization support
//!
//! This module provides:
//! - Toolpath interpretation (`Visualizer::parse_gcode`)
//! - The orbit camera maths (`Camera3D`)
//! - Height-map stock removal simulation and its surface mesh
//! - Toolpath bounds tracking

pub mod camera;
pub mod stock_removal_3d;
pub mod viewport;

#[allow(clippy::module_inception)]
pub mod visualizer;

pub use camera::Camera as Camera3D;

pub use stock_removal_3d::{
    HeightMap, MAX_MESH_FLOATS, MAX_STOCK_CELLS, StockSimulator3D, StockXyReference,
    StockZeroReference, ToolpathSegment, ToolpathSegmentType, detect_stock_zero_reference,
    generate_surface_mesh, generate_surface_mesh_within, grid_face_count_within,
    toolpath_from_commands,
};
pub use viewport::Bounds;
pub use visualizer::{GCodeCommand, Point3D, Visualizer, is_content_replacement};
