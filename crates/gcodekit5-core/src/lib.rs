//! # GCodeKit5 Core
//!
//! Core types, traits, and utilities for GCodeKit5.
//! Provides the fundamental abstractions for controller management,
//! state machines, events, and data models.
#![deny(unsafe_code)]

pub mod constants;
pub mod core;
pub mod data;
pub mod error;
pub mod files;
pub mod gcode;
pub mod probe;
pub mod types;
pub mod units;

pub use core::{
    OverrideState,
    message::{Message, MessageLevel},
};

pub use data::{
    CNCPoint, CommunicatorState, ControllerState, ControllerStatus, MachineStatus,
    MachineStatusSnapshot, PartialPosition, Position, Units,
};

pub use probe::{
    Axis as ProbeAxis, Corner, Direction as ProbeDirection, ProbePinState, ProbeReport,
    ProbeResult, ProbeRoutine, ProbeType,
};

pub use error::{Error, Result};

pub use gcode::{CommandNumberGenerator, GcodeCommand};

// Re-export type aliases for convenience
pub use types::{
    Callback, DataCallback, ThreadSafe, ThreadSafeRw, ThreadSafeRwMap, ThreadSafeVec, thread_safe,
    thread_safe_rw, thread_safe_vec,
};
