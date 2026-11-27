//! G-code generation for designer state

use super::DesignerState;
use super::cam::{self, CamSettings};
use crate::ToolpathToGcode;
use crate::canvas::DrawingObject;
use crate::designer_state::MachineMode;
use crate::gcode_gen::MachineLimits;
use crate::model::DesignerShape;
use crate::model::LaserParams;
use crate::shapes::OperationType;
use gcodekit5_core::Units;

/// Comment block added when a toolpath leaves the machine's working area.
const LIMIT_WARNING: &str = ";
; *********************************************
; ********** WARNING: OUT OF LIMITS ***********
; ** This toolpath contains coordinates      **
; ** outside the machine working area.       **
; ** Risk of collision if no limit switches! **
; *********************************************
;
";

impl DesignerState {
    /// Computes effective start depth for CNC from UI Z.
    ///
    /// In CNC mode, UI Z is treated as depth from stock top.
    /// Machine Z is absolute with table at 0 and stock top at stock_thickness,
    /// therefore: machine_z = stock_thickness - ui_depth.
    fn effective_cnc_start_depth_from_shape(&self, shape: &DrawingObject) -> f64 {
        let stock_thickness = self
            .stock_material
            .as_ref()
            .map(|s| s.thickness as f64)
            .unwrap_or(10.0);
        stock_thickness - shape.start_depth
    }

    /// Returns a safety violation summary when any object starts at/above safe Z in CNC mode.
    ///
    /// # Arguments
    /// * `is_exported` - Whether a shape takes part in the export (e.g. not hidden in the
    ///   view); shapes it rejects are not checked.
    ///
    /// # Returns
    /// `(safe_z, violating_count, max_object_start_z)`, or `None` when no exported shape
    /// starts at or above Safe Z (always `None` outside CNC mode).
    pub fn safe_z_clearance_violation_summary(
        &self,
        is_exported: impl Fn(&DrawingObject) -> bool,
    ) -> Option<(f64, usize, f64)> {
        if self.machine_mode() != MachineMode::Cnc3D {
            return None;
        }

        let safe_z = self
            .stock_material
            .as_ref()
            .map(|s| s.safe_z as f64)
            .unwrap_or(10.0);

        let mut violating_count = 0usize;
        let mut max_start_z = f64::NEG_INFINITY;

        for shape in self.canvas.shape_store.iter().filter(|o| is_exported(o)) {
            let start_z = self.effective_cnc_start_depth_from_shape(shape);
            if start_z >= safe_z {
                violating_count += 1;
                if start_z > max_start_z {
                    max_start_z = start_z;
                }
            }
        }

        if violating_count > 0 {
            Some((safe_z, violating_count, max_start_z))
        } else {
            None
        }
    }

    /// The effective laser parameters of `shape` (see [`CamSettings::effective_laser_params`]).
    fn get_effective_laser_params(&self, shape: &DrawingObject) -> Option<LaserParams> {
        Some(cam::effective_laser_params(&self.tool_settings, shape))
    }

    /// Phase 1 of G-code generation: the G-code writer set up for the
    /// stock (safe Z, Z floor), the machine limits and the machine mode.
    fn configured_gcode_generator(&self, machine_limits: Option<MachineLimits>) -> ToolpathToGcode {
        // Get safe_z from stock_material, default to 10.0 if not set
        let safe_z = self
            .stock_material
            .as_ref()
            .map(|s| s.safe_z as f64)
            .unwrap_or(10.0);
        let mut gcode_gen = ToolpathToGcode::new(Units::MM, safe_z)
            .with_continuous_z_between_passes(self.tool_settings.continuous_z_between_passes);

        if let Some(limits) = machine_limits {
            gcode_gen = gcode_gen.with_machine_limits(limits);
        }

        // In CNC mode machine Z is absolute with the table at 0 and the stock top
        // at the stock thickness, so a configured stock bounds Z from below at 0.
        if self.machine_mode() == MachineMode::Cnc3D && self.stock_material.is_some() {
            gcode_gen = gcode_gen.with_z_min_limit(0.0);
        }

        // Use state mode:
        match self.machine_mode() {
            MachineMode::Laser2D => {
                gcode_gen = gcode_gen.with_laser_2d();
            }
            MachineMode::Cnc3D => {}
        }

        gcode_gen.num_axes = self.num_axes;
        if self.num_axes < 3 {
            // Use state mode:
            match self.machine_mode() {
                MachineMode::Laser2D => {
                    gcode_gen = gcode_gen
                        .with_laser_2d()
                        .with_curve_tolerance(0.05) // Adjust as needed (0.03 ~ 0.15)
                        .with_min_point_distance(0.15);
                }
                MachineMode::Cnc3D => {}
            }
        }
        gcode_gen
    }

