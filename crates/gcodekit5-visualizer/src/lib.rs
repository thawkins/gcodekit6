//! # GCodeKit5 Visualizer
//!
//! G-code visualization for GCodeKit5: toolpath interpretation
//! (`Visualizer::parse_gcode`), camera and stock-removal simulation.
#![deny(unsafe_code)]

pub mod gcode;
pub mod helpers;
pub mod visualizer;

pub use visualizer::{
    Camera, Camera3D, GCodeCommand, HeightMap, Point3D, Renderer, Scene, StockSimulator3D,
    ToolpathSegment, ToolpathSegmentType, Visualizer, VisualizerControls, generate_surface_mesh,
    toolpath_from_commands,
};
