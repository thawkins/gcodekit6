//! CAM dispatch: the effective CAM parameters of a shape and the toolpaths made from them.
//!
//! This is the one owner of the shape-to-toolpath rules. G-code export
//! (`designer_state::gcode`) and the egui canvas toolpath preview both build their
//! toolpaths through [`CamSettings::shape_toolpaths`], so the preview cannot drift from
//! the G-code it previews.
//!
//! [`CamSettings`] is a snapshot of the design-wide settings the rules read (machine mode,
//! tool settings, stock top, default CAM properties), so a worker thread can build
//! toolpaths without borrowing the [`DesignerState`].
//!
//! Dependencies: the toolpath generator and the shape model of this crate.

use super::{DesignerState, MachineMode, ToolSettings};
use crate::canvas::DrawingObject;
use crate::model::{DesignPath, DesignerShape, LaserParams, Shape};
use crate::shapes::OperationType;
use crate::toolpath::{Toolpath, ToolpathGenerator};

/// The toolpaths of one shape, as built by [`CamSettings::shape_toolpaths`].
#[derive(Clone, Debug)]
pub struct ShapeToolpaths {
    /// The shape with the CAM parameters that were applied
    /// (see [`CamSettings::effective_object`]).
    pub object: DrawingObject,
    /// The toolpaths, in cutting order.
    pub toolpaths: Vec<Toolpath>,
    /// Whether a text pocket cut nothing and fell back to a profile of the glyphs.
    pub pocket_fallback_to_profile: bool,
}

/// The design-wide settings that decide each shape's effective CAM parameters.
#[derive(Clone, Debug)]
pub struct CamSettings {
    tool_settings: ToolSettings,
    /// Stock top Z (mm); 10 mm when no stock is configured.
    stock_top_z: f64,
    /// Default CAM properties for shapes that do not use custom values.
    defaults: DrawingObject,
}

impl CamSettings {
    /// The settings of `state`.
    ///
    /// # Arguments
    /// * `state` - The design whose tool settings, stock and default properties apply.
    pub fn of(state: &DesignerState) -> Self {
        Self {
            tool_settings: state.tool_settings.clone(),
            stock_top_z: state
                .stock_material
                .as_ref()
                .map(|s| s.thickness)
                .unwrap_or(10.0),
            defaults: state.default_properties_shape.clone(),
        }
    }

    fn is_cnc(&self) -> bool {
        self.tool_settings.machine_mode == MachineMode::Cnc3D
    }

    /// The shape with the CAM parameters G-code generation uses.
    ///
    /// In CNC mode a shape without custom values takes the default CAM properties and
    /// the global step down, and global laser parameters follow the tool settings; a
    /// non-positive step down becomes 0.1 mm and a negative pocket depth 0. In laser mode
    /// the shape is used as it is.
    ///
    /// # Arguments
    /// * `shape` - The shape as drawn.
    pub fn effective_object(&self, shape: &DrawingObject) -> DrawingObject {
        let mut effective = shape.clone();
        if !self.is_cnc() {
            return effective;
        }
        let tool = &self.tool_settings;
        if !shape.use_custom_values {
            effective.operation_type = self.defaults.operation_type;
            effective.step_in = self.defaults.step_in;
            effective.ramp_angle = self.defaults.ramp_angle;
            effective.pocket_strategy = self.defaults.pocket_strategy;
            effective.raster_fill_ratio = self.defaults.raster_fill_ratio;

            effective.step_down = if tool.step_down > 0.0 {
                tool.step_down as f32
            } else {
                self.defaults.step_down
            };

            effective.laser_params.use_global = true;
            effective.laser_params.feed_rate = tool.feed_rate;
            effective.laser_params.power_percent = (tool.spindle_speed as f64) / 10.0;
        } else if effective.laser_params.use_global {
            effective.laser_params.feed_rate = tool.feed_rate;
            effective.laser_params.power_percent = (tool.spindle_speed as f64) / 10.0;
        }

        // CNC multipass must be driven by object CAM properties. Keep only a
        // technical fallback for legacy objects with invalid/empty step-down.
        if effective.step_down <= 0.0 {
            effective.step_down = 0.1;
        }
        if effective.pocket_depth < 0.0 {
            effective.pocket_depth = 0.0;
        }
        effective
    }

