//! File I/O operations (save, load, new) for designer state.

use super::DesignerState;
use crate::model::Shape;

/// One shape of a design file that a load could not restore in full.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeLoadIssue {
    /// The shape's id in the file.
    pub id: i32,
    /// The shape's name in the file (may be empty).
    pub name: String,
    /// The shape's `shape_type` in the file.
    pub shape_type: String,
    /// Why it was not restored: the conversion error, or the raster's image file path.
    pub detail: String,
}

impl ShapeLoadIssue {
    /// A short label for the shape: its name, or `#id`, followed by its type.
    pub fn label(&self) -> String {
        if self.name.is_empty() {
            format!("#{} ({})", self.id, self.shape_type)
        } else {
            format!("{} ({})", self.name, self.shape_type)
        }
    }
}

/// What [`DesignerState::load_from_file_report`] could not restore.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadReport {
    /// Shapes left out of the design, for example an unknown `shape_type` written by a newer
    /// version. A save rewrites the file without them.
    pub skipped: Vec<ShapeLoadIssue>,
    /// Raster images loaded without pixels because their image file is missing or unreadable.
    pub missing_images: Vec<ShapeLoadIssue>,
}

impl LoadReport {
    /// Whether every shape was restored in full.
    pub fn is_clean(&self) -> bool {
        self.skipped.is_empty() && self.missing_images.is_empty()
    }
}

impl DesignerState {
    /// Save design to file.
    pub fn save_to_file(&mut self, path: impl AsRef<std::path::Path>) -> anyhow::Result<()> {
        use crate::serialization::{DesignFile, DesignMode};

        let mut design = DesignFile::new(&self.design_name);
        // Keep the loaded file's creation time, author and description.
        if let Some(meta) = &self.file_metadata {
            design.metadata.created = meta.created;
            design.metadata.author = meta.author.clone();
            design.metadata.description = meta.description.clone();
        }

        // Persist current machine mode as document mode for phase-based 2D/3D workflows.
        design.design_mode = match self.machine_mode() {
            super::MachineMode::Laser2D => DesignMode::TwoD,
            super::MachineMode::Cnc3D => DesignMode::ThreeD,
        };

        // Save viewport state
        design.viewport.zoom = self.canvas.zoom();
        design.viewport.pan_x = self.canvas.pan_x();
        design.viewport.pan_y = self.canvas.pan_y();

        // Save all shapes
        for obj in self.canvas.shapes() {
            design.shapes.push(DesignFile::from_drawing_object(obj));
        }

        // Save default properties
        design.default_properties = Some(DesignFile::from_drawing_object(
            &self.default_properties_shape,
        ));

        // Save tool settings
        design.toolpath_params.feed_rate = self.tool_settings.feed_rate;
        design.toolpath_params.spindle_speed = self.tool_settings.spindle_speed as f64;
        design.toolpath_params.tool_diameter = self.tool_settings.tool_diameter;
        design.toolpath_params.cut_depth = self.tool_settings.cut_depth;
        design.toolpath_params.step_down = self.tool_settings.step_down;
        design.toolpath_params.start_depth = self.tool_settings.start_depth;
        design.toolpath_params.continuous_z_between_passes =
            self.tool_settings.continuous_z_between_passes;

        // Save stock settings
        if let Some(stock) = &self.stock_material {
            design.toolpath_params.stock_width = stock.width;
            design.toolpath_params.stock_height = stock.height;
            design.toolpath_params.stock_thickness = stock.thickness;
            design.toolpath_params.safe_z_height = stock.safe_z;
            (
                design.toolpath_params.stock_origin_x,
                design.toolpath_params.stock_origin_y,
                design.toolpath_params.stock_origin_z,
            ) = stock.origin;
        }

        // Save to file
        design.save_to_file(&path)?;

        // Update state
        self.current_file_path = Some(path.as_ref().to_path_buf());
        self.is_modified = false;

        Ok(())
    }

    /// Load design from file. Shapes that cannot be restored are left out; use
    /// [`Self::load_from_file_report`] to learn which.
    pub fn load_from_file(&mut self, path: impl AsRef<std::path::Path>) -> anyhow::Result<()> {
        self.load_from_file_report(path).map(|_| ())
    }

