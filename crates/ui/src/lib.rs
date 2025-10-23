// UI crate public surface. This re-exports the generated UI shim used when
// the real Slint-generated modules are not present.
pub mod ui_generated;
// The build script writes `src/ui_impl.rs` which will either include the
// real Slint-generated modules (when `slint_generated` feature is set and
// generation was performed) or re-export the committed `ui_generated` stub.
// We include that file here to expose the public types.
include!("ui_impl.rs");
pub mod slint_bindings;