    /// Phase 2 of G-code generation: the toolpaths of every shape to cut
    /// (the selection, or all shapes), in machining order, with whether a
    /// pocket fell back to a profile cut.
    fn build_shape_toolpaths(&mut self) -> Vec<(DrawingObject, Vec<crate::Toolpath>, bool)> {
        // Store shape-to-toolpath mapping
        let mut shape_toolpaths: Vec<(DrawingObject, Vec<crate::Toolpath>, bool)> = Vec::new();

        // Obtener el ID del objeto seleccionado (si hay)
        let selected_ids = self
            .canvas
            .selection_manager
            .selected_ids(&self.canvas.shape_store);

        // Collect shape IDs in reverse draw order (front to back) for G-code generation
        let shape_ids: Vec<u64> = self.canvas.shape_store.draw_order_iter().collect();
        let cam = CamSettings::of(self);

        for shape_id in shape_ids {
            let Some(shape_obj) = self.canvas.shape_store.get(shape_id) else {
                continue;
            };

            if !selected_ids.is_empty() && !selected_ids.contains(&shape_obj.id) {
                continue; // Saltar si no está en la selección
            }

            let built = cam.shape_toolpaths(&mut self.toolpath_generator, shape_obj);
            shape_toolpaths.push((
                built.object,
                built.toolpaths,
                built.pocket_fallback_to_profile,
            ));
        }

        shape_toolpaths
    }

    /// Phase 3 of G-code generation: appends each shape's comment header
    /// and passes to `gcode`, numbering lines from 10 in steps of 10.
    /// `header_speed` is the CNC spindle speed the header started.
    fn emit_shapes_gcode(
        &mut self,
        gcode: &mut String,
        gcode_gen: &ToolpathToGcode,
        shape_toolpaths: &[(DrawingObject, Vec<crate::Toolpath>, bool)],
        header_speed: u32,
    ) {
        let mut line_number = 10;
        // The header starts the CNC spindle at `header_speed`; a later shape
        // that asks for another speed gets an `S` word before it cuts.
        let mut spindle_speed = header_speed;

        // ------ Loop Shape --------
        for (shape, toolpaths, pocket_fallback_to_profile) in shape_toolpaths.iter() {
            // ============================================================
            // Obtener parámetros efectivos para G-code
            // ============================================================
            let effective_params = self.get_effective_laser_params(shape);

            // Aplicar override si estamos en modo láser
            if gcode_gen.is_laser_2d {
                self.apply_laser_generator_params(effective_params.as_ref());
            }

            if let crate::model::Shape::RasterImage(raster) = &shape.shape {
                Self::emit_raster_gcode(gcode, shape, raster);
                continue; // Saltar el procesamiento normal para este shape
            }

            self.emit_shape_comments(gcode, gcode_gen, shape, *pocket_fallback_to_profile);

            // Generate G-code for all toolpaths associated with this shape
            let num_passes = if gcode_gen.is_laser_2d {
                if let Some(params) = &effective_params {
                    params.passes.max(1) as usize
                } else {
                    self.tool_settings.step_down.max(1.0) as usize
                }
            } else {
                // CNC MODE: toolpaths already include multipass depth logic.
                1
            };

            Self::emit_shape_passes(
                gcode,
                gcode_gen,
                toolpaths,
                num_passes,
                &mut line_number,
                &mut spindle_speed,
            );
        } // for (shape, toolpaths, pocket_fallback_to_profile)
    }

