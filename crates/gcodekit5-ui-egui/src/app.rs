//! Placeholder application state and root UI for the egui front end.
//!
//! Dependencies: `egui`.

/// Root application state of the egui front end.
#[derive(Debug, Default)]
pub struct GcodeKitApp {}

impl GcodeKitApp {
    /// Draws the application UI into `ui`.
    ///
    /// # Arguments
    /// * `ui` - The egui `Ui` to draw into.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("GCodeKit");
    }
}

impl eframe::App for GcodeKitApp {
    /// Draws the central panel hosting the application UI.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui(ui);
    }
}