    /// The laser parameters a shape is cut with: the global ones (feed and power from
    /// the tool settings) when the shape or its embedded geometry asks for them,
    /// otherwise the shape's own.
    ///
    /// # Arguments
    /// * `shape` - The shape (usually already [`Self::effective_object`]).
    pub fn effective_laser_params(&self, shape: &DrawingObject) -> LaserParams {
        effective_laser_params(&self.tool_settings, shape)
    }

    /// `(start_depth, cut_depth)` for the toolpath generator.
    ///
    /// In CNC mode machine Z is absolute (table at 0, stock top at stock thickness):
    /// passes start at the stock top and descend to the object's Z. In laser mode the
    /// object's start depth applies and there is no cut depth.
    fn depths(&self, effective: &DrawingObject) -> (f64, f64) {
        if self.is_cnc() {
            let start_z = self.stock_top_z - effective.start_depth;
            (self.stock_top_z, (self.stock_top_z - start_z).max(0.0))
        } else {
            (effective.start_depth, 0.0)
        }
    }

    /// Builds the toolpaths of one shape with its effective CAM parameters.
    ///
    /// Every generator setting a toolpath depends on is set here, per shape, so the
    /// result does not depend on what `generator` built before.
    ///
    /// # Arguments
    /// * `generator` - The generator to configure and run.
    /// * `shape` - The shape as drawn.
    pub fn shape_toolpaths(
        &self,
        generator: &mut ToolpathGenerator,
        shape: &DrawingObject,
    ) -> ShapeToolpaths {
        let object = self.effective_object(shape);
        let laser = !self.is_cnc();
        let laser_params = self.effective_laser_params(&object);

        if laser {
            generator.set_feed_rate(laser_params.feed_rate);
            generator.set_spindle_speed((laser_params.power_percent * 10.0) as u32);
        } else {
            // CNC: shapes using global values read feed/speed from the generator.
            generator.set_feed_rate(self.tool_settings.feed_rate);
            generator.set_spindle_speed(self.tool_settings.spindle_speed);
        }

        let (start_depth, cut_depth) = self.depths(&object);
        generator.set_tool_diameter(self.tool_settings.tool_diameter);
        generator.set_pocket_strategy(object.pocket_strategy);
        generator.set_start_depth(start_depth);
        generator.set_cut_depth(cut_depth);
        generator.set_step_in(object.step_in as f64);
        generator.set_ramp_angle(object.ramp_angle);
        generator.set_raster_fill_ratio(object.raster_fill_ratio);

        let mut effective_shape = object.get_effective_shape();
        // Keep shape-embedded laser params in sync with the effective per-object/global
        // choice so 2D toolpath generation cannot drift.
        if laser {
            apply_laser_params_to_shape(&mut effective_shape, laser_params);
        }

        let (toolpaths, pocket_fallback_to_profile) =
            dispatch(generator, &object, &effective_shape);
        ShapeToolpaths {
            object,
            toolpaths,
            pocket_fallback_to_profile,
        }
    }
}