    /// Shape phase (laser mode): points the toolpath generator's feed and
    /// power at the shape's effective laser parameters, or the global tool
    /// settings when it has none.
    fn apply_laser_generator_params(&mut self, effective_params: Option<&LaserParams>) {
        if let Some(params) = effective_params {
            self.toolpath_generator.set_feed_rate(params.feed_rate);
            self.toolpath_generator
                .set_spindle_speed((params.power_percent * 10.0) as u32);
        } else {
            self.toolpath_generator
                .set_feed_rate(self.tool_settings.feed_rate);
            self.toolpath_generator
                .set_spindle_speed(self.tool_settings.spindle_speed);
        }
    }

    /// Shape phase for a raster image: its comment header and the engraver's
    /// G-code. Problems are logged and the image is skipped.
    fn emit_raster_gcode(
        gcode: &mut String,
        shape: &DrawingObject,
        raster: &crate::model::RasterImage,
    ) {
        // Generar G-code para raster
        use crate::engraving::ImageEngraver;

        if raster.image_data.is_empty() && raster.original_path.is_none() {
            tracing::warn!(
                shape_id = shape.id,
                "RasterImage has no image data; skipping"
            );
            return;
        }

        // Calcular esquina inferior izquierda
        let start_x = raster.center.x - raster.width_mm / 2.0;
        let start_y = raster.center.y - raster.height_mm / 2.0;

        // Añadir comentarios
        gcode.push_str(&format!("\n; Raster Image ID={}\n", shape.id));
        gcode.push_str(&format!("; Name: {}\n", shape.name));
        gcode.push_str(&format!("; Position: ({:.3}, {:.3})\n", start_x, start_y));
        gcode.push_str(&format!(
            "; Size: {:.2} x {:.2} mm\n",
            raster.width_mm, raster.height_mm
        ));

        // Crear parámetros y generar G-code
        let params = raster.engraving_params();

        let engraver_result = if let Some(path) = &raster.original_path {
            ImageEngraver::from_file(path, params)
        } else if !raster.image_data.is_empty() {
            match image::load_from_memory(&raster.image_data) {
                Ok(img) => ImageEngraver::from_image(img, params),
                Err(e) => {
                    tracing::error!(shape_id = shape.id, error = %e, "Failed to load raster image from memory");
                    return;
                }
            }
        } else {
            return;
        };

        match engraver_result {
            Ok(engraver) => match engraver.generate_gcode() {
                Ok(image_gcode) => {
                    gcode.push_str(&image_gcode);
                    gcode.push('\n');
                }
                Err(e) => {
                    tracing::error!(shape_id = shape.id, error = %e, "Failed to generate raster G-code");
                }
            },
            Err(e) => {
                tracing::error!(
                    shape_id = shape.id,
                    error = %e,
                    "Failed to create raster image engraver"
                );
            }
        }
    }

    /// Shape phase: the comment block naming the shape, its operation,
    /// geometry and depths.
    fn emit_shape_comments(
        &self,
        gcode: &mut String,
        gcode_gen: &ToolpathToGcode,
        shape: &DrawingObject,
        pocket_fallback_to_profile: bool,
    ) {
        // Add shape metadata as comments
        gcode.push_str(&format!(
            "\n; Shape ID={}, Type={:?}\n",
            shape.id,
            shape.shape.shape_type()
        ));
        gcode.push_str(&format!("; Name: {}\n", shape.name));
        gcode.push_str(&format!("; Operation: {:?}\n", shape.operation_type));
        if pocket_fallback_to_profile {
            gcode.push_str("; NOTE: Text pocketing produced no valid pocket area for the current tool/text size; fell back to profile toolpath.\n");
        }

        // Add shape-specific data
        Self::append_shape_metadata(gcode, shape);

        if shape.operation_type == OperationType::Pocket {
            gcode.push_str(&format!(
                "; Pocket depth: {:.3}mm, Step down: {:.3}mm, Step in: {:.3}mm\n",
                shape.effective_pocket_depth(),
                shape.step_down,
                shape.step_in
            ));
            gcode.push_str(&format!("; Strategy: {:?}\n", shape.pocket_strategy));
        } else {
            if gcode_gen.is_laser_2d {
            } else {
                let cnc_effective_depth = if self.machine_mode() == MachineMode::Cnc3D {
                    let stock_top_z = self
                        .stock_material
                        .as_ref()
                        .map(|s| s.thickness as f64)
                        .unwrap_or(10.0);
                    let object_final_z = self.effective_cnc_start_depth_from_shape(shape);
                    (stock_top_z - object_final_z).max(0.0)
                } else {
                    shape.pocket_depth
                };
                gcode.push_str(&format!(
                    "; Cut depth: {:.3}mm, Step down: {:.3}mm\n",
                    cnc_effective_depth, shape.step_down
                ));
            }
        }
    }

