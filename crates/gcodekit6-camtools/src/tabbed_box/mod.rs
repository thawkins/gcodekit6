//! Tabbed Box Maker
//!
//! Based on the superior algorithm from <https://github.com/florianfesti/boxes>
//! Uses finger/space multiples of thickness for automatic finger calculation

mod finger_styles;
mod types;

pub use types::*;

/// The plain layout's rows: which panels (group indices) go side by side, row by row.
///
/// Panels are drawn at a provisional origin and moved into place by
/// [`TabbedBoxMaker::layout_rows`] once their real outlines are known.
#[derive(Clone, Debug, Default)]
struct LayoutRows {
    rows: Vec<Vec<usize>>,
}

/// The smallest box width, depth or height accepted (mm).
pub const MIN_SIDE_MM: f64 = 20.0;
/// The thinnest material accepted (mm).
pub const MIN_THICKNESS_MM: f64 = 1.0;
/// The thickest material accepted (mm).
pub const MAX_THICKNESS_MM: f64 = 20.0;
/// The smallest accepted magnitude of finger plus space (in thicknesses).
pub const MIN_JOINT_WIDTH: f64 = 0.1;
/// The fewest laser passes accepted.
pub const MIN_LASER_PASSES: f64 = 1.0;
/// The smallest gap left between neighbouring panels (mm).
pub const MIN_PANEL_GAP_MM: f64 = 5.0;
/// The material left standing between two neighbouring cuts (mm).
const PANEL_WEB_MM: f64 = 1.0;

/// The gap between neighbouring panels for a burn (tool or beam diameter) of `burn` mm.
///
/// Both layouts measure it between the panels' cut extents: the bounding box of each
/// outline (the tool centre line, fingers and slots included) grown by half a burn, the
/// outer edge of the cut. The gap is therefore the material left standing between two
/// neighbouring cuts: at least [`MIN_PANEL_GAP_MM`], and at least `2 * burn +
/// PANEL_WEB_MM`, so a wide router bit leaves a web wider than the bit. The default
/// 0.1 mm burn gives 5 mm.
///
/// # Arguments
/// * `burn` - the burn / tool diameter in mm.
///
/// # Returns
/// The gap in mm.
pub fn panel_gap(burn: f64) -> f64 {
    MIN_PANEL_GAP_MM.max(2.0 * burn + PANEL_WEB_MM)
}

impl LayoutRows {
    /// Adds panel group `group` to the current row.
    fn place(&mut self, group: usize) {
        match self.rows.last_mut() {
            Some(row) => row.push(group),
            None => self.rows.push(vec![group]),
        }
    }

    /// Starts a new row.
    fn next_row(&mut self) {
        self.rows.push(Vec::new());
    }
}

pub struct TabbedBoxMaker {
    params: BoxParameters,
    x: f64,
    y: f64,
    h: f64,
    t: f64,
    paths: Vec<Vec<Point>>,
    /// Human-readable label of each entry in `paths` (same indices).
    path_labels: Vec<String>,
    path_groups: Vec<Vec<usize>>,
    /// Panel name of the group currently being built.
    current_group_name: String,
}

impl TabbedBoxMaker {
    pub fn new(params: BoxParameters) -> Result<Self, String> {
        Self::validate_parameters(&params)?;

        let mut x = params.x;
        let mut y = params.y;
        let mut h = params.h;

        let t = params.thickness;

        if params.outside {
            x = Self::adjust_size(x, t);
            y = Self::adjust_size(y, t);
            h = Self::adjust_size(h, t);
        }

        let maker = Self {
            params,
            x,
            y,
            h,
            t,
            paths: Vec::new(),
            path_labels: Vec::new(),
            path_groups: Vec::new(),
            current_group_name: String::new(),
        };
        maker.validate_finger_style()?;
        Ok(maker)
    }

    /// Checks that the Springs, Barbs or Snap fingers fit the joint: a shape that does not
    /// fit would run into the next finger, the panel corner or the opposite edge.
    fn validate_finger_style(&self) -> Result<(), String> {
        let t = self.t;
        let p = &self.params;
        let kerf = p.burn;
        let finger = p.finger_joint.finger * t + kerf;
        let space = p.finger_joint.space * t - kerf;
        let finger_h = t + p.finger_joint.extra_length * t;
        let sides = [self.x, self.y, self.h];
        // Half the leftover beside the end fingers of every edge length, as drawn.
        let margins = sides.map(|side| {
            let (fingers, leftover) = self.calc_fingers(side);
            (fingers, (leftover - kerf) / 2.0)
        });
        match p.finger_joint.style {
            FingerStyle::Rectangular | FingerStyle::Dogbone => Ok(()),
            FingerStyle::Springs => {
                if finger <= finger_styles::springs_min_finger(finger_h, t) {
                    return Err("Fingers are too narrow for the Springs style".to_string());
                }
                Ok(())
            }
            FingerStyle::Barbs => {
                let barb = finger_styles::BARB_DEPTH * t;
                if finger_styles::barb_count(finger_h, t) == 0 {
                    return Err("Fingers are too short for the Barbs style".to_string());
                }
                if space <= 2.0 * barb || margins.iter().any(|&(n, m)| n > 0 && m <= barb) {
                    return Err("Spaces are too narrow for the Barbs style".to_string());
                }
                Ok(())
            }
            FingerStyle::Snap => {
                if p.dividers_x > 0 || p.dividers_y > 0 {
                    return Err("The Snap style cannot be used with dividers".to_string());
                }
                if finger <= finger_styles::SNAP_MIN_FINGER * t {
                    return Err("Fingers are too narrow for the Snap style".to_string());
                }
                // Two facing hooks stand out half a thickness each, beyond their slits.
                let slits = 2.0 * finger_styles::SNAP_OUTER_SLIT * t;
                if space <= t + slits {
                    return Err("Spaces are too narrow for the Snap style".to_string());
                }
                // The relief slits of two opposite edges must not meet.
                let relief = finger_styles::SNAP_RELIEF * t;
                if sides.iter().any(|&side| side <= 2.0 * (relief + t)) {
                    return Err("The box is too small for the Snap style".to_string());
                }
                // Nor may they reach the slits of the edge round the corner.
                let clear = relief + 2.0 * finger_styles::SNAP_OUTER_SLIT * t;
                let too_close = margins.iter().any(|&(n, m)| {
                    let nearest = match n {
                        0 => f64::INFINITY,
                        1 => m - finger_styles::SNAP_OUTER_SLIT * t,
                        _ => m + finger - finger_styles::SNAP_SLIT_WALL * t,
                    };
                    nearest <= clear
                });
                if too_close {
                    return Err(
                        "The end fingers are too close to the corners for the Snap style"
                            .to_string(),
                    );
                }
                Ok(())
            }
        }
    }

