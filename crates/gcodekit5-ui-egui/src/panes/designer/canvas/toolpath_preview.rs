//! The canvas's toolpath preview: the toolpaths the G-code is made from, drawn over the
//! shapes while the toolbox's "Show Toolpaths" is on.
//!
//! Ported from the GTK `designer_canvas/toolpath_preview.rs`. The per-shape parameters are
//! resolved exactly as G-code generation resolves them: in CNC mode shapes without custom
//! values use the global and default CAM values and depths are measured from the stock top;
//! in laser mode the shape's own values apply. Hidden shapes are left out, as the export
//! leaves them out.
//!
//! The toolpaths are built on a worker thread from a copy of the visible shapes, so a large
//! design never stalls the frame. The pane counts changes to the design (its revision); a
//! revision newer than the preview's starts a build, at most one at a time: changes made
//! while a build runs coalesce into the next build, which starts when the running one
//! finishes. No build starts during a canvas drag; the release starts one.
//! Rapid moves are dashed in the warning colour at 50%; cuts and arcs are in the success
//! colour at 70% (GTK's colours).
//!
//! Not ported: GTK's status-bar progress and cancel button for the build.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (toolpath generator, model).

use std::sync::mpsc::{Receiver, TryRecvError, channel};

use egui::{Painter, Pos2, Shape as EguiShape, Stroke};
use gcodekit5_designer::DesignerState;
use gcodekit5_designer::canvas::DrawingObject;
use gcodekit5_designer::designer_state::MachineMode;
use gcodekit5_designer::model::{DesignPath, DesignerShape, Shape};
use gcodekit5_designer::shapes::OperationType;
use gcodekit5_designer::toolpath::{Toolpath, ToolpathGenerator, ToolpathSegmentType};

use super::{CanvasView, ShapeFlags};

/// The settings a build needs to resolve each shape's effective CAM parameters.
#[derive(Clone, Debug)]
struct PreviewSettings {
    is_cnc_mode: bool,
    /// Global step down from the tool settings (mm).
    global_step_down: f64,
    /// Stock top Z (mm); G-code assumes 10 mm when no stock is configured.
    stock_top_z: f64,
    /// Default CAM properties for shapes that do not use custom values.
    defaults: DrawingObject,
    feed_rate: f64,
    spindle_speed: u32,
    tool_diameter: f64,
}

impl PreviewSettings {
    /// The settings of `state`.
    fn of(state: &DesignerState) -> Self {
        Self {
            is_cnc_mode: state.machine_mode() == MachineMode::Cnc3D,
            global_step_down: state.tool_settings.step_down,
            stock_top_z: state
                .stock_material
                .as_ref()
                .map_or(10.0, |s| f64::from(s.thickness)),
            defaults: state.default_properties_shape.clone(),
            feed_rate: state.tool_settings.feed_rate,
            spindle_speed: state.tool_settings.spindle_speed,
            tool_diameter: state.tool_settings.tool_diameter,
        }
    }

    /// The shape with the CAM parameters G-code generation would use.
    fn effective_object(&self, shape: &DrawingObject) -> DrawingObject {
        if !self.is_cnc_mode {
            return shape.clone();
        }
        let mut effective = shape.clone();
        if !shape.use_custom_values {
            effective.operation_type = self.defaults.operation_type;
            effective.step_in = self.defaults.step_in;
            effective.ramp_angle = self.defaults.ramp_angle;
            effective.pocket_strategy = self.defaults.pocket_strategy;
            effective.raster_fill_ratio = self.defaults.raster_fill_ratio;
            effective.step_down = if self.global_step_down > 0.0 {
                self.global_step_down as f32
            } else {
                self.defaults.step_down
            };
        }
        if effective.step_down <= 0.0 {
            effective.step_down = 0.1;
        }
        if effective.pocket_depth < 0.0 {
            effective.pocket_depth = 0.0;
        }
        effective
    }

    /// `(start_depth, cut_depth)` for the toolpath generator.
    fn depths(&self, effective: &DrawingObject) -> (f64, f64) {
        if self.is_cnc_mode {
            // Machine Z is absolute (table at 0, stock top at stock thickness); passes start
            // at the stock top and descend to the object's Z.
            let start_z = self.stock_top_z - effective.start_depth;
            (self.stock_top_z, (self.stock_top_z - start_z).max(0.0))
        } else {
            (effective.start_depth, 0.0)
        }
    }
}

