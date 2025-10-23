// Prefer the real Slint-generated UI when both features are enabled. Otherwise
// fall back to a minimal generated shim that allows the crate to compile and
// run under CI `--all-features` builds.
// Include the build-time generated file which either includes real Slint
// modules or re-exports the local generated stub.
include!("ui_impl.rs");

fn main() {
    let ui = MainWindow::new();
    ui.run();
}