    /// One finger-joint edge of `length` in the edge's own frame (x along the edge, y
    /// negative outside the panel), as the panels use it: `positive` for fingers, else
    /// notches. For tests that compare mating edges.
    #[doc(hidden)]
    pub fn finger_edge(&self, length: f64, positive: bool) -> Vec<Point> {
        self.draw_finger_edge(length, positive, true)
    }

    fn validate_parameters(params: &BoxParameters) -> Result<(), String> {
        // `NaN < 20.0` is false, so non-numbers must be rejected explicitly.
        let numbers = [
            params.x,
            params.y,
            params.h,
            params.thickness,
            params.z_step_down,
            params.finger_joint.space,
            params.finger_joint.finger,
            params.finger_joint.surrounding_spaces,
            params.finger_joint.play,
            params.finger_joint.extra_length,
            params.finger_joint.dimple_height,
            params.finger_joint.dimple_length,
            params.burn,
            params.feed_rate,
            params.offset_x,
            params.offset_y,
        ];
        if numbers.iter().any(|v| !v.is_finite()) {
            return Err("All dimensions must be numbers".to_string());
        }

        if params.laser_passes < MIN_LASER_PASSES as i32 {
            return Err("Laser passes must be at least 1".to_string());
        }

        if params.z_step_down < 0.0 {
            return Err("Z step down must not be negative".to_string());
        }

        if params.x < MIN_SIDE_MM || params.y < MIN_SIDE_MM || params.h < MIN_SIDE_MM {
            return Err("All dimensions must be at least 20mm".to_string());
        }

        if params.thickness < MIN_THICKNESS_MM || params.thickness > MAX_THICKNESS_MM {
            return Err("Material thickness must be between 1mm and 20mm".to_string());
        }

        if (params.finger_joint.space + params.finger_joint.finger).abs() < MIN_JOINT_WIDTH {
            return Err("Finger + space must not be close to zero".to_string());
        }

        // A zero or negative feed emits `F0`/`F-n` and a negative power `S-n`: GRBL
        // rejects both mid-job, after the head has already moved.
        if params.feed_rate <= 0.0 {
            return Err("Feed rate must be greater than 0".to_string());
        }

        if params.laser_power < 0 {
            return Err("Laser power must not be negative".to_string());
        }

        // The finger edge draws `space - burn` and the notch edge `finger + play - burn`
        // and `space - play + burn`; a burn or play that makes any of them negative walks
        // the path backwards and the outline crosses itself.
        let space = params.finger_joint.space * params.thickness;
        let finger = params.finger_joint.finger * params.thickness;
        if params.burn < 0.0 || params.burn >= space || params.burn >= finger {
            return Err(
                "Burn must not be negative and must be less than the finger and \
                 space widths"
                    .to_string(),
            );
        }

        let play = params.finger_joint.play * params.thickness;
        if finger + play - params.burn <= 0.0 || space - play + params.burn <= 0.0 {
            return Err("Play is too large for the finger and space widths".to_string());
        }

        Ok(())
    }

    fn adjust_size(size: f64, thickness: f64) -> f64 {
        size - 2.0 * thickness
    }

    /// Calculate number of fingers and leftover space for a given length
    pub fn calc_fingers(&self, length: f64) -> (usize, f64) {
        let settings = &self.params.finger_joint;
        let t = self.t;

        let space = settings.space * t;
        let finger = settings.finger * t;

        // Calculate number of fingers that fit
        let mut fingers = ((length - (settings.surrounding_spaces - 1.0) * space)
            / (space + finger))
            .floor() as usize;

        // Shrink surrounding space up to half thickness each side if needed
        if fingers == 0 && length > finger + 1.0 * t {
            fingers = 1;
        }

        if finger == 0.0 {
            fingers = 0;
        }

        // Calculate leftover space
        let leftover = if fingers > 0 {
            length - (fingers as f64) * (space + finger) + space
        } else {
            length
        };

        (fingers, leftover)
    }