    /// Shape phase: `num_passes` runs over the shape's toolpaths (laser
    /// toolpaths are optimised first), continuing the line numbering and the
    /// CNC spindle speed in effect.
    fn emit_shape_passes(
        gcode: &mut String,
        gcode_gen: &ToolpathToGcode,
        toolpaths: &[crate::Toolpath],
        num_passes: usize,
        line_number: &mut u32,
        spindle_speed: &mut u32,
    ) {
        let mut current_z = gcode_gen.safe_z;
        for pass in 0..num_passes {
            if pass > 0 {
                // Reposition to the start for subsequent passes
                gcode.push_str(&format!("; Reposition for pass {}\n", pass + 1));
            }

            if gcode_gen.is_laser_2d {
                for toolpath in toolpaths {
                    // Optimizes curves and joins collinear segments
                    let optimized = gcode_gen.optimize_toolpath_for_laser(toolpath);

                    let (body_gcode, final_z) =
                        gcode_gen.generate_body_continuing(&optimized, *line_number, current_z);

                    gcode.push_str(&body_gcode);
                    *line_number += (optimized.segments.len() as u32) * 10;
                    current_z = final_z;
                }
            } else {
                // CNC mode without optimization
                for toolpath in toolpaths {
                    if let Some(speed) = toolpath.segments.first().map(|s| s.spindle_speed)
                        && speed != *spindle_speed
                    {
                        gcode.push_str(&format!("S{}        ; Spindle speed\n", speed));
                        *spindle_speed = speed;
                    }
                    let (body_gcode, final_z) =
                        gcode_gen.generate_body_continuing(toolpath, *line_number, current_z);
                    gcode.push_str(&body_gcode);
                    *line_number += (toolpath.segments.len() as u32) * 10;
                    current_z = final_z;
                }
            }
        }
    }

    /// Generates G-code from the current design and reports whether any toolpath
    /// falls outside the machine limits.
    #[allow(clippy::collapsible_else_if)]
    pub fn generate_gcode_with_warning_info(
        &mut self,
        machine_limits: Option<MachineLimits>,
    ) -> (String, bool) {
        let mut gcode = String::new();
        let gcode_gen = self.configured_gcode_generator(machine_limits);
        let shape_toolpaths = self.build_shape_toolpaths();

        // Calculate total length from all toolpaths
        let total_length: f64 = shape_toolpaths
            .iter()
            .flat_map(|(_, tps, _)| tps.iter())
            .map(|tp| tp.total_length())
            .sum();

        // Use settings from first toolpath if available, or defaults
        // Taken from the first toolpath that cuts anything: a shape can yield
        // empty toolpaths (e.g. a pocket smaller than the cutter).
        let (header_speed, header_feed, header_diam) = shape_toolpaths
            .iter()
            .flat_map(|(_, tps, _)| tps.iter())
            .find_map(|tp| {
                tp.segments
                    .first()
                    .map(|seg| (seg.spindle_speed, seg.feed_rate, tp.tool_diameter))
            })
            .unwrap_or((3000, 100.0, 3.175));
        // The header reports the deepest Z any pass reaches.
        let header_depth = shape_toolpaths
            .iter()
            .flat_map(|(_, tps, _)| tps.iter())
            .map(|tp| tp.depth)
            .reduce(f64::min)
            .unwrap_or(0.0);

        gcode.push_str(&gcode_gen.generate_header(
            header_speed,
            header_feed,
            header_diam,
            header_depth,
            total_length,
        ));

        let has_violation = shape_toolpaths
            .iter()
            .flat_map(|(_, tps, _)| tps.iter())
            .any(|tp| gcode_gen.has_boundary_violation(tp));
        if has_violation {
            gcode.push_str(LIMIT_WARNING);
        }

        self.emit_shapes_gcode(&mut gcode, &gcode_gen, &shape_toolpaths, header_speed);

        gcode.push_str(&gcode_gen.generate_footer());

        self.generated_gcode = gcode.clone();
        self.gcode_generated = self.canvas.shape_count() > 0;
        (gcode, has_violation)
    }

