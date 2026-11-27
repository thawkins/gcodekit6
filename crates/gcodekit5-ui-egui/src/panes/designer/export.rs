//! Designer output: G-code (File > Export G-Code to a file, the toolbox's Generate G-Code
//! and Frame to the editor) and SVG (File > Export SVG).
//!
//! Every G-code export goes through [`DesignerPane::generate_gcode`], which works on a copy
//! of the design without the layers panel's hidden shapes, so a hidden shape is never cut
//! (it reads nothing back from the live design: the copy's `generated_gcode` is discarded).
//! Before generating, GTK's checks apply ([`DesignerPane::export_check`], which generates
//! nothing): nothing to export, every shape hidden ("Nothing to
//! export: all shapes are hidden"), or objects at or above Safe Z in CNC mode (export
//! blocked, with the counts). Toolpaths outside the machine limits still export, with GTK's
//! warning. Generate G-Code with one raster image selected engraves that image instead.
//!
//! The toolbox's buttons only ask ([`DesignerPane::request_to_editor`]); the app checks for a
//! running job and unsaved editor edits first, then calls [`DesignerPane::generate_to_editor`]
//! or [`DesignerPane::frame_to_editor`] and takes the result from
//! [`DesignerPane::take_editor_gcode`].
//!
//! Ported from the GTK `designer/file_ops.rs` (`export_gcode`, `export_svg`),
//! `designer/mod.rs` (Generate G-Code, frame bounds) and `designer_toolbox.rs`
//! (`generate_frame_gcode`).
//!
//! Dependencies: `gcodekit5-designer` (G-code generation, image engraving),
//! `gcodekit5-app` (translations).

use std::path::Path;

use gcodekit5_app::i18n::Translator;
use gcodekit5_designer::DesignerState;
use gcodekit5_designer::engraving::image_engraver::{ImageEngraver, OVERSCAN_MM};
use gcodekit5_designer::gcode_gen::MachineLimits;
use gcodekit5_designer::model::{DesignerShape, RasterImage, Shape};

use super::DesignerPane;
use super::svg_export;

/// What the toolbox asks to send to the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToEditor {
    /// Generate G-Code: the design's program.
    Generate,
    /// Frame: a low-power frame around the design.
    Frame,
}

/// Why a G-code export did not run.
#[derive(Clone, Debug, PartialEq)]
pub enum ExportRefusal {
    /// The design has no shapes.
    Empty,
    /// Every shape is hidden in the layers panel.
    AllHidden,
    /// In CNC mode, `count` objects start at or above Safe Z.
    AboveSafeZ {
        /// The Safe Z height, mm.
        safe_z: f64,
        /// How many objects conflict.
        count: usize,
        /// The highest start Z among them, mm.
        max_start_z: f64,
    },
}

impl ExportRefusal {
    /// The translated message for the user.
    ///
    /// # Arguments
    /// * `tr` - Translator.
    /// * `to_file` - Exporting to a file (else generating for the editor), for the wording.
    pub fn message(&self, tr: &Translator, to_file: bool) -> String {
        match self {
            Self::Empty => tr.tr("Nothing to export").into_owned(),
            Self::AllHidden => tr
                .tr("Nothing to export: all shapes are hidden")
                .into_owned(),
            Self::AboveSafeZ {
                safe_z,
                count,
                max_start_z,
            } => {
                let (blocked, lower) = if to_file {
                    (
                        n_!("Export blocked: objects at/above Safe Z"),
                        n_!("Lower object Z positions before exporting G-code."),
                    )
                } else {
                    (
                        n_!("G-code blocked: objects at/above Safe Z"),
                        n_!("Lower object Z positions before generating G-code."),
                    )
                };
                format!(
                    "{}\n{} {}\n{}: {safe_z:.3} mm, {}: {count} ({}: {max_start_z:.3} mm)",
                    tr.tr(blocked),
                    tr.tr("One or more objects are positioned at or above the Safe Z height."),
                    tr.tr(lower),
                    tr.tr("Safe Z"),
                    tr.tr("Objects in conflict"),
                    tr.tr("highest start Z"),
                )
            }
        }
    }
}