    /// Draw finger joint edge.
    ///
    /// `styled` false draws plain fingers whatever the style (an edge that gets half-lap
    /// slots cut into it afterwards).
    fn draw_finger_edge(&self, length: f64, positive: bool, styled: bool) -> Vec<Point> {
        let mut path = Vec::new();
        let settings = &self.params.finger_joint;
        let t = self.t;

        let mut space = settings.space * t;
        let mut finger = settings.finger * t;
        let play = settings.play * t;
        let extra = settings.extra_length * t;
        let kerf = self.params.burn;
        let half_kerf = kerf / 2.0;
        let dogbone = settings.style == FingerStyle::Dogbone;
        let dimple_h = settings.dimple_height;
        let dimple_l = settings.dimple_length;
        // Overcut for dogbone: usually tool radius. Assuming burn is tool diameter.
        let overcut = half_kerf;

        let (fingers, mut leftover) = self.calc_fingers(length);

        // Adjust for play
        if !positive {
            finger += play;
            space -= play;
            leftover -= play;
        }

        let mut x = 0.0;

        let base_y = -half_kerf;

        let (finger_draw, space_draw, leftover_draw, tip_y) = if positive {
            // Fingers out
            // Finger width increases by kerf
            (
                finger + kerf,
                space - kerf,
                leftover - kerf,
                -t - extra - half_kerf,
            )
        } else {
            // Notches in
            // Notch width decreases by kerf
            (finger - kerf, space + kerf, leftover + kerf, t - half_kerf)
        };

        // Springs, Barbs and Snap reshape the fingers only; the notches stay plain.
        let finger_h = base_y - tip_y;
        let shape = |i: usize| -> Option<Vec<(f64, f64)>> {
            if !positive || !styled {
                return None;
            }
            match settings.style {
                FingerStyle::Springs => Some(finger_styles::springs(finger_draw, finger_h, t)),
                FingerStyle::Barbs => Some(finger_styles::barbs(finger_draw, finger_h, t)),
                // As boxes.py: hooks on the first half face right, on the second half left,
                // so every hook faces the middle of the edge.
                FingerStyle::Snap => Some(finger_styles::snap(
                    finger_draw,
                    finger_h,
                    t,
                    i < fingers / 2,
                )),
                FingerStyle::Rectangular | FingerStyle::Dogbone => None,
            }
        };

        // Helper for dimpled side
        // Draws a side from (x, y1) to (x, y2) with a dimple if configured
        let draw_side = |path: &mut Vec<Point>, x: f64, y1: f64, y2: f64| {
            if dimple_h > 0.0 && dimple_l > 0.0 && (y2 - y1).abs() > dimple_l {
                let mid_y = (y1 + y2) / 2.0;
                let half_l = dimple_l / 2.0;
                let dir = if y2 > y1 { 1.0 } else { -1.0 };

                // Start of side
                path.push(Point::new(x, y1));

                // Start of dimple
                let d_start_y = mid_y - half_l * dir;
                path.push(Point::new(x, d_start_y));

                // Dimple peak
                // Dimple sticks out from the finger side.
                // If positive (finger), side goes out (tip_y) to in (base_y) or vice versa.
                // We want the dimple to bulge OUT of the finger material.
                // If positive:
                //   Left side: base -> tip. Bulge is -x direction? No, +x is along edge.
                //   Wait, x is along the edge length. y is depth.
                //   The side is vertical (constant x).
                //   We want the dimple to change x.

                // Let's assume dimple bulges OUTWARDS from the finger center.
                // But here we are drawing the outline.
                // If positive (finger), the material is "inside" the finger.
                // Left side of finger: x increases. Side is at x. Material is at x+? No.
                // We are drawing the path.
                // Finger: (x, base) -> (x, tip) -> (x+w, tip) -> (x+w, base).
                // Left side: (x, base) -> (x, tip). Material is to the right (x increasing).
                // So bulge should be to the left (-x).
                // Right side: (x+w, tip) -> (x+w, base). Material is to the left.
                // So bulge should be to the right (+x).

                // If negative (notch):
                // (x, base) -> (x, tip) -> (x+w, tip) -> (x+w, base).
                // Left side: (x, base) -> (x, tip). Material is to the left (it's a hole).
                // So bulge should be to the right (+x) (into the hole, making the hole smaller/friction).

                // Wait, for friction fit:
                // Finger: dimple sticks OUT (wider finger).
                // Notch: dimple sticks IN (narrower hole).

                // So:
                // Left side (base->tip):
                //   Positive: Bulge Left (-x).
                //   Negative: Bulge Right (+x).
                // Right side (tip->base):
                //   Positive: Bulge Right (+x).
                //   Negative: Bulge Left (-x).

                let bulge_dir = if y2 < y1 {
                    // tip -> base (Right side)
                    if positive { 1.0 } else { -1.0 }
                } else {
                    // base -> tip (Left side)
                    if positive { -1.0 } else { 1.0 }
                };

                path.push(Point::new(x + dimple_h * bulge_dir, mid_y));

                // End of dimple
                let d_end_y = mid_y + half_l * dir;
                path.push(Point::new(x, d_end_y));

                // End of side
                path.push(Point::new(x, y2));
            } else {
                path.push(Point::new(x, y1));
                path.push(Point::new(x, y2));
            }
        };

        // Start point
        path.push(Point::new(x, base_y));
        x += leftover_draw / 2.0;
        path.push(Point::new(x, base_y));

        // Draw fingers
        for i in 0..fingers {
            if let Some(points) = shape(i) {
                // The shape starts on the panel edge itself (a snap outer slit starts before
                // `x`), so drop the edge point just placed at the finger's root.
                if path
                    .last()
                    .is_some_and(|p: &Point| p.x == x && p.y == base_y)
                {
                    path.pop();
                }
                path.extend(
                    points
                        .into_iter()
                        .map(|(u, v)| Point::new(x + u, base_y - v)),
                );
                x += finger_draw;
            } else if positive {
                // Finger protrudes
                // Left side: base -> tip
                draw_side(&mut path, x, base_y, tip_y);

                x += finger_draw;
                path.push(Point::new(x, tip_y));

                // Right side: tip -> base
                draw_side(&mut path, x, tip_y, base_y);
            } else {
                // Notch for finger
                // Left side: base -> tip
                // path.push(Point::new(x, tip_y)); // Replaced by draw_side

                if dogbone {
                    // Dogbone overcut at first corner
                    path.push(Point::new(x, base_y)); // Ensure we start at base
                    path.push(Point::new(x, tip_y + overcut)); // Go past tip
                    path.push(Point::new(x - overcut, tip_y + overcut)); // Dogbone out
                    path.push(Point::new(x, tip_y)); // Back to corner
                } else {
                    draw_side(&mut path, x, base_y, tip_y);
                }

                x += finger_draw;
                path.push(Point::new(x, tip_y));

                if dogbone {
                    // Dogbone overcut at second corner
                    path.push(Point::new(x + overcut, tip_y + overcut));
                    path.push(Point::new(x, tip_y + overcut));
                    path.push(Point::new(x, base_y)); // Back to base
                } else {
                    draw_side(&mut path, x, tip_y, base_y);
                }
            }

            // Space between fingers
            if i < fingers - 1 {
                x += space_draw;
                path.push(Point::new(x, base_y));
            }
        }

        // End with leftover/2
        x += leftover_draw / 2.0;
        path.push(Point::new(x, base_y));

        path
    }

    /// Draw a rectangular wall with finger joints on specified edges
    /// edges: 4-char string, each char: 'f' = finger out, 'F' = finger in, 'e' = plain edge
    /// Edges: [0]=bottom, [1]=right, [2]=top, [3]=left
    fn draw_rectangular_wall(
        &self,
        width: f64,
        height: f64,
        edges: &str,
        start_x: f64,
        start_y: f64,
    ) -> Vec<Point> {
        let mut path: Vec<Point> = Vec::new();
        let edge_chars: Vec<char> = edges.chars().collect();
        let kerf = self.params.burn;
        let half_kerf = kerf / 2.0;

        // Bottom edge: left to right (0,0) -> (width,0)
        if let Some(&c) = edge_chars.first() {
            if c == 'f' || c == 'F' {
                let base_path = self.draw_finger_edge(width, c == 'f', true);
                for p in &base_path {
                    push_unique_point(&mut path, Point::new(start_x + p.x, start_y + p.y));
                }
            } else {
                // Plain edge. Offset outwards (down)
                push_unique_point(
                    &mut path,
                    Point::new(start_x - half_kerf, start_y - half_kerf),
                );
                push_unique_point(
                    &mut path,
                    Point::new(start_x + width + half_kerf, start_y - half_kerf),
                );
            }
        }
        // Ensure corner 1 is closed
        push_unique_point(
            &mut path,
            Point::new(start_x + width + half_kerf, start_y - half_kerf),
        );

        // Right edge: bottom to top (width,0) -> (width,height)
        if let Some(&c) = edge_chars.get(1) {
            if c == 'f' || c == 'F' {
                let base_path = self.draw_finger_edge(height, c == 'f', true);
                for p in &base_path {
                    push_unique_point(&mut path, Point::new(start_x + width - p.y, start_y + p.x));
                }
            } else {
                // Plain edge. Offset outwards (right)
                push_unique_point(
                    &mut path,
                    Point::new(start_x + width + half_kerf, start_y - half_kerf),
                );
                push_unique_point(
                    &mut path,
                    Point::new(start_x + width + half_kerf, start_y + height + half_kerf),
                );
            }
        }
        // Ensure corner 2 is closed
        push_unique_point(
            &mut path,
            Point::new(start_x + width + half_kerf, start_y + height + half_kerf),
        );

        // Top edge: right to left (width,height) -> (0,height)
        if let Some(&c) = edge_chars.get(2) {
            if c == 'F' || c == 'f' {
                let base_path = self.draw_finger_edge(width, c == 'f', true);
                for p in &base_path {
                    push_unique_point(
                        &mut path,
                        Point::new(start_x + width - p.x, start_y + height - p.y),
                    );
                }
            } else {
                // Plain edge. Offset outwards (up)
                push_unique_point(
                    &mut path,
                    Point::new(start_x + width + half_kerf, start_y + height + half_kerf),
                );
                push_unique_point(
                    &mut path,
                    Point::new(start_x - half_kerf, start_y + height + half_kerf),
                );
            }
        }
        // Ensure corner 3 is closed
        push_unique_point(
            &mut path,
            Point::new(start_x - half_kerf, start_y + height + half_kerf),
        );

        // Left edge: top to bottom (0,height) -> (0,0)
        if let Some(&c) = edge_chars.get(3) {
            if c == 'f' || c == 'F' {
                let base_path = self.draw_finger_edge(height, c == 'f', true);
                for p in &base_path {
                    push_unique_point(&mut path, Point::new(start_x + p.y, start_y + height - p.x));
                }
            } else {
                // Plain edge. Offset outwards (left)
                push_unique_point(
                    &mut path,
                    Point::new(start_x - half_kerf, start_y + height + half_kerf),
                );
                push_unique_point(
                    &mut path,
                    Point::new(start_x - half_kerf, start_y - half_kerf),
                );
            }
        }
        // Ensure corner 4 is closed (back to start)
        push_unique_point(
            &mut path,
            Point::new(start_x - half_kerf, start_y - half_kerf),
        );

        // Ensure closed loop by connecting back to the very first point
        if let Some(first) = path.first().cloned() {
            push_unique_point(&mut path, first);
        }

        path
    }

