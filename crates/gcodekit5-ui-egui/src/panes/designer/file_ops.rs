//! Designer document operations: New, Open and Save of design files, and Import of SVG, DXF
//! and raster images. The app asks for the files ([`crate::dialogs`]) and the unsaved-changes
//! question; these run once it has the answer.
//!
//! Ported from the GTK `designer/file_ops.rs` and the canvas's `import_raster_image`:
//!
//! - New clears the design and its undo history in the chosen mode (2D laser, 3D CNC).
//! - Open loads a `.gckd` / `.gck5` file through the model's serialization into a copy of
//!   the design, so a file that cannot be read leaves the design as it was. Like GTK, a
//!   non-positive step down becomes 0.1 mm and the ids continue after the loaded shapes;
//!   a stored zoom that is not usable fits the view instead.
//! - Save writes through the model's serialization to a temporary file beside the target,
//!   then renames it ([`crate::files`]): a read-only target is refused and a failed save
//!   leaves the design modified.
//! - Import adds the shapes of an SVG or DXF file (each as its own undo step), applies a
//!   DXF entity's elevation and thickness, switches to CNC mode when any shape has depth,
//!   fits the view and reports discarded solids and replaced numbers. An image becomes a
//!   raster shape, in laser mode only.
//!
//! New and Open also forget the layers panel's hidden and locked shapes: they are view state
//! keyed by shape id, and ids repeat between documents. The outcome is shown as the pane's
//! notice ([`DesignerPane::notice`]), GTK's status label and warning dialogs.
//!
//! Not ported: STL import (Settings' "enable STL import"), remembering the last folder in
//! the settings, the window title's file name.
//!
//! Dependencies: `gcodekit5-designer` (serialization, importers), `gcodekit5-app`
//! (translations), `anyhow`.

use std::path::Path;

use gcodekit5_app::i18n::Translator;
use gcodekit5_designer::designer_state::MachineMode;
use gcodekit5_designer::image_importer::ImageImporter;
use gcodekit5_designer::import::{DxfImporter, ImportedDesign, SvgImporter};

use super::DesignerPane;
use super::canvas::editing;

/// The msgids the document operations show, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("New 2D design created"),
        n_!("New 3D design created"),
        n_!("Loaded:"),
        n_!("Open Failed"),
        n_!("Saved:"),
        n_!("Save Failed"),
        n_!("Imported:"),
        n_!("Error importing file:"),
        n_!("3D depth detected"),
        n_!("Object with depth: Switching to CNC 3D mode."),
        n_!("Unsupported 3D solids discarded"),
        n_!(
            "This DXF file contains ACIS solid entities (3D solids) that are not supported and were discarded:"
        ),
        n_!("Invalid numbers replaced"),
        n_!(
            "values in this DXF file were not numbers and were replaced by defaults, so some geometry may be wrong."
        ),
        n_!("Raster image import is only available in 2D mode"),
        n_!("Image imported successfully"),
        n_!("Import error"),
        n_!("Close"),
    ]
}

impl DesignerPane {
    /// The file the design was last opened from or saved to.
    pub fn design_path(&self) -> Option<&Path> {
        self.state.current_file_path.as_deref()
    }

    /// The pane's notice: the outcome of the last file operation, or why it was refused.
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// Shows `message` as the pane's notice until it is closed or replaced.
    ///
    /// # Arguments
    /// * `message` - The translated message.
    pub fn set_notice(&mut self, message: String) {
        self.notice = Some(message);
    }

    /// Starts a new, empty design in `mode`; the caller has dealt with unsaved changes.
    ///
    /// # Arguments
    /// * `mode` - Laser 2D (File > New 2D) or CNC 3D (File > New 3D).
    /// * `tr` - Translator for the notice.
    pub fn new_design(&mut self, mode: MachineMode, tr: &Translator) {
        self.state.new_design();
        self.state.set_machine_mode(mode);
        self.state.is_modified = false;
        self.forget_view_state();
        let msgid = match mode {
            MachineMode::Laser2D => n_!("New 2D design created"),
            MachineMode::Cnc3D => n_!("New 3D design created"),
        };
        self.notice = Some(tr.tr(msgid).into_owned());
    }