/// Generated G-code, and whether a toolpath leaves the machine limits.
#[derive(Clone, Debug, PartialEq)]
pub struct GcodeExport {
    /// The program.
    pub gcode: String,
    /// A toolpath falls outside the machine's working area.
    pub out_of_limits: bool,
}

/// GTK's out-of-limits warning.
const OUT_OF_LIMITS: &str = n_!(
    "The generated G-code contains coordinates outside the machine working area (Negative values). Review the path before sending it to the machine."
);

/// The msgids the export shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Nothing to export"),
        n_!("Nothing to export: all shapes are hidden"),
        n_!("Export blocked: objects at/above Safe Z"),
        n_!("G-code blocked: objects at/above Safe Z"),
        n_!("One or more objects are positioned at or above the Safe Z height."),
        n_!("Lower object Z positions before exporting G-code."),
        n_!("Lower object Z positions before generating G-code."),
        n_!("Safe Z"),
        n_!("Objects in conflict"),
        n_!("highest start Z"),
        n_!("Out of limits warning"),
        OUT_OF_LIMITS,
        n_!("Exported G-Code:"),
        n_!("Exported SVG:"),
        n_!("Export Failed"),
        n_!("G-Code generated"),
        n_!("G-Code generated for image"),
        n_!("Image Engraving Failed"),
        n_!("Image Load Failed"),
        n_!("Generate G-Code"),
        n_!("Frame "),
        n_!("Generate low-power frame to position material"),
    ]
}

/// GTK's low-power laser frame around (x1, y1)-(x2, y2) mm.
fn frame_gcode(x1: f64, y1: f64, x2: f64, y2: f64) -> String {
    format!(
        "; --- Laser Frame Start ---\n\
         G90 ; Absolute coordinates\n\
         M3 S10 ; Laser ON low power\n\
         G0 X{x1:.3} Y{y1:.3} F3000 ; Go to corner\n\
         G1 X{x2:.3} Y{y1:.3} F1000 ; Bottom side\n\
         G1 X{x2:.3} Y{y2:.3} ; Right side\n\
         G1 X{x1:.3} Y{y2:.3} ; Top side\n\
         G1 X{x1:.3} Y{y1:.3} ; Left side\n\
         M5 ; Laser OFF\n\
         G0 X0 Y0 ; Return to origin\n\
         M30 ; End of program\n\
         ; --- Laser Frame End ---"
    )
}

impl DesignerPane {
    /// Asks for G-code for the editor (the toolbox's Generate G-Code or Frame); the app
    /// takes the request ([`DesignerPane::take_editor_request`]) and decides when to run it.
    ///
    /// # Arguments
    /// * `what` - Generate G-Code or Frame.
    pub fn request_to_editor(&mut self, what: ToEditor) {
        self.editor_request = Some(what);
    }

    /// Takes the pending request for editor G-code, once.
    pub fn take_editor_request(&mut self) -> Option<ToEditor> {
        self.editor_request.take()
    }

    /// A copy of the design without the hidden shapes (the model sets its toolpath generator
    /// from the tool settings when it generates).
    fn export_copy(&self) -> DesignerState {
        let mut copy = self.state.clone();
        for id in self.state.canvas.shape_store.draw_order_iter() {
            if self.flags.is_hidden(id) {
                copy.canvas.remove_shape(id);
            }
        }
        copy
    }

    /// Generates G-code for the design, leaving out hidden shapes.
    ///
    /// Hidden shapes ([`super::ShapeFlags`]) are view state the model does not know, so this
    /// builds the code from a copy of the design without them (the design is untouched, so
    /// its `generated_gcode` stays stale: use the returned value). Every export must go
    /// through here, so a hidden shape is never cut. Locked shapes are cut.
    ///
    /// # Arguments
    /// * `machine_limits` - The machine's limits, if known (as the model's `generate_gcode`).
    pub fn generate_gcode(&self, machine_limits: Option<MachineLimits>) -> String {
        self.export_copy().generate_gcode(machine_limits)
    }