    fn pack_paths(&mut self) {
        if self.paths.is_empty() {
            return;
        }

        struct Item {
            group_index: usize,
            width: f64,
            height: f64,
            original_min_x: f64,
            original_min_y: f64,
        }

        let mut items = Vec::new();
        let spacing = panel_gap(self.params.burn);

        // If path_groups is empty (legacy or bug), treat each path as a group
        if self.path_groups.is_empty() {
            for i in 0..self.paths.len() {
                self.path_groups.push(vec![i]);
            }
        }

        for i in 0..self.path_groups.len() {
            let Some((min_x, min_y, max_x, max_y)) = self.group_extent(i) else {
                continue;
            };
            items.push(Item {
                group_index: i,
                width: max_x - min_x,
                height: max_y - min_y,
                original_min_x: min_x,
                original_min_y: min_y,
            });
        }

        // Sort by height descending. Heights within 0.1 um of each other (panels of one size
        // whose outlines differ only by rounding) are a tie and keep their generation order;
        // the sort is stable.
        items.sort_by_key(|item| std::cmp::Reverse((item.height * 1e4).round() as i64));

        // Estimate target width as sqrt of total area * 1.5 (aspect ratio preference)
        // Or just ensure it's at least as wide as the widest item
        let total_area: f64 = items.iter().map(|i| i.width * i.height).sum();
        let max_item_width = items.iter().map(|i| i.width).fold(0.0, f64::max);
        let target_width = (total_area.sqrt() * 1.5).max(max_item_width);

        let mut current_x = 0.0;
        let mut current_y = 0.0;
        let mut row_height: f64 = 0.0;

        // Store new positions: group_index -> (x, y)
        let mut new_positions = vec![(0.0, 0.0); self.path_groups.len()];

        for item in &items {
            if current_x > 0.0 && current_x + item.width > target_width {
                // New row
                current_x = 0.0;
                current_y += row_height + spacing;
                row_height = 0.0;
            }

            new_positions[item.group_index] = (current_x, current_y);

            row_height = row_height.max(item.height);
            current_x += item.width + spacing;
        }

        // Apply new positions
        for item in items {
            let (new_x, new_y) = new_positions[item.group_index];
            self.translate_group(
                item.group_index,
                new_x - item.original_min_x,
                new_y - item.original_min_y,
            );
        }
    }

    /// The cut extent of panel group `group`: the bounding box `(min_x, min_y, max_x, max_y)`
    /// of its paths (tool centre lines, fingers and slots included) grown by half a burn,
    /// the outer edge of the cut. `None` for a group without points.
    fn group_extent(&self, group: usize) -> Option<(f64, f64, f64, f64)> {
        let points = self
            .path_groups
            .get(group)?
            .iter()
            .filter_map(|&i| self.paths.get(i))
            .flatten();
        let (min_x, min_y, max_x, max_y) = points.fold(
            (
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ),
            |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
        );
        if min_x == f64::INFINITY {
            return None;
        }
        let half_burn = self.params.burn / 2.0;
        Some((
            min_x - half_burn,
            min_y - half_burn,
            max_x + half_burn,
            max_y + half_burn,
        ))
    }

    /// Moves every path of panel group `group` by `(dx, dy)`.
    fn translate_group(&mut self, group: usize, dx: f64, dy: f64) {
        let Some(indices) = self.path_groups.get(group) else {
            return;
        };
        for &i in indices {
            if let Some(path) = self.paths.get_mut(i) {
                for p in path {
                    p.x += dx;
                    p.y += dy;
                }
            }
        }
    }

    /// Places the plain layout: the panels of each row side by side, left to right, rows
    /// bottom to top, with [`panel_gap`] between the cut extents ([`Self::group_extent`]) of
    /// neighbouring panels and rows, so fingers that stick out past a panel edge are counted.
    fn layout_rows(&mut self, layout: &LayoutRows) {
        let gap = panel_gap(self.params.burn);
        let mut y = 0.0;
        for row in &layout.rows {
            let mut x = 0.0;
            let mut row_height: f64 = 0.0;
            for &group in row {
                let Some((min_x, min_y, max_x, max_y)) = self.group_extent(group) else {
                    continue;
                };
                self.translate_group(group, x - min_x, y - min_y);
                x += max_x - min_x + gap;
                row_height = row_height.max(max_y - min_y);
            }
            if row_height > 0.0 {
                y += row_height + gap;
            }
        }
    }

