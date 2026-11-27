//! # Device Database
//!
//! Manages CNC device profiles including machine capabilities,
//! work area dimensions, firmware type, and connection parameters.
//! Provides CRUD operations and persistence for the device library.
#![deny(unsafe_code)]

pub mod manager;
pub mod model;

pub use manager::DeviceManager;
pub use model::{AxisLimits, ControllerType, DeviceProfile, DeviceType};