/// Runs the generator call for `shape`: a pocket or a contour by the object's operation.
///
/// # Returns
/// The toolpaths and whether a text pocket fell back to a profile cut.
fn dispatch(
    generator: &mut ToolpathGenerator,
    object: &DrawingObject,
    shape: &Shape,
) -> (Vec<Toolpath>, bool) {
    let is_pocket = object.operation_type == OperationType::Pocket;
    let step_down = object.step_down as f64;
    let step_in = object.step_in as f64;
    let depth = object.effective_pocket_depth();
    let toolpaths = match shape {
        Shape::Rectangle(r) if is_pocket => {
            generator.generate_rectangle_pocket(r, depth, step_down, step_in)
        }
        Shape::Rectangle(r) => generator.generate_rectangle_contour(r, step_down),
        Shape::Circle(c) if is_pocket => {
            generator.generate_circle_pocket(c, depth, step_down, step_in)
        }
        Shape::Circle(c) => generator.generate_circle_contour(c, step_down),
        Shape::Line(l) => generator.generate_line_contour(l, step_down),
        Shape::Ellipse(e) if is_pocket => {
            generator.generate_ellipse_pocket(e, depth, step_down, step_in)
        }
        Shape::Ellipse(e) => generator.generate_ellipse_contour(e, step_down),
        // Path rotation is a render-time rotation about the path bounds centre; the
        // generator applies it for both contour and pocket.
        Shape::Path(p) if is_pocket => generator.generate_path_pocket(p, depth, step_down, step_in),
        Shape::Path(p) => generator.generate_path_contour(p, step_down),
        Shape::Text(text) if is_pocket => {
            let pocket = generator.generate_text_pocket_toolpath(text, step_down);
            let length: f64 = pocket.iter().map(Toolpath::total_length).sum();
            if length <= 1e-9 {
                // Profile the glyphs when the pocket cuts nothing.
                return (generator.generate_text_toolpath(text, step_down), true);
            }
            pocket
        }
        Shape::Text(text) => generator.generate_text_toolpath(text, step_down),
        Shape::Triangle(t) if is_pocket => {
            generator.generate_triangle_pocket(t, depth, step_down, step_in)
        }
        Shape::Triangle(t) => generator.generate_triangle_contour(t, step_down),
        Shape::Polygon(p) if is_pocket => {
            generator.generate_polygon_pocket(p, depth, step_down, step_in)
        }
        Shape::Polygon(p) => generator.generate_polygon_contour(p, step_down),
        // Raster images are engraved line by line, not as a vector toolpath.
        Shape::RasterImage(_) => Vec::new(),
        // Gears and sprockets go through their rendered outline.
        Shape::Gear(_) | Shape::Sprocket(_) => {
            let path = DesignPath::from_lyon_path(&shape.render());
            if is_pocket {
                generator.generate_path_pocket(&path, depth, step_down, step_in)
            } else {
                generator.generate_path_contour(&path, step_down)
            }
        }
    };
    (toolpaths, false)
}

/// See [`CamSettings::effective_laser_params`].
pub(super) fn effective_laser_params(
    tool_settings: &ToolSettings,
    shape: &DrawingObject,
) -> LaserParams {
    let use_global = shape.use_global_laser
        || shape.laser_params.use_global
        || shape_embedded_laser_use_global(shape);

    if use_global {
        let passes = if tool_settings.machine_mode == MachineMode::Laser2D {
            // Laser mode: step_down IS the number of passes (1-10).
            tool_settings.step_down.round().clamp(1.0, 10.0) as u32
        } else {
            // CNC mode: convert depth to a number of passes.
            (tool_settings.step_down / 0.1).max(1.0).round() as u32
        };

        LaserParams {
            feed_rate: tool_settings.feed_rate,
            power_percent: (tool_settings.spindle_speed as f64) / 10.0,
            passes,
            use_global: true,
        }
    } else {
        let mut params = shape.laser_params;
        params.use_global = false;
        params
    }
}

fn shape_embedded_laser_use_global(shape: &DrawingObject) -> bool {
    match &shape.shape {
        Shape::Rectangle(s) => s.laser_params.use_global,
        Shape::Circle(s) => s.laser_params.use_global,
        Shape::Line(s) => s.laser_params.use_global,
        Shape::Ellipse(s) => s.laser_params.use_global,
        Shape::Path(s) => s.laser_params.use_global,
        Shape::Text(s) => s.laser_params.use_global,
        Shape::Triangle(s) => s.laser_params.use_global,
        Shape::Polygon(s) => s.laser_params.use_global,
        Shape::Gear(s) => s.laser_params.use_global,
        Shape::Sprocket(s) => s.laser_params.use_global,
        Shape::RasterImage(_) => shape.laser_params.use_global,
    }
}

fn apply_laser_params_to_shape(shape: &mut Shape, params: LaserParams) {
    match shape {
        Shape::Rectangle(s) => s.laser_params = params,
        Shape::Circle(s) => s.laser_params = params,
        Shape::Line(s) => s.laser_params = params,
        Shape::Ellipse(s) => s.laser_params = params,
        Shape::Path(s) => s.laser_params = params,
        Shape::Text(s) => s.laser_params = params,
        Shape::Triangle(s) => s.laser_params = params,
        Shape::Polygon(s) => s.laser_params = params,
        Shape::Gear(s) => s.laser_params = params,
        Shape::Sprocket(s) => s.laser_params = params,
        Shape::RasterImage(_) => {}
    }
}