    /// Opens the design file `path`, replacing the design; the caller has dealt with unsaved
    /// changes. A file that cannot be read leaves the design as it was.
    ///
    /// # Arguments
    /// * `path` - The design file.
    /// * `tr` - Translator for the notice.
    ///
    /// # Returns
    /// Whether the file was opened.
    pub fn open_design(&mut self, path: &Path, tr: &Translator) -> bool {
        let mut next = self.state.clone();
        if let Err(error) = next.load_from_file(path) {
            tracing::warn!(%error, path = %path.display(), "cannot open design");
            self.notice = Some(format!(
                "{}: {}: {error:#}",
                tr.tr("Open Failed"),
                path.display()
            ));
            return false;
        }
        let mut max_id = 0;
        for obj in next.canvas.shapes_mut() {
            max_id = max_id.max(obj.id);
            if obj.step_down <= 0.0 {
                obj.step_down = 0.1;
            }
        }
        next.canvas.set_next_id(max_id + 1);
        next.is_modified = false;
        self.state = next;
        self.forget_view_state();
        let zoom = self.state.canvas.zoom();
        if !(zoom.is_finite() && zoom > 0.0001) && self.state.canvas.shape_count() > 0 {
            editing::zoom_fit(&mut self.state);
        }
        self.notice = Some(format!("{} {}", tr.tr("Loaded:"), path.display()));
        true
    }

    /// Saves the design to `path` (temporary file, then rename) and makes it the design's
    /// file. A failed save leaves the design modified and its file unchanged.
    ///
    /// # Arguments
    /// * `path` - Where to save.
    /// * `tr` - Translator for the notice.
    ///
    /// # Returns
    /// Whether the file was written.
    pub fn save_design(&mut self, path: &Path, tr: &Translator) -> bool {
        let (file, modified) = (self.state.current_file_path.clone(), self.state.is_modified);
        if let Some(stem) = path.file_stem() {
            self.state.design_name = stem.to_string_lossy().into_owned();
        }
        let state = &mut self.state;
        let written = crate::files::write_atomic_with(path, |temp| {
            state.save_to_file(temp).map_err(std::io::Error::other)
        });
        match written {
            Ok(()) => {
                self.state.current_file_path = Some(path.to_path_buf());
                self.state.is_modified = false;
                self.notice = Some(format!("{} {}", tr.tr("Saved:"), path.display()));
                true
            }
            Err(error) => {
                tracing::warn!(%error, path = %path.display(), "cannot save design");
                // The model's save names the temporary file as the design's file.
                self.state.current_file_path = file;
                self.state.is_modified = modified;
                self.notice = Some(format!(
                    "{}: {}: {error}",
                    tr.tr("Save Failed"),
                    path.display()
                ));
                false
            }
        }
    }

