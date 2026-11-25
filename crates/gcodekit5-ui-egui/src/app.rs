//! Placeholder application state and root UI for the egui front end.
//!
//! Dependencies: `egui`, `craft-ui` (widget palette).

/// Flat dark widget palette applied to every widget state.
pub const WIDGET_PALETTE: craft_ui::theme::WidgetPalette = craft_ui::theme::WidgetPalette {
    noninteractive: egui::Color32::from_gray(32),
    inactive: egui::Color32::from_gray(44),
    hovered: egui::Color32::from_gray(60),
    active: egui::Color32::from_gray(76),
    open: egui::Color32::from_gray(52),
};

/// Applies [`WIDGET_PALETTE`] to a set of widget visuals.
///
/// # Arguments
/// * `widgets` - The widget visuals to restyle.
pub fn apply_widget_palette(widgets: &mut egui::style::Widgets) {
    let foreground = egui::Stroke::new(1.0, egui::Color32::from_gray(220));
    WIDGET_PALETTE.apply_to(widgets, egui::CornerRadius::same(3), foreground);
}

/// Root application state of the egui front end.
#[derive(Debug, Default)]
pub struct GcodeKitApp {}

impl GcodeKitApp {
    /// Draws the application UI into `ui`.
    ///
    /// # Arguments
    /// * `ui` - The egui `Ui` to draw into.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        apply_widget_palette(&mut ui.style_mut().visuals.widgets);
        ui.heading("GCodeKit");
    }
}

impl eframe::App for GcodeKitApp {
    /// Draws the central panel hosting the application UI.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui(ui);
    }
}