    fn apply_slots_to_path(
        &self,
        path: Vec<Point>,
        slots: &[f64],
        depth: f64,
        slot_width: f64,
    ) -> Vec<Point> {
        let mut new_path = Vec::new();
        let mut path_iter = path.into_iter();
        let mut current_slot_idx = 0;
        let mut sorted_slots = slots.to_vec();
        sorted_slots.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        if let Some(start) = path_iter.next() {
            new_path.push(start.clone());
            let mut last_pt = start;

            for pt in path_iter {
                // Process segment last_pt -> pt
                loop {
                    if current_slot_idx >= sorted_slots.len() {
                        new_path.push(pt.clone());
                        last_pt = pt;
                        break;
                    }

                    let s_center = sorted_slots[current_slot_idx];
                    let s_start = s_center - slot_width / 2.0;
                    let s_end = s_center + slot_width / 2.0;

                    // If we are already past this slot, move to next slot
                    if last_pt.x >= s_end {
                        current_slot_idx += 1;
                        continue;
                    }

                    // If segment ends before this slot starts, just draw segment
                    if pt.x <= s_start {
                        new_path.push(pt.clone());
                        last_pt = pt;
                        break;
                    }

                    // Intersection with slot
                    // We are at last_pt (which is < s_end).
                    // pt is > s_start.

                    // If we are before the slot, draw to start of slot
                    if last_pt.x < s_start {
                        let t = if (pt.x - last_pt.x).abs() > 0.001 {
                            (s_start - last_pt.x) / (pt.x - last_pt.x)
                        } else {
                            0.0
                        };
                        let y_at_start = last_pt.y + t * (pt.y - last_pt.y);

                        new_path.push(Point::new(s_start, y_at_start));
                        new_path.push(Point::new(s_start, y_at_start + depth));
                        new_path.push(Point::new(s_end, y_at_start + depth));
                        new_path.push(Point::new(s_end, y_at_start));

                        last_pt = Point::new(s_end, y_at_start);
                        current_slot_idx += 1;

                        // Continue loop to check for more slots in the rest of the segment (s_end -> pt)
                    } else {
                        // We are inside the slot (last_pt.x >= s_start)
                        // Skip points until we exit
                        if pt.x <= s_end {
                            // Still inside, skip pt
                            last_pt = pt;
                            break; // Get next pt from iterator
                        } else {
                            // Exiting slot
                            // We should have drawn the slot already.
                            // Just update last_pt to s_end and continue
                            // But we need y at s_end
                            let t = if (pt.x - last_pt.x).abs() > 0.001 {
                                (s_end - last_pt.x) / (pt.x - last_pt.x)
                            } else {
                                0.0
                            };
                            let y_at_end = last_pt.y + t * (pt.y - last_pt.y);

                            last_pt = Point::new(s_end, y_at_end);
                            // We don't increment slot idx here because we might have just finished the slot we were inside?
                            // No, if we were inside, we must have entered it.
                            // If we entered it, we incremented idx.
                            // Wait, if we entered it in previous segment, we incremented idx.
                            // So current_slot_idx points to NEXT slot.
                            // So s_center is NEXT slot.
                            // So last_pt.x >= s_end check at top handles it?
                            // No, s_end is NEXT slot's end.
                            // If we are inside a slot, we need to know WHICH slot.
                            // But we only track current_slot_idx.

                            // If we are inside a slot, it means we processed the entry in a previous segment.
                            // So current_slot_idx was incremented.
                            // So `sorted_slots[current_slot_idx]` is the *next* slot.
                            // So `s_start` is the *next* slot's start.
                            // So `last_pt.x` should be < `s_start` (unless slots overlap or are very close).
                            // So we shouldn't be "inside" the *current* slot (which is the next one).

                            // So the `else` block (last_pt.x >= s_start) implies we are inside the *next* slot?
                            // Yes.
                            // This means we started inside the next slot without drawing the entry?
                            // This happens if the path starts inside a slot.
                            // In that case, we should probably just skip until we exit.

                            if pt.x <= s_end {
                                last_pt = pt;
                                break;
                            } else {
                                // Exiting the slot we started inside
                                // We can't draw the slot properly because we missed the start.
                                // Just connect to exit?
                                let t = if (pt.x - last_pt.x).abs() > 0.001 {
                                    (s_end - last_pt.x) / (pt.x - last_pt.x)
                                } else {
                                    0.0
                                };
                                let y_at_end = last_pt.y + t * (pt.y - last_pt.y);
                                last_pt = Point::new(s_end, y_at_end);
                                current_slot_idx += 1;
                                continue;
                            }
                        }
                    }
                }
            }
        }
        new_path
    }

    // Box panel generation requires all dimensional parameters.
    #[allow(clippy::too_many_arguments)]
    fn draw_divider(
        &self,
        width: f64,
        height: f64,
        edges: &str,
        start_x: f64,
        start_y: f64,
        slots: &[f64],
        slot_from_top: bool,
    ) -> Vec<Point> {
        let mut path: Vec<Point> = Vec::new();
        let edge_chars: Vec<char> = edges.chars().collect();
        let kerf = self.params.burn;
        let half_kerf = kerf / 2.0;
        let slot_depth = height / 2.0;
        let slot_width = self.t - kerf; // Slot is a hole, so subtract kerf? No, hole needs to be wider?
        // If we want a hole of size T, we cut T-kerf? No.
        // If we cut a line, the kerf makes the hole wider.
        // So if we want hole T, we draw T-kerf.
        // Yes.

        // Bottom edge (0)
        if let Some(&c) = edge_chars.first() {
            let mut base_path = if c == 'f' || c == 'F' {
                self.draw_finger_edge(width, c == 'f', slot_from_top || slots.is_empty())
            } else {
                vec![Point::new(0.0, -half_kerf), Point::new(width, -half_kerf)]
            };

            if !slot_from_top && !slots.is_empty() {
                base_path = self.apply_slots_to_path(base_path, slots, slot_depth, slot_width);
            }

            for p in &base_path {
                push_unique_point(&mut path, Point::new(start_x + p.x, start_y + p.y));
            }
        }
        push_unique_point(
            &mut path,
            Point::new(start_x + width + half_kerf, start_y - half_kerf),
        );

        // Right edge (1)
        if let Some(&c) = edge_chars.get(1) {
            let base_path = if c == 'f' || c == 'F' {
                self.draw_finger_edge(height, c == 'f', true)
            } else {
                vec![Point::new(0.0, -half_kerf), Point::new(height, -half_kerf)]
            };
            for p in &base_path {
                push_unique_point(&mut path, Point::new(start_x + width - p.y, start_y + p.x));
            }
        }
        push_unique_point(
            &mut path,
            Point::new(start_x + width + half_kerf, start_y + height + half_kerf),
        );

        // Top edge (2)
        if let Some(&c) = edge_chars.get(2) {
            let mut base_path = if c == 'F' || c == 'f' {
                self.draw_finger_edge(width, c == 'f', !slot_from_top || slots.is_empty())
            } else {
                vec![Point::new(0.0, -half_kerf), Point::new(width, -half_kerf)]
            };

            if slot_from_top && !slots.is_empty() {
                // Slots go "into" the panel.
                // For Top edge (rotated), "into" is +Y in local frame.
                base_path = self.apply_slots_to_path(base_path, slots, slot_depth, slot_width);
            }

            for p in &base_path {
                push_unique_point(
                    &mut path,
                    Point::new(start_x + width - p.x, start_y + height - p.y),
                );
            }
        }
        push_unique_point(
            &mut path,
            Point::new(start_x - half_kerf, start_y + height + half_kerf),
        );

        // Left edge (3)
        if let Some(&c) = edge_chars.get(3) {
            let base_path = if c == 'f' || c == 'F' {
                self.draw_finger_edge(height, c == 'f', true)
            } else {
                vec![Point::new(0.0, -half_kerf), Point::new(height, -half_kerf)]
            };
            for p in &base_path {
                push_unique_point(&mut path, Point::new(start_x + p.y, start_y + height - p.x));
            }
        }
        push_unique_point(
            &mut path,
            Point::new(start_x - half_kerf, start_y - half_kerf),
        );

        if let Some(first) = path.first().cloned() {
            push_unique_point(&mut path, first);
        }

        path
    }

