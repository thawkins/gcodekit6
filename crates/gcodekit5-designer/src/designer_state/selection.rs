//! Selection operations for designer state.

use super::DesignerState;
use crate::DrawingMode;

impl DesignerState {
    /// Get number of selected shapes.
    pub fn selected_count(&self) -> usize {
        self.canvas.selected_count()
    }

    /// Deselects all shapes.
    pub fn deselect_all(&mut self) {
        self.canvas.deselect_all();
    }

    /// Selects all shapes.
    pub fn select_all(&mut self) {
        self.canvas.select_all();
    }

    /// Selects shapes within the given rectangle.
    pub fn select_in_rect(&mut self, x: f64, y: f64, width: f64, height: f64, multi_select: bool) {
        if self.canvas.mode() == DrawingMode::Select {
            self.canvas
                .select_in_rect(x, y, width, height, multi_select);
        }
    }
}
