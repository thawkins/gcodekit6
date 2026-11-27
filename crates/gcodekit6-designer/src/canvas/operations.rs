//! Shape manipulation operations for Canvas.

use super::types::{Alignment, DrawingObject};
use crate::model::{DesignerShape, Point};

use crate::spatial_index::Bounds;

use super::Canvas;

impl Canvas {
    /// Moves the selected shape by (dx, dy).
    pub fn move_selected(&mut self, dx: f64, dy: f64) {
        let mut updates = Vec::new();

        for obj in self.shape_store.iter_mut() {
            if obj.selected {
                let (old_x1, old_y1, old_x2, old_y2) = obj.get_total_bounds();

                obj.shape.translate(dx, dy);

                let (new_x1, new_y1, new_x2, new_y2) = obj.get_total_bounds();
                updates.push((
                    obj.id,
                    Bounds::new(old_x1, old_y1, old_x2, old_y2),
                    Bounds::new(new_x1, new_y1, new_x2, new_y2),
                ));
            }
        }

        for (id, old_bounds, new_bounds) in updates {
            self.spatial_manager.remove_bounds(id, &old_bounds);
            self.spatial_manager.insert_bounds(id, &new_bounds);
        }
    }

    /// Calculates the deltas (dx, dy) required to align each selected shape according to the specified alignment.
    /// Returns a vector of (shape_id, dx, dy) for each selected shape that needs to move.
    pub fn calculate_alignment_deltas(&self, alignment: Alignment) -> Vec<(u64, f64, f64)> {
        let selected: Vec<_> = self.shape_store.iter().filter(|o| o.selected).collect();
        if selected.is_empty() {
            return Vec::new();
        }

        // 1. Calculate target value
        let target = match alignment {
            Alignment::Left => selected
                .iter()
                .map(|o| o.shape.bounds().0)
                .fold(f64::INFINITY, f64::min),
            Alignment::Right => selected
                .iter()
                .map(|o| o.shape.bounds().2)
                .fold(f64::NEG_INFINITY, f64::max),
            Alignment::CenterHorizontal => {
                let (min_x, max_x) =
                    selected
                        .iter()
                        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), o| {
                            let (x1, _, x2, _) = o.shape.bounds();
                            (min.min(x1), max.max(x2))
                        });
                if min_x.is_infinite() {
                    f64::INFINITY
                } else {
                    (min_x + max_x) / 2.0
                }
            }
            Alignment::Top => selected
                .iter()
                .map(|o| o.shape.bounds().3)
                .fold(f64::NEG_INFINITY, f64::max),
            Alignment::Bottom => selected
                .iter()
                .map(|o| o.shape.bounds().1)
                .fold(f64::INFINITY, f64::min),
            Alignment::CenterVertical => {
                let (min_y, max_y) =
                    selected
                        .iter()
                        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), o| {
                            let (_, y1, _, y2) = o.shape.bounds();
                            (min.min(y1), max.max(y2))
                        });
                if min_y.is_infinite() {
                    f64::INFINITY
                } else {
                    (min_y + max_y) / 2.0
                }
            }
        };

        if target.is_infinite() {
            return Vec::new();
        }

        let mut deltas = Vec::new();

        for obj in selected {
            let (x1, y1, x2, y2) = obj.shape.bounds();
            let (dx, dy) = match alignment {
                Alignment::Left => (target - x1, 0.0),
                Alignment::Right => (target - x2, 0.0),
                Alignment::CenterHorizontal => (target - (x1 + x2) / 2.0, 0.0),
                Alignment::Top => (0.0, target - y2),
                Alignment::Bottom => (0.0, target - y1),
                Alignment::CenterVertical => (0.0, target - (y1 + y2) / 2.0),
            };

            if dx.abs() > f64::EPSILON || dy.abs() > f64::EPSILON {
                deltas.push((obj.id, dx, dy));
            }
        }

        deltas
    }

    /// Calculates position and size updates without modifying the canvas.
    pub fn calculate_position_and_size_updates(
        &self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        update_position: bool,
        update_size: bool,
    ) -> Vec<(u64, DrawingObject)> {
        let mut updates = Vec::new();

        // 1. Calculate union bounding box of all selected items
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut has_selected = false;

        for obj in self.shape_store.iter().filter(|o| o.selected) {
            let (x1, y1, x2, y2) = obj.shape.bounds();
            min_x = min_x.min(x1);
            min_y = min_y.min(y1);
            max_x = max_x.max(x2);
            max_y = max_y.max(y2);
            has_selected = true;
        }

        if !has_selected {
            return updates;
        }

        let old_w = max_x - min_x;
        let old_h = max_y - min_y;

        // Calculate the current center
        let old_center_x = min_x + old_w / 2.0;
        let old_center_y = min_y + old_h / 2.0;

        // 2. Determine target values
        // When update_position is false, preserve the current center (x,y from UI are center coords)
        // When update_position is true, use the new center from x,y
        let target_center_x = if update_position { x } else { old_center_x };
        let target_center_y = if update_position { y } else { old_center_y };
        let target_w = if update_size { w } else { old_w };
        let target_h = if update_size { h } else { old_h };

        // Nothing to do: returning no updates keeps no-op edits (e.g. committing an
        // unchanged property field) from pushing empty undo steps.
        const NO_CHANGE_EPS: f64 = 1e-9;
        if (target_center_x - old_center_x).abs() <= NO_CHANGE_EPS
            && (target_center_y - old_center_y).abs() <= NO_CHANGE_EPS
            && (target_w - old_w).abs() <= NO_CHANGE_EPS
            && (target_h - old_h).abs() <= NO_CHANGE_EPS
        {
            return updates;
        }

        // Calculate target top-left from center
        let target_x = target_center_x - target_w / 2.0;
        let target_y = target_center_y - target_h / 2.0;

        // 3. Calculate scale factors
        let sx = if update_size && old_w.abs() > 1e-6 {
            target_w / old_w
        } else {
            1.0
        };
        let sy = if update_size && old_h.abs() > 1e-6 {
            target_h / old_h
        } else {
            1.0
        };

        // Center of the original group
        let group_center_x = min_x + old_w / 2.0;
        let group_center_y = min_y + old_h / 2.0;

        for obj in self.shape_store.iter().filter(|o| o.selected) {
            let mut new_obj = obj.clone();

            // Apply scaling relative to group center
            new_obj
                .shape
                .scale(sx, sy, Point::new(group_center_x, group_center_y));

            // Calculate translation to move to target position
            // The group's new center after scaling is still (group_center_x, group_center_y)
            // But its size is (target_w, target_h), so its new top-left is:
            let current_new_x = group_center_x - target_w / 2.0;
            let current_new_y = group_center_y - target_h / 2.0;

            let dx = target_x - current_new_x;
            let dy = target_y - current_new_y;

            new_obj.shape.translate(dx, dy);

            updates.push((obj.id, new_obj));
        }

        updates
    }

    pub fn set_selected_position_and_size_with_flags(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        update_position: bool,
        update_size: bool,
    ) -> bool {
        // 1. Calculate union bounding box
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut has_selected = false;

        for obj in self.shape_store.iter().filter(|o| o.selected) {
            let (x1, y1, x2, y2) = obj.shape.bounds();
            min_x = min_x.min(x1);
            min_y = min_y.min(y1);
            max_x = max_x.max(x2);
            max_y = max_y.max(y2);
            has_selected = true;
        }

        if !has_selected {
            return false;
        }

        let old_w = max_x - min_x;
        let old_h = max_y - min_y;

        // Calculate the current center
        let old_center_x = min_x + old_w / 2.0;
        let old_center_y = min_y + old_h / 2.0;

        // 2. Determine target values
        // When update_position is false, preserve the current center (x,y from UI are center coords)
        // When update_position is true, use the new center from x,y
        let target_center_x = if update_position { x } else { old_center_x };
        let target_center_y = if update_position { y } else { old_center_y };
        let target_w = if update_size { w } else { old_w };
        let target_h = if update_size { h } else { old_h };

        // Calculate target top-left from center
        let target_x = target_center_x - target_w / 2.0;
        let target_y = target_center_y - target_h / 2.0;

        // 3. Calculate scale factors
        let sx = if update_size && old_w.abs() > 1e-6 {
            target_w / old_w
        } else {
            1.0
        };
        let sy = if update_size && old_h.abs() > 1e-6 {
            target_h / old_h
        } else {
            1.0
        };

        // Center of the original group
        let group_center_x = min_x + old_w / 2.0;
        let group_center_y = min_y + old_h / 2.0;

        let mut changed_any = false;
        let mut updates = Vec::new();

        for obj in self.shape_store.iter_mut() {
            if !obj.selected {
                continue;
            }

            let (old_x, old_y, old_x2, old_y2) = obj.get_total_bounds();

            // Apply scaling relative to group center
            obj.shape
                .scale(sx, sy, Point::new(group_center_x, group_center_y));

            // Calculate translation to move to target position
            let current_new_x = group_center_x - target_w / 2.0;
            let current_new_y = group_center_y - target_h / 2.0;

            let dx = target_x - current_new_x;
            let dy = target_y - current_new_y;

            obj.shape.translate(dx, dy);

            changed_any = true;

            let (new_x1, new_y1, new_x2, new_y2) = obj.get_total_bounds();
            updates.push((
                obj.id,
                Bounds::new(old_x, old_y, old_x2, old_y2),
                Bounds::new(new_x1, new_y1, new_x2, new_y2),
            ));
        }

        for (id, old_bounds, new_bounds) in updates {
            self.spatial_manager.remove_bounds(id, &old_bounds);
            self.spatial_manager.insert_bounds(id, &new_bounds);
        }

        changed_any
    }
}