    /// Checks the design and generates its G-code through [`DesignerPane::generate_gcode`]'s
    /// copy (hidden shapes left out).
    ///
    /// # Arguments
    /// * `machine_limits` - The machine's limits, if known.
    ///
    /// # Errors
    /// Nothing to export, every shape hidden, or objects at or above Safe Z.
    pub fn export_gcode(
        &self,
        machine_limits: Option<MachineLimits>,
    ) -> Result<GcodeExport, ExportRefusal> {
        self.export_check()?;
        let (gcode, out_of_limits) = self
            .export_copy()
            .generate_gcode_with_warning_info(machine_limits);
        Ok(GcodeExport {
            gcode,
            out_of_limits,
        })
    }

    /// The export's refusals without generating anything (cheap, for asking before a file
    /// dialog): no shapes, every shape hidden, or visible objects at or above Safe Z in CNC
    /// mode (the model's rule, [`DesignerState::safe_z_clearance_violation_summary`]).
    ///
    /// # Errors
    /// Why the export would be refused.
    pub fn export_check(&self) -> Result<(), ExportRefusal> {
        let canvas = &self.state.canvas;
        if canvas.shape_count() == 0 {
            return Err(ExportRefusal::Empty);
        }
        if canvas.shapes().all(|o| self.flags.is_hidden(o.id)) {
            return Err(ExportRefusal::AllHidden);
        }
        match self
            .state
            .safe_z_clearance_violation_summary(|o| !self.flags.is_hidden(o.id))
        {
            Some((safe_z, count, max_start_z)) => Err(ExportRefusal::AboveSafeZ {
                safe_z,
                count,
                max_start_z,
            }),
            None => Ok(()),
        }
    }

    /// Exports the design's G-code to `path` (File > Export G-Code).
    ///
    /// # Arguments
    /// * `path` - The file to write.
    /// * `tr` - Translator for the notice.
    ///
    /// # Returns
    /// Whether the file was written.
    pub fn write_gcode(&mut self, path: &Path, tr: &Translator) -> bool {
        let export = match self.export_gcode(self.limits.clone()) {
            Ok(export) => export,
            Err(refusal) => {
                self.notice = Some(refusal.message(tr, true));
                return false;
            }
        };
        match crate::files::write_atomic(path, &export.gcode) {
            Ok(()) => {
                let mut notice = format!("{} {}", tr.tr("Exported G-Code:"), path.display());
                push_limits_warning(&mut notice, export.out_of_limits, tr);
                self.notice = Some(notice);
                true
            }
            Err(error) => {
                tracing::warn!(%error, path = %path.display(), "cannot export G-code");
                self.notice = Some(export_failed(path, &error, tr));
                false
            }
        }
    }

    /// Exports the visible shapes as SVG to `path` (File > Export SVG).
    ///
    /// # Arguments
    /// * `path` - The file to write.
    /// * `tr` - Translator for the notice.
    ///
    /// # Returns
    /// Whether the file was written.
    pub fn write_svg(&mut self, path: &Path, tr: &Translator) -> bool {
        let objects: Vec<_> = self
            .state
            .canvas
            .shapes()
            .filter(|o| !self.flags.is_hidden(o.id))
            .collect();
        let Some(svg) = svg_export::design_svg(&objects) else {
            self.notice = Some(tr.tr("Nothing to export").into_owned());
            return false;
        };
        match crate::files::write_atomic(path, &svg) {
            Ok(()) => {
                self.notice = Some(format!("{} {}", tr.tr("Exported SVG:"), path.display()));
                true
            }
            Err(error) => {
                tracing::warn!(%error, path = %path.display(), "cannot export SVG");
                self.notice = Some(export_failed(path, &error, tr));
                false
            }
        }
    }