    fn draw_divider_slots(
        &mut self,
        start_x: f64,
        start_y: f64,
        length: f64,
        thickness: f64,
        vertical: bool,
    ) {
        let settings = &self.params.finger_joint;

        let (fingers, mut leftover) = self.calc_fingers(length);

        let mut space = settings.space * self.t;
        let mut finger = settings.finger * self.t;
        let play = settings.play * self.t;
        let kerf = self.params.burn;

        // Adjust for play (we are making holes, so they should be larger)
        finger += play;
        space -= play;
        leftover -= play;

        // Hole dimension = Desired - Kerf
        let slot_w = finger - kerf;
        let slot_h = thickness - kerf;

        // Holes are drawn on the cut centreline, so inset by half the kerf; the finger pitch
        // (finger + space) must match the mating finger edge exactly.
        let mut pos = leftover / 2.0 + kerf / 2.0;

        for _ in 0..fingers {
            let (x, y, w, h) = if vertical {
                (start_x - slot_h / 2.0, start_y + pos, slot_h, slot_w)
            } else {
                (start_x + pos, start_y - slot_h / 2.0, slot_w, slot_h)
            };

            let path = vec![
                Point::new(x, y),
                Point::new(x + w, y),
                Point::new(x + w, y + h),
                Point::new(x, y + h),
                Point::new(x, y),
            ];

            self.add_path(path);

            pos += finger + space;
        }
    }

    fn add_divider_slots(
        &mut self,
        count: u32,
        total_length: f64,
        start_x: f64,
        start_y: f64,
        slot_depth: f64,
        vertical: bool,
    ) {
        if count == 0 {
            return;
        }

        let spacing = total_length / (count as f64 + 1.0);
        for idx in 1..=count {
            let offset = spacing * idx as f64;
            if vertical {
                self.draw_divider_slots(start_x + offset, start_y, slot_depth, self.t, true);
            } else {
                self.draw_divider_slots(start_x, start_y + offset, slot_depth, self.t, false);
            }
        }
    }

    /// All generated cut paths (panel outlines and slot holes) in generation order.
    pub fn paths(&self) -> &Vec<Vec<Point>> {
        &self.paths
    }

    /// Label of each path returned by [`Self::paths`], e.g. `"Wall 3 (back)"` or
    /// `"Bottom slot 2"`.
    ///
    /// Labels are attached when a path is created, so they stay correct when panels are
    /// omitted by the box type or slot holes are interleaved with the outlines.
    pub fn path_labels(&self) -> &[String] {
        &self.path_labels
    }

    fn start_new_group(&mut self, name: &str) {
        self.path_groups.push(Vec::new());
        self.current_group_name = name.to_string();
    }

    fn add_path(&mut self, path: Vec<Point>) {
        let idx = self.paths.len();
        self.paths.push(path);
        if let Some(group) = self.path_groups.last_mut() {
            // The first path of a group is the panel outline; later ones are its slot holes.
            let label = if group.is_empty() {
                self.current_group_name.clone()
            } else {
                format!("{} slot {}", self.current_group_name, group.len())
            };
            self.path_labels.push(label);
            group.push(idx);
        } else {
            // If no group exists, create one (fallback)
            self.path_labels.push(format!("Path {}", idx + 1));
            self.path_groups.push(vec![idx]);
        }
    }

