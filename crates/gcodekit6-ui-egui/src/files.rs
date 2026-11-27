//! Saving files safely: the shared atomic write of `gcodekit6-app` (temporary file, flushed,
//! renamed over the target), used by the G-code editor, the designer and the CAM presets.
//!
//! Dependencies: `gcodekit6-app`.

pub(crate) use gcodekit6_app::files::{write_atomic, write_atomic_with};
