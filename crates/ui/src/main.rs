#[cfg(feature = "with-slint")]
fn main() {
    // Slint UI entrypoint
    slint::include_modules!();
    let ui = ui::MainWindow::new();
    ui.run();
}

#[cfg(not(feature = "with-slint"))]
fn main() {
    // Fallback placeholder when Slint feature is not enabled
    println!("gcodekit-ui: placeholder (run with feature 'with-slint' to enable Slint UI)");
}