    /// Generates G-code from the current design.
    pub fn generate_gcode(&mut self, machine_limits: Option<MachineLimits>) -> String {
        self.generate_gcode_with_warning_info(machine_limits).0
    }

    /// Appends shape-specific metadata to G-code comments.
    fn append_shape_metadata(gcode: &mut String, shape: &DrawingObject) {
        match &shape.shape {
            crate::model::Shape::Rectangle(rect) => {
                let (x1, y1, x2, y2) = rect.bounds();
                gcode.push_str(&format!(
                    "; Position: ({:.3}, {:.3}) to ({:.3}, {:.3})\n",
                    x1, y1, x2, y2
                ));
                gcode.push_str(&format!("; Corner radius: {:.3}mm\n", rect.corner_radius));
            }
            crate::model::Shape::Circle(circle) => {
                gcode.push_str(&format!(
                    "; Center: ({:.3}, {:.3}), Radius: {:.3}mm\n",
                    circle.center.x, circle.center.y, circle.radius
                ));
            }
            crate::model::Shape::Line(line) => {
                gcode.push_str(&format!(
                    "; Start: ({:.3}, {:.3}), End: ({:.3}, {:.3})\n",
                    line.start.x, line.start.y, line.end.x, line.end.y
                ));
            }
            crate::model::Shape::Ellipse(ellipse) => {
                let (x1, y1, x2, y2) = ellipse.bounds();
                gcode.push_str(&format!(
                    "; Position: ({:.3}, {:.3}) to ({:.3}, {:.3})\n",
                    x1, y1, x2, y2
                ));
            }
            crate::model::Shape::Path(path) => {
                let (x1, y1, x2, y2) = path.bounds();
                gcode.push_str(&format!(
                    "; Path bounds: ({:.3}, {:.3}) to ({:.3}, {:.3})\n",
                    x1, y1, x2, y2
                ));
            }
            crate::model::Shape::Text(text) => {
                gcode.push_str(&format!(
                    "; Text: \"{}\", Font size: {:.3}mm\n",
                    text.text, text.font_size
                ));
                gcode.push_str(&format!("; Position: ({:.3}, {:.3})\n", text.x, text.y));
            }
            crate::model::Shape::Triangle(triangle) => {
                gcode.push_str(&format!(
                    "; Triangle: Center ({:.3}, {:.3}), Width: {:.3}mm, Height: {:.3}mm\n",
                    triangle.center.x, triangle.center.y, triangle.width, triangle.height
                ));
            }
            crate::model::Shape::Polygon(polygon) => {
                gcode.push_str(&format!(
                    "; Polygon: Center ({:.3}, {:.3}), Radius: {:.3}mm, Sides: {}\n",
                    polygon.center.x, polygon.center.y, polygon.radius, polygon.sides
                ));
            }
            crate::model::Shape::Gear(gear) => {
                gcode.push_str(&format!(
                    "; Gear: Center ({:.3}, {:.3}), Module: {:.3}, Teeth: {}\n",
                    gear.center.x, gear.center.y, gear.module, gear.teeth
                ));
            }
            crate::model::Shape::Sprocket(sprocket) => {
                gcode.push_str(&format!(
                    "; Sprocket: Center ({:.3}, {:.3}), Pitch: {:.3}, Teeth: {}\n",
                    sprocket.center.x, sprocket.center.y, sprocket.pitch, sprocket.teeth
                ));
            }

            crate::model::Shape::RasterImage(raster) => {
                let (x1, y1, x2, y2) = raster.bounds();
                gcode.push_str(&format!(
                    "; Raster Image: ({:.3}, {:.3}) to ({:.3}, {:.3})\n",
                    x1, y1, x2, y2
                ));
                if let Some(path) = &raster.original_path {
                    gcode.push_str(&format!("; Original file: {}\n", path.display()));
                }
            }
        }
    }
}