    /// Imports the shapes of the SVG or DXF file `path` into the design.
    ///
    /// # Arguments
    /// * `path` - The file; its extension picks the importer.
    /// * `tr` - Translator for the notice.
    ///
    /// # Returns
    /// Whether anything was imported.
    pub fn import_file(&mut self, path: &Path, tr: &Translator) -> bool {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase);
        let result = match extension.as_deref() {
            Some("svg") => std::fs::read_to_string(path)
                .map_err(|e| anyhow::anyhow!("Failed to read file: {e}"))
                .and_then(|content| SvgImporter::new(1.0, 0.0, 0.0).import_string(&content)),
            Some("dxf") => DxfImporter::new(1.0, 0.0, 0.0).import_file(&path.to_string_lossy()),
            Some(_) => Err(anyhow::anyhow!("Unsupported file format")),
            None => Err(anyhow::anyhow!("Unknown file extension")),
        };
        match result {
            Ok(design) => {
                let messages = self.add_imported(design, tr);
                let mut notice = format!("{} {}", tr.tr("Imported:"), path.display());
                for message in messages {
                    notice.push('\n');
                    notice.push_str(&message);
                }
                self.notice = Some(notice);
                true
            }
            Err(error) => {
                tracing::warn!(%error, path = %path.display(), "cannot import");
                self.notice = Some(format!("{} {error:#}", tr.tr("Error importing file:")));
                false
            }
        }
    }

    /// Adds the imported shapes, with their depth, then fits the view.
    ///
    /// # Returns
    /// The warnings to show: CNC mode switched on, solids discarded, numbers replaced.
    fn add_imported(&mut self, mut design: ImportedDesign, tr: &Translator) -> Vec<String> {
        let unsupported = std::mem::take(&mut design.unsupported_entities);
        let mut z = std::mem::take(&mut design.z_metadata);
        if z.len() != design.shapes.len() {
            z = vec![(0.0, 0.0); design.shapes.len()];
        }
        let mut with_depth = false;
        for (shape, (elevation, thickness)) in design.shapes.into_iter().zip(z) {
            if elevation.abs() > f64::EPSILON || thickness.abs() > f64::EPSILON {
                with_depth = true;
                self.state
                    .add_shape_with_undo_and_depth(shape, elevation, thickness.abs());
            } else {
                self.state.add_shape_with_undo(shape);
            }
        }
        let mut messages = Vec::new();
        // Laser 2D ignores per-shape Z, so geometry with depth switches to CNC 3D.
        if with_depth && self.state.machine_mode() != MachineMode::Cnc3D {
            self.state.set_machine_mode(MachineMode::Cnc3D);
            messages.push(format!(
                "{}: {}",
                tr.tr("3D depth detected"),
                tr.tr("Object with depth: Switching to CNC 3D mode.")
            ));
        }
        if !unsupported.is_empty() {
            let details: Vec<String> = unsupported
                .iter()
                .map(|(name, count)| format!("{name} (x{count})"))
                .collect();
            messages.push(format!(
                "{}: {} {}",
                tr.tr("Unsupported 3D solids discarded"),
                tr.tr("This DXF file contains ACIS solid entities (3D solids) that are not supported and were discarded:"),
                details.join(", ")
            ));
        }
        if design.replaced_numbers > 0 {
            messages.push(format!(
                "{}: {} {}",
                tr.tr("Invalid numbers replaced"),
                design.replaced_numbers,
                tr.tr("values in this DXF file were not numbers and were replaced by defaults, so some geometry may be wrong.")
            ));
        }
        editing::zoom_fit(&mut self.state);
        messages
    }

    /// Imports the image `path` as a raster shape (laser mode only), as one undo step.
    ///
    /// # Arguments
    /// * `path` - A JPEG, PNG, BMP, TIFF or WebP image.
    /// * `tr` - Translator for the notice.
    ///
    /// # Returns
    /// Whether the image was added.
    pub fn import_image(&mut self, path: &Path, tr: &Translator) -> bool {
        if self.state.machine_mode() == MachineMode::Cnc3D {
            self.notice = Some(
                tr.tr("Raster image import is only available in 2D mode")
                    .into_owned(),
            );
            return false;
        }
        match ImageImporter::import(path) {
            Ok(shape) => {
                self.state.add_shape_with_undo(shape);
                self.notice = Some(tr.tr("Image imported successfully").into_owned());
                true
            }
            Err(error) => {
                tracing::warn!(%error, path = %path.display(), "image import failed");
                self.notice = Some(format!("{}: {error:#}", tr.tr("Import error")));
                false
            }
        }
    }

    /// Forgets the view state of the previous document: hidden and locked shapes, a drag or
    /// polyline, the effect preview and an armed gallery shape.
    fn forget_view_state(&mut self) {
        self.flags.clear();
        self.input.reset();
        self.preview.clear();
        self.gallery.cancel();
    }

    /// The notice strip above the canvas, with a Close button.
    pub(super) fn show_notice(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        let Some(message) = &self.notice else {
            return;
        };
        let mut close = false;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                close = ui.button(tr.tr("Close")).clicked();
                ui.add(egui::Label::new(message.as_str()).wrap());
            });
        });
        if close {
            self.notice = None;
        }
    }
}
