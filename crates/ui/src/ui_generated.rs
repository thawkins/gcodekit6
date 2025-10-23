// Minimal generated UI shim used for CI and --all-features builds.
// This provides a tiny `MainWindow` type with `new()` and `run()` so the
// crate builds even when real Slint-generated modules are not present.
pub struct MainWindow;

impl MainWindow {
    pub fn new() -> Self {
        MainWindow
    }

    pub fn run(&self) {
        // In the real app this would start the UI event loop.
        println!("gcodekit-ui: fake MainWindow running (generated stub)");
    }
}

impl Default for MainWindow {
    fn default() -> Self {
        Self::new()
    }
}