    pub fn generate(&mut self) -> Result<(), String> {
        self.paths.clear();
        self.path_labels.clear();
        self.path_groups.clear();

        let x = self.x;
        let y = self.y;
        let h = self.h;
        let _t = self.t;

        // Every panel is drawn at the origin; `layout_rows` (or `pack_paths`) places it.
        let mut layout = LayoutRows::default();

        let (has_top, has_bottom, has_front, has_back, has_left, has_right) =
            match self.params.box_type {
                BoxType::FullBox => (true, true, true, true, true, true),
                BoxType::NoTop => (false, true, true, true, true, true),
                BoxType::NoBottom => (true, false, true, true, true, true),
                BoxType::NoSides | BoxType::NoLeftRight => (true, true, true, true, false, false),
                BoxType::NoFrontBack => (true, true, false, false, true, true),
            };

        // Helper to format edge string
        let edges = |b, r, t, l| {
            let c = |cond, ch| if cond { ch } else { 'e' };
            format!("{}{}{}{}", c(b, 'F'), c(r, 'F'), c(t, 'F'), c(l, 'F'))
        };

        // Wall 2/4 edges need 'f' for side connections
        let edges_side = |b, r, t, l| {
            let c = |cond, ch| if cond { ch } else { 'e' };
            format!("{}{}{}{}", c(b, 'F'), c(r, 'f'), c(t, 'F'), c(l, 'f'))
        };

        // Top/Bottom edges need 'f' for all connections
        let edges_tb = |b, r, t, l| {
            let c = |cond, ch| if cond { ch } else { 'e' };
            format!("{}{}{}{}", c(b, 'f'), c(r, 'f'), c(t, 'f'), c(l, 'f'))
        };

        let key_walls = self.params.key_divider_type == KeyDividerType::WallsAndFloor
            || self.params.key_divider_type == KeyDividerType::WallsOnly;
        let key_floor = self.params.key_divider_type == KeyDividerType::WallsAndFloor
            || self.params.key_divider_type == KeyDividerType::FloorOnly;

        // Wall 1: Front (x by h)
        if has_front {
            self.start_new_group("Wall 1 (front)");
            let e = edges(has_bottom, has_right, has_top, has_left);
            layout.place(self.path_groups.len() - 1);
            let (start_x, start_y) = (0.0, 0.0);
            self.add_path(self.draw_rectangular_wall(x, h, &e, start_x, start_y));

            if key_walls {
                self.add_divider_slots(self.params.dividers_x, x, start_x, start_y, h, true);
            }
        }

        // Wall 2: Right (y by h)
        if has_right {
            self.start_new_group("Wall 2 (right)");
            let e = edges_side(has_bottom, has_back, has_top, has_front);
            layout.place(self.path_groups.len() - 1);
            let (start_x, start_y) = (0.0, 0.0);
            self.add_path(self.draw_rectangular_wall(y, h, &e, start_x, start_y));

            if key_walls {
                self.add_divider_slots(self.params.dividers_y, y, start_x, start_y, h, true);
            }

            layout.next_row();
        }

        // Wall 4: Left (y by h)
        if has_left {
            self.start_new_group("Wall 4 (left)");
            let e = edges_side(has_bottom, has_front, has_top, has_back);
            layout.place(self.path_groups.len() - 1);
            let (start_x, start_y) = (0.0, 0.0);
            self.add_path(self.draw_rectangular_wall(y, h, &e, start_x, start_y));

            if key_walls {
                self.add_divider_slots(self.params.dividers_y, y, start_x, start_y, h, true);
            }
        }

        // Wall 3: Back (x by h)
        if has_back {
            self.start_new_group("Wall 3 (back)");
            let e = edges(has_bottom, has_left, has_top, has_right);
            layout.place(self.path_groups.len() - 1);
            let (start_x, start_y) = (0.0, 0.0);
            self.add_path(self.draw_rectangular_wall(x, h, &e, start_x, start_y));

            if key_walls {
                self.add_divider_slots(self.params.dividers_x, x, start_x, start_y, h, true);
            }
        }

        // Top: x by y
        if has_top {
            self.start_new_group("Top");
            let e = edges_tb(has_front, has_right, has_back, has_left);
            layout.place(self.path_groups.len() - 1);
            let (start_x, start_y) = (0.0, 0.0);
            self.add_path(self.draw_rectangular_wall(x, y, &e, start_x, start_y));
        }

        // Bottom: x by y
        if has_bottom {
            self.start_new_group("Bottom");
            let e = edges_tb(has_front, has_right, has_back, has_left);
            layout.place(self.path_groups.len() - 1);
            let (start_x, start_y) = (0.0, 0.0);
            self.add_path(self.draw_rectangular_wall(x, y, &e, start_x, start_y));

            if key_floor {
                self.add_divider_slots(self.params.dividers_x, x, start_x, start_y, y, true);
                self.add_divider_slots(self.params.dividers_y, y, start_x, start_y, x, false);
            }
        }

        // Dividers
        let div_edges_x = match self.params.key_divider_type {
            KeyDividerType::WallsAndFloor => "ffef", // Bottom=f(Tab), Right=f(Tab), Top=e(Plain), Left=f(Tab)
            KeyDividerType::WallsOnly => "efef", // Bottom=e(Plain), Right=f(Tab), Top=e(Plain), Left=f(Tab)
            KeyDividerType::FloorOnly => "feee", // Bottom=f(Tab), Right=e(Plain), Top=e(Plain), Left=e(Plain)
            KeyDividerType::None => "eeee",
        };

        // Calculate slot positions
        let mut slots_x = Vec::new(); // For Y-dividers (spanning X)
        if self.params.dividers_x > 0 {
            let spacing_x = x / (self.params.dividers_x as f64 + 1.0);
            for i in 1..=self.params.dividers_x {
                slots_x.push(i as f64 * spacing_x);
            }
        }

        let mut slots_y = Vec::new(); // For X-dividers (spanning Y)
        if self.params.dividers_y > 0 {
            let spacing_y = y / (self.params.dividers_y as f64 + 1.0);
            for i in 1..=self.params.dividers_y {
                slots_y.push(i as f64 * spacing_y);
            }
        }

        if self.params.dividers_x > 0 {
            for n in 1..=self.params.dividers_x {
                // X-divider (Width=Y). Slots at Y-positions (slots_y).
                // Slots from Top (Plain edge).
                self.start_new_group(&format!("X divider {}", n));
                layout.place(self.path_groups.len() - 1);
                let (start_x, start_y) = (0.0, 0.0);
                self.add_path(self.draw_divider(
                    y,
                    h,
                    div_edges_x,
                    start_x,
                    start_y,
                    &slots_y,
                    true,
                ));
            }
        }

        if self.params.dividers_y > 0 {
            for n in 1..=self.params.dividers_y {
                // Y-divider (Width=X). Slots at X-positions (slots_x).
                // Slots from Bottom (Connecting edge).
                self.start_new_group(&format!("Y divider {}", n));
                layout.place(self.path_groups.len() - 1);
                let (start_x, start_y) = (0.0, 0.0);
                self.add_path(self.draw_divider(
                    x,
                    h,
                    div_edges_x,
                    start_x,
                    start_y,
                    &slots_x,
                    false,
                ));
            }
        }

        if self.params.optimize_layout {
            self.pack_paths();
        } else {
            self.layout_rows(&layout);
        }

        // Normalize so (offset_x, offset_y) is always the minimum XY.
        // Without this, finger joints can protrude into negative coordinates even with 0 offsets.
        // Note: guard against accidental NaNs preventing normalization.
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        for path in &self.paths {
            for p in path {
                if p.x.is_finite() {
                    min_x = min_x.min(p.x);
                }
                if p.y.is_finite() {
                    min_y = min_y.min(p.y);
                }
            }
        }
        if min_x.is_finite() && min_y.is_finite() {
            for path in &mut self.paths {
                for p in path {
                    if p.x.is_finite() {
                        p.x -= min_x;
                    }
                    if p.y.is_finite() {
                        p.y -= min_y;
                    }
                }
            }
        }

        // Apply global offset to all paths
        if self.params.offset_x != 0.0 || self.params.offset_y != 0.0 {
            for path in &mut self.paths {
                for point in path {
                    point.x += self.params.offset_x;
                    point.y += self.params.offset_y;
                }
            }
        }

        Ok(())
    }

