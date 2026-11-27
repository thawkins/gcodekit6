//! Shapes a CAM page generates for the design: the jigsaw puzzle's outlines and the tabbed
//! box's panels.
//!
//! The outlines are added as one undo step, each as a path through its points (closed or
//! open as the page made it); a page that asks for it has them grouped, so they move
//! together (the puzzle is grouped). The view then fits the design.
//!
//! Dependencies: `gcodekit6-designer` (model).

use gcodekit6_designer::DesignerState;
use gcodekit6_designer::commands::{DesignerCommand, PasteShapes};
use gcodekit6_designer::model::{DesignPath, Point, Shape};

use super::DesignerPane;
use super::canvas::editing;
use crate::panes::cam_tools::requests::DesignerPaths;

impl DesignerPane {
    /// Adds the outlines of `generated` to the design as one undo step and fits the view.
    ///
    /// # Arguments
    /// * `generated` - The outlines a CAM page made.
    ///
    /// # Returns
    /// The number of shapes added.
    pub fn add_generated_paths(&mut self, generated: &DesignerPaths) -> usize {
        // The model gives a new shape its mode defaults only through `add_shape_with_undo`,
        // so each shape is made in a scratch design with the same defaults, then all are
        // added here with one `PasteShapes` command.
        let mut scratch = DesignerState::new();
        scratch.default_properties_shape = self.state.default_properties_shape.clone();
        let group =
            (generated.group && generated.paths.len() > 1).then(|| self.state.canvas.generate_id());
        let mut objects = Vec::new();
        for path in &generated.paths {
            let points: Vec<Point> = path
                .points
                .iter()
                .map(|(x, y)| Point::new(*x, *y))
                .collect();
            let shape = Shape::Path(DesignPath::from_points(&points, path.closed));
            let id = scratch.add_shape_with_undo(shape);
            if let Some(mut obj) = scratch.canvas.get_shape(id).cloned() {
                obj.id = self.state.canvas.generate_id();
                obj.group_id = group;
                objects.push(obj);
            }
        }
        let added = objects.len();
        if added > 0 {
            self.touch();
            self.state
                .push_command(DesignerCommand::PasteShapes(PasteShapes {
                    ids: objects.iter().map(|o| o.id).collect(),
                    objects: objects.into_iter().map(Some).collect(),
                }));
            let area = self.work_area();
            editing::zoom_fit(&mut self.state, area);
        }
        added
    }
}