    /// Load design from file and report the shapes it could not restore in full.
    ///
    /// # Arguments
    /// * `path` - The design file.
    ///
    /// # Returns
    /// The shapes left out and the raster images without their image file.
    ///
    /// # Errors
    /// The file cannot be read or parsed; the design is then unchanged.
    pub fn load_from_file_report(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> anyhow::Result<LoadReport> {
        use crate::serialization::{DesignFile, DesignMode};
        use crate::stock_removal::StockMaterial;

        let design = DesignFile::load_from_file(&path)?;

        // Restore machine mode from serialized design mode (old files default to 2D).
        self.set_machine_mode(match design.design_mode {
            DesignMode::TwoD => super::MachineMode::Laser2D,
            DesignMode::ThreeD => super::MachineMode::Cnc3D,
        });

        // Clear existing shapes
        self.canvas.clear();

        // Restore viewport
        self.canvas.set_zoom(design.viewport.zoom);
        self.canvas
            .set_pan(design.viewport.pan_x, design.viewport.pan_y);

        // Restore shapes with full DrawingObject (preserves all properties)
        let mut report = LoadReport::default();
        for shape_data in &design.shapes {
            let issue = |detail: String| ShapeLoadIssue {
                id: shape_data.id,
                name: shape_data.name.clone(),
                shape_type: shape_data.shape_type.clone(),
                detail,
            };
            match DesignFile::to_drawing_object(shape_data, shape_data.id) {
                Ok(obj) => {
                    if let Shape::RasterImage(raster) = &obj.shape
                        && raster.image_data.is_empty()
                    {
                        report
                            .missing_images
                            .push(issue(shape_data.original_path.clone().unwrap_or_default()));
                    }
                    self.canvas.restore_shape(obj);
                }
                Err(error) => {
                    tracing::warn!(id = shape_data.id, %error, "design shape not loaded");
                    report.skipped.push(issue(format!("{error:#}")));
                }
            }
        }

        // Restore default properties
        if let Some(default_props) = &design.default_properties
            && let Ok(obj) = DesignFile::to_drawing_object(default_props, 0)
        {
            self.default_properties_shape = obj;
        }

        // Restore tool settings
        self.tool_settings.feed_rate = design.toolpath_params.feed_rate;
        self.tool_settings.spindle_speed = design.toolpath_params.spindle_speed as u32;
        self.tool_settings.tool_diameter = design.toolpath_params.tool_diameter;
        self.tool_settings.cut_depth = design.toolpath_params.cut_depth;
        self.tool_settings.start_depth = design.toolpath_params.start_depth;
        // Machine mode is restored above, so step-down is validated for that mode.
        // A non-positive value means "unset" (fall back to per-object defaults).
        if design.toolpath_params.step_down > 0.0 {
            self.set_step_down(design.toolpath_params.step_down);
        } else {
            self.tool_settings.step_down = 0.0;
        }
        self.tool_settings.continuous_z_between_passes =
            design.toolpath_params.continuous_z_between_passes;

        // Also update the toolpath generator to match
        self.toolpath_generator
            .set_feed_rate(design.toolpath_params.feed_rate);
        self.toolpath_generator
            .set_spindle_speed(design.toolpath_params.spindle_speed as u32);
        self.toolpath_generator
            .set_tool_diameter(design.toolpath_params.tool_diameter);
        self.toolpath_generator
            .set_cut_depth(design.toolpath_params.cut_depth);

        // Restore stock settings
        let mut stock_material = StockMaterial {
            width: design.toolpath_params.stock_width,
            height: design.toolpath_params.stock_height,
            thickness: design.toolpath_params.stock_thickness,
            origin: (
                design.toolpath_params.stock_origin_x,
                design.toolpath_params.stock_origin_y,
                design.toolpath_params.stock_origin_z,
            ),
            safe_z: design.toolpath_params.safe_z_height,
        };
        stock_material.normalize_safe_z();
        self.stock_material = Some(stock_material);

        // Update state
        self.design_name = design.metadata.name.clone();
        self.file_metadata = Some(design.metadata);
        self.current_file_path = Some(path.as_ref().to_path_buf());
        self.is_modified = false;
        self.clear_history();

        Ok(report)
    }

    /// Create new design (clear all).
    pub fn new_design(&mut self) {
        self.canvas.clear();
        self.generated_gcode.clear();
        self.gcode_generated = false;
        self.current_file_path = None;
        self.is_modified = false;
        self.design_name = "Untitled".to_string();
        self.file_metadata = None;
        self.clear_history();
    }

    /// Get display name for the design.
    pub fn display_name(&self) -> String {
        let name = if let Some(path) = &self.current_file_path {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&self.design_name)
        } else {
            &self.design_name
        };

        if self.is_modified {
            format!("{}*", name)
        } else {
            name.to_string()
        }
    }
}