    pub fn to_gcode(&self) -> String {
        let mut gcode = String::new();

        gcode.push_str("; Tabbed Box Maker G-code\n");
        gcode.push_str("; Based on https://github.com/florianfesti/boxes\n");
        gcode.push_str(";\n");
        gcode.push_str("; --- Box Dimensions ---\n");
        gcode.push_str(&format!(
            "; Dimensions: {}x{}x{} mm\n",
            self.params.x, self.params.y, self.params.h
        ));
        gcode.push_str(&format!("; Outside Dimensions: {}\n", self.params.outside));
        gcode.push_str(&format!("; Box Type: {:?}\n", self.params.box_type));
        gcode.push_str(&format!("; Dividers X: {}\n", self.params.dividers_x));
        gcode.push_str(&format!("; Dividers Y: {}\n", self.params.dividers_y));
        gcode.push_str(&format!(
            "; Divider Keying: {:?}\n",
            self.params.key_divider_type
        ));
        gcode.push_str(&format!(
            "; Optimize Layout: {}\n",
            self.params.optimize_layout
        ));
        gcode.push_str(";\n");

        gcode.push_str("; --- Material Settings ---\n");
        gcode.push_str(&format!(
            "; Material thickness: {} mm\n",
            self.params.thickness
        ));
        gcode.push_str(&format!("; Burn / Tool Dia: {} mm\n", self.params.burn));
        gcode.push_str(";\n");

        gcode.push_str("; --- Finger Joint Settings ---\n");
        gcode.push_str(&format!(
            "; Finger width: {} * thickness = {} mm\n",
            self.params.finger_joint.finger,
            self.params.finger_joint.finger * self.params.thickness
        ));
        gcode.push_str(&format!(
            "; Space width: {} * thickness = {} mm\n",
            self.params.finger_joint.space,
            self.params.finger_joint.space * self.params.thickness
        ));
        gcode.push_str(&format!(
            "; Surrounding spaces: {}\n",
            self.params.finger_joint.surrounding_spaces
        ));
        gcode.push_str(&format!(
            "; Play: {} mm\n",
            self.params.finger_joint.play * self.params.thickness
        ));
        gcode.push_str(&format!(
            "; Extra length: {} mm\n",
            self.params.finger_joint.extra_length * self.params.thickness
        ));
        gcode.push_str(&format!(
            "; Finger Style: {:?}\n",
            self.params.finger_joint.style
        ));
        gcode.push_str(";\n");

        gcode.push_str("; --- Laser Settings ---\n");
        gcode.push_str(&format!("; Laser passes: {}\n", self.params.laser_passes));
        gcode.push_str(&format!("; Laser power: S{}\n", self.params.laser_power));
        gcode.push_str(&format!(
            "; Feed rate: {:.0} mm/min\n",
            self.params.feed_rate
        ));
        gcode.push_str(";\n");

        gcode.push_str("; --- Work Origin Offsets ---\n");
        gcode.push_str(&format!("; Offset X: {} mm\n", self.params.offset_x));
        gcode.push_str(&format!("; Offset Y: {} mm\n", self.params.offset_y));
        gcode.push_str(";\n");

        gcode.push_str("; Initialization\n");
        crate::job_preamble::push_modal_header(&mut gcode, true);
        gcode.push('\n');

        crate::job_preamble::push_work_setup(&mut gcode, self.params.home_before_job);
        gcode.push('\n');

        // Ensure emitted coordinates honour user offsets (offset_x/offset_y represent desired minimum XY).
        // This also guards against any upstream layout producing negative min bounds.
        let (shift_x, shift_y) = {
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            for path in &self.paths {
                for p in path {
                    if p.x.is_finite() {
                        min_x = min_x.min(p.x);
                    }
                    if p.y.is_finite() {
                        min_y = min_y.min(p.y);
                    }
                }
            }
            if min_x.is_finite() && min_y.is_finite() {
                (self.params.offset_x - min_x, self.params.offset_y - min_y)
            } else {
                (0.0, 0.0)
            }
        };

        let has_z = self.params.num_axes >= 3;

        // Cut each panel's slot holes before its outline so the panel is still held by the
        // surrounding stock while the holes are cut.
        let mut order: Vec<usize> = Vec::with_capacity(self.paths.len());
        let mut seen = vec![false; self.paths.len()];
        for group in &self.path_groups {
            for &idx in group.iter().skip(1).chain(group.iter().take(1)) {
                if idx < self.paths.len() && !seen[idx] {
                    seen[idx] = true;
                    order.push(idx);
                }
            }
        }
        order.extend((0..self.paths.len()).filter(|&i| !seen[i]));

        for (n, &i) in order.iter().enumerate() {
            let path = &self.paths[i];
            let label = self
                .path_labels
                .get(i)
                .map(String::as_str)
                .unwrap_or("Unknown");
            gcode.push_str(&format!("; Path {}: {}\n", n + 1, label));

            if let Some(first_point) = path.first() {
                gcode.push_str(&format!(
                    "G0 X{:.2} Y{:.2} ; Rapid to start\n",
                    first_point.x + shift_x,
                    first_point.y + shift_y
                ));

                for pass_num in 1..=self.params.laser_passes {
                    let z_depth = -(pass_num as f64 - 1.0) * self.params.z_step_down;

                    // Every pass sets its own height (pass 1 is the work Z0 focus height),
                    // so no panel inherits the previous panel's deepest pass.
                    if has_z {
                        gcode.push_str(&format!("G0 Z{:.2} ; Move to pass height\n", z_depth));
                    }
                    gcode.push_str(&format!(
                        "; Pass {}/{} at Z{:.2}\n",
                        pass_num, self.params.laser_passes, z_depth
                    ));

                    // M4 (dynamic power) scales power with speed so corners do not over-burn.
                    gcode.push_str(&format!(
                        "M4 S{} ; Laser on (dynamic power)\n",
                        self.params.laser_power
                    ));

                    for (idx, point) in path.iter().skip(1).enumerate() {
                        if idx == 0 {
                            gcode.push_str(&format!(
                                "G1 X{:.2} Y{:.2} F{:.0}\n",
                                point.x + shift_x,
                                point.y + shift_y,
                                self.params.feed_rate
                            ));
                        } else {
                            gcode.push_str(&format!(
                                "G1 X{:.2} Y{:.2}\n",
                                point.x + shift_x,
                                point.y + shift_y
                            ));
                        }
                    }

                    gcode.push_str("M5 ; Laser off\n");

                    if pass_num < self.params.laser_passes {
                        gcode.push_str(&format!(
                            "G0 X{:.2} Y{:.2} ; Return to start\n",
                            first_point.x + shift_x,
                            first_point.y + shift_y
                        ));
                    }
                }
            }

            gcode.push('\n');
        }

        gcode.push_str("M5 ; Ensure laser off\n");
        if has_z {
            if self.params.home_before_job {
                // Work Z0 is not machine Z0 (the Z work offset is kept), so the final retract
                // after homing is made in machine coordinates, never above the homed position.
                gcode.push_str(&crate::job_preamble::travel_z_line(true, 0.0, "Retract"));
            } else {
                // Back to the first-pass (focus) height in the user's work coordinates.
                gcode.push_str("G0 Z0.00 ; Return to first-pass height\n");
            }
        }
        gcode.push_str("G0 X0 Y0 ; Return to origin\n");
        gcode.push_str("M2 ; Program end\n");

        gcode
    }
}
