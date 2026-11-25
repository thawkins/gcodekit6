//! Saving files safely: the shared atomic write of `gcodekit5-app` (temporary file, flushed,
//! renamed over the target), used by the G-code editor, the designer and the CAM presets.
//!
//! Dependencies: `gcodekit5-app`.

pub(crate) use gcodekit5_app::files::{write_atomic, write_atomic_with};
