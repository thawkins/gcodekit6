//! # GCodeKit6 Visualizer
//!
//! G-code visualization for GCodeKit6: toolpath interpretation
//! (`Visualizer::parse_gcode`), camera and stock-removal simulation.
#![deny(unsafe_code)]

pub mod gcode;
pub mod visualizer;

pub use visualizer::{
    Camera3D, GCodeCommand, HeightMap, Point3D, StockSimulator3D, ToolpathSegment,
    ToolpathSegmentType, Visualizer, generate_surface_mesh, toolpath_from_commands,
};