    /// The toolbox's Generate G-Code: the program for the editor, kept until the app takes
    /// it ([`DesignerPane::take_editor_gcode`]). One selected raster image is engraved;
    /// otherwise the design is generated as File > Export G-Code does.
    ///
    /// # Arguments
    /// * `tr` - Translator for the notice.
    pub fn generate_to_editor(&mut self, tr: &Translator) {
        if let Some(image) = self.selected_image() {
            match engrave(&image) {
                Ok(gcode) => {
                    self.editor_gcode = Some(gcode);
                    self.notice = Some(tr.tr("G-Code generated for image").into_owned());
                }
                Err((title, error)) => {
                    tracing::warn!(%error, "image G-code not generated");
                    self.notice = Some(format!("{}: {error:#}", tr.tr(title)));
                }
            }
            return;
        }
        match self.export_gcode(self.limits.clone()) {
            Ok(export) => {
                self.editor_gcode = Some(export.gcode);
                let mut notice = tr.tr("G-Code generated").into_owned();
                push_limits_warning(&mut notice, export.out_of_limits, tr);
                self.notice = Some(notice);
            }
            Err(refusal) => self.notice = Some(refusal.message(tr, false)),
        }
    }

    /// The toolbox's Frame: a low-power laser frame around the visible selection, or the
    /// visible shapes, for the editor; nothing when there is nothing to frame. A selected raster
    /// image's frame is moved by its overscan, as its engraving is.
    pub fn frame_to_editor(&mut self) {
        let union = |a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)| {
            (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
        };
        let visible = || {
            self.state
                .canvas
                .shapes()
                .filter(|o| !self.flags.is_hidden(o.id))
        };
        let selection = visible()
            .filter(|o| o.selected)
            .map(|o| o.shape.bounds())
            .reduce(union);
        let bounds = selection.or_else(|| visible().map(|o| o.get_total_bounds()).reduce(union));
        let Some((x1, y1, x2, y2)) = bounds else {
            tracing::debug!("frame requested but the canvas is empty");
            return;
        };
        let (dx, dy) = match self.selected_image() {
            Some(img) => {
                let (ox, oy) = (
                    img.center.x - img.width_mm / 2.0,
                    img.center.y - img.height_mm / 2.0,
                );
                let overscan = f64::from(OVERSCAN_MM);
                (ox.max(overscan) - ox, oy.max(overscan) - oy)
            }
            None => (0.0, 0.0),
        };
        self.editor_gcode = Some(frame_gcode(x1 + dx, y1 + dy, x2 + dx, y2 + dy));
    }

    /// Takes the G-code waiting for the editor, once.
    pub fn take_editor_gcode(&mut self) -> Option<String> {
        self.editor_gcode.take()
    }

    /// The selected shape when it is a raster image (GTK's primary selection).
    fn selected_image(&self) -> Option<RasterImage> {
        let id = self.state.canvas.selection_manager.selected_id()?;
        match &self.state.canvas.get_shape(id)?.shape {
            Shape::RasterImage(img) => Some(img.clone()),
            _ => None,
        }
    }
}

/// Engraves `image` with its own laser settings and placed size
/// ([`RasterImage::engraving_params`], as the design export does).
///
/// # Errors
/// The msgid of the failure's title and the error.
fn engrave(image: &RasterImage) -> Result<String, (&'static str, anyhow::Error)> {
    let engraver = ImageEngraver::from_raster_image(image, image.engraving_params())
        .map_err(|e| (n_!("Image Load Failed"), e))?;
    engraver
        .generate_gcode()
        .map_err(|e| (n_!("Image Engraving Failed"), e))
}

/// Appends GTK's out-of-limits warning to `notice` when `out_of_limits`.
fn push_limits_warning(notice: &mut String, out_of_limits: bool, tr: &Translator) {
    if out_of_limits {
        notice.push('\n');
        notice.push_str(&format!(
            "{}: {}",
            tr.tr("Out of limits warning"),
            tr.tr(OUT_OF_LIMITS)
        ));
    }
}

/// "Export Failed: <path>: <error>".
fn export_failed(path: &Path, error: &std::io::Error, tr: &Translator) -> String {
    format!("{}: {}: {error}", tr.tr("Export Failed"), path.display())
}
