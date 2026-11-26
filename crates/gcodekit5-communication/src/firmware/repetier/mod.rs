//! gcodekit5-communication/src/firmware/repetier/mod.rs
//! # Repetier Protocol Firmware Module
//!
//! The Repetier stream processor: line numbering and checksums, translation of
//! GRBL-style commands, and acknowledgement and resend detection. The app's
//! `MachineController` streams with it.

pub mod processor;
pub use processor::RepetierProcessor;