/// The toolpaths of one shape, with the per-shape parameters and generator calls of G-code
/// generation.
fn shape_toolpaths(
    generator: &mut ToolpathGenerator,
    settings: &PreviewSettings,
    shape: &DrawingObject,
) -> Vec<Toolpath> {
    let obj = settings.effective_object(shape);
    let (start_depth, cut_depth) = settings.depths(&obj);
    generator.set_pocket_strategy(obj.pocket_strategy);
    generator.set_start_depth(start_depth);
    generator.set_cut_depth(cut_depth);
    generator.set_step_in(f64::from(obj.step_in));
    generator.set_ramp_angle(f64::from(obj.ramp_angle));
    generator.set_raster_fill_ratio(obj.raster_fill_ratio);

    let is_pocket = obj.operation_type == OperationType::Pocket;
    let step_down = f64::from(obj.step_down);
    let step_in = f64::from(obj.step_in);
    let depth = obj.effective_pocket_depth();
    let effective_shape = obj.get_effective_shape();
    match &effective_shape {
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
        Shape::Path(p) if is_pocket => generator.generate_path_pocket(p, depth, step_down, step_in),
        Shape::Path(p) => generator.generate_path_contour(p, step_down),
        Shape::Text(text) if is_pocket => {
            let pocket = generator.generate_text_pocket_toolpath(text, step_down);
            let length: f64 = pocket.iter().map(Toolpath::total_length).sum();
            if length <= 1e-9 {
                // As G-code generation: profile the glyphs when the pocket cuts nothing.
                generator.generate_text_toolpath(text, step_down)
            } else {
                pocket
            }
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
        // Gears and sprockets go through their rendered outline, as in G-code.
        Shape::Gear(_) | Shape::Sprocket(_) => {
            let path = DesignPath::from_lyon_path(&effective_shape.render());
            if is_pocket {
                generator.generate_path_pocket(&path, depth, step_down, step_in)
            } else {
                generator.generate_path_contour(&path, step_down)
            }
        }
    }
}

/// Builds the toolpaths of `shapes`.
fn build(shapes: &[DrawingObject], settings: &PreviewSettings) -> Vec<Toolpath> {
    let mut generator = ToolpathGenerator::new();
    generator.set_feed_rate(settings.feed_rate);
    generator.set_spindle_speed(settings.spindle_speed);
    generator.set_tool_diameter(settings.tool_diameter);
    generator.set_step_in(settings.tool_diameter * 0.4);
    shapes
        .iter()
        .flat_map(|shape| shape_toolpaths(&mut generator, settings, shape))
        .collect()
}

/// The visible shapes of `state`, in draw order.
fn visible(state: &DesignerState, flags: &ShapeFlags) -> Vec<DrawingObject> {
    state
        .canvas
        .shapes()
        .filter(|o| !flags.is_hidden(o.id))
        .cloned()
        .collect()
}

/// The preview's toolpaths and the build in progress.
#[derive(Debug, Default)]
pub struct ToolpathPreview {
    toolpaths: Vec<Toolpath>,
    /// The design revision `toolpaths` were built for.
    built_for: Option<u64>,
    /// The build on the worker thread and the revision it builds for (at most one).
    running: Option<(u64, Receiver<Vec<Toolpath>>)>,
    /// How many builds were started (for tests).
    started: u64,
}

impl ToolpathPreview {
    /// The toolpaths of the last finished build.
    pub fn toolpaths(&self) -> &[Toolpath] {
        &self.toolpaths
    }

    /// Whether the toolpaths were built for design revision `revision` (always true while
    /// the preview is off).
    ///
    /// # Arguments
    /// * `state` - The design (whether the preview is on).
    /// * `revision` - The design's current revision.
    pub fn is_current(&self, state: &DesignerState, revision: u64) -> bool {
        !state.show_toolpaths || (self.built_for == Some(revision) && self.running.is_none())
    }

    /// Builds running: 0 or 1.
    pub fn running(&self) -> usize {
        usize::from(self.running.is_some())
    }

    /// Builds started so far.
    pub fn started(&self) -> u64 {
        self.started
    }

    /// Takes a finished build, then starts one for `revision` when the toolpaths are older
    /// and no build is running. Requests made while a build runs coalesce: the next build
    /// starts from the design as it is when the running one finishes. Nothing starts while
    /// a canvas drag is in progress (the release changes the revision). Does nothing while
    /// the preview is off.
    ///
    /// # Arguments
    /// * `ctx` - Repainted while a build runs, so its result is picked up.
    /// * `state` - The design.
    /// * `flags` - The hidden shapes, which are not previewed.
    /// * `revision` - The design's revision ([`super::super::DesignerPane`] counts changes).
    /// * `dragging` - A canvas drag is in progress.
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        state: &DesignerState,
        flags: &ShapeFlags,
        revision: u64,
        dragging: bool,
    ) {
        if !state.show_toolpaths {
            self.running = None;
            self.built_for = None;
            return;
        }
        if let Some((key, receiver)) = &self.running {
            match receiver.try_recv() {
                Ok(toolpaths) => {
                    self.toolpaths = toolpaths;
                    self.built_for = Some(*key);
                    self.running = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    tracing::warn!("toolpath preview build stopped without a result");
                    self.running = None;
                }
            }
        }
        if self.running.is_none() && !dragging && self.built_for != Some(revision) {
            let (sender, receiver) = channel();
            let shapes = visible(state, flags);
            let settings = PreviewSettings::of(state);
            let spawned = std::thread::Builder::new()
                .name("designer-toolpath-preview".into())
                .spawn(move || {
                    // The receiver is gone when the preview was switched off meanwhile.
                    let _ = sender.send(build(&shapes, &settings));
                });
            match spawned {
                Ok(_) => {
                    self.running = Some((revision, receiver));
                    self.started += 1;
                }
                Err(error) => tracing::warn!(%error, "toolpath preview not built"),
            }
        }
        if self.running.is_some() {
            ctx.request_repaint();
        }
    }

    /// Paints the toolpaths while the preview is on.
    ///
    /// # Arguments
    /// * `painter` - Painter clipped to the canvas.
    /// * `view` - The view to paint with.
    /// * `state` - The design (whether the preview is on).
    /// * `palette` - Theme colours.
    pub fn paint(
        &self,
        painter: &Painter,
        view: &CanvasView,
        state: &DesignerState,
        palette: &crate::theme::Palette,
    ) {
        if !state.show_toolpaths {
            return;
        }
        let rapid = Stroke::new(2.0, palette.warning.gamma_multiply(0.5));
        let cut = Stroke::new(2.0, palette.success.gamma_multiply(0.7));
        for segment in self.toolpaths.iter().flat_map(|tp| tp.segments.iter()) {
            let (a, b) = (
                view.to_screen(segment.start.x, segment.start.y),
                view.to_screen(segment.end.x, segment.end.y),
            );
            match segment.segment_type {
                ToolpathSegmentType::RapidMove => {
                    painter.extend(EguiShape::dashed_line(&[a, b], rapid, 2.0, 2.0));
                }
                ToolpathSegmentType::LinearMove => {
                    painter.line_segment([a, b], cut);
                }
                ToolpathSegmentType::ArcCW | ToolpathSegmentType::ArcCCW => {
                    let points = match segment.center {
                        Some(c) => arc_points(
                            view,
                            (c.x, c.y),
                            (segment.start.x, segment.start.y),
                            (segment.end.x, segment.end.y),
                            segment.segment_type == ToolpathSegmentType::ArcCW,
                        ),
                        None => vec![a, b],
                    };
                    painter.add(EguiShape::line(points, cut));
                }
            }
        }
    }
}

/// Screen points along the arc from `start` to `end` about `center` (world mm), clockwise
/// or counter-clockwise; a full circle when the ends meet.
fn arc_points(
    view: &CanvasView,
    center: (f64, f64),
    start: (f64, f64),
    end: (f64, f64),
    clockwise: bool,
) -> Vec<Pos2> {
    use std::f64::consts::TAU;
    let radius = (start.0 - center.0).hypot(start.1 - center.1);
    let a0 = (start.1 - center.1).atan2(start.0 - center.0);
    let a1 = (end.1 - center.1).atan2(end.0 - center.0);
    let mut sweep = if clockwise { a0 - a1 } else { a1 - a0 };
    sweep = sweep.rem_euclid(TAU);
    if sweep <= 1e-9 {
        sweep = TAU;
    }
    let screen_len = radius * sweep * f64::from(view.zoom());
    let steps = (screen_len / 4.0).ceil().clamp(4.0, 256.0) as usize;
    let sign = if clockwise { -1.0 } else { 1.0 };
    (0..=steps)
        .map(|i| {
            let a = a0 + sign * sweep * i as f64 / steps as f64;
            view.to_screen(center.0 + radius * a.cos(), center.1 + radius * a.sin())
        })
        .collect()
}
