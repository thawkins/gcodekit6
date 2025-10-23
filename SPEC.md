# Project SPEC: GCodeKit6

GCodeKit6 is a Rust-based, Slint UI desktop application to control fabrication
machines (CNC, laser cutters/engravers, 3D printers). This SPEC summarizes the
project goals and primary requirements.

Goals
- Reliable, safe device control for GRBL, TinyG, Smoothieware, and G2core
- Cross-platform desktop app with responsive UI and robust serial communication
- Modular Rust workspace with clear testing and governance per constitution

Primary requirements
- Device discovery and connection (serial/USB)
- G-code file preview and streaming with pause/resume
- Emergency Stop functionality
- Structured logging and persistent job history

Where to find detailed specs
- Feature specs live in `specs/` per feature (e.g., `specs/001-initial-plan/spec.md`)
- Architecture decisions in `specs/001-initial-plan/research.md`

