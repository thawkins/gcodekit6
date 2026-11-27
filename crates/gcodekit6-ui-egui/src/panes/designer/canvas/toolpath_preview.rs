//! The canvas's toolpath preview: the toolpaths the G-code is made from, drawn over the
//! shapes while the toolbox's "Show Toolpaths" is on.
//!
//! Each shape's toolpaths come
//! from the designer crate's CAM dispatch ([`CamSettings::shape_toolpaths`]), the same code
//! G-code export runs, so the effective parameters (defaults for shapes without custom
//! values, depths from the stock top, global laser feed and power) cannot drift from the
//! export. Hidden shapes are left out, as the export leaves them out.
//!
//! The toolpaths are built on a worker thread from a copy of the visible shapes, so a large
//! design never stalls the frame. The pane counts changes to the design (its revision); a
//! revision newer than the preview's starts a build, at most one at a time: changes made
//! while a build runs coalesce into the next build, which starts when the running one
//! finishes. No build starts during a canvas drag; the release starts one. A build that
//! panics clears the preview and is not retried until the design changes again.
//! Rapid moves are dashed in the warning colour at 50%; cuts and arcs are in the success
//! colour at 70%.
//!
//! While a build runs, the toolbox's View Controls show a spinner with the share of
//! shapes done and a Cancel button ([`ToolpathPreview::progress`], [`ToolpathPreview::cancel`]):
//! the worker stops at the next shape, the preview is cleared (no stale one) and no
//! build starts again until the design changes. The progress is not in the status
//! bar; it shows only beside Show Toolpaths.
//!
//! Dependencies: `egui`, `gcodekit6-designer` (CAM dispatch, toolpath generator).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use egui::{Painter, Pos2, Shape as EguiShape, Stroke};
use gcodekit6_designer::DesignerState;
use gcodekit6_designer::canvas::DrawingObject;
use gcodekit6_designer::designer_state::cam::CamSettings;
use gcodekit6_designer::toolpath::{Toolpath, ToolpathGenerator, ToolpathSegmentType};

use super::{CanvasView, ShapeFlags};

/// Builds the toolpaths of `shapes`.
///
/// # Returns
/// The toolpaths, or `None` when `cancel` was set before the last shape was done.
fn build(
    shapes: &[DrawingObject],
    settings: &CamSettings,
    cancel: &AtomicBool,
    done: &AtomicUsize,
) -> Option<Vec<Toolpath>> {
    // `shape_toolpaths` sets every generator parameter per shape.
    let mut generator = ToolpathGenerator::new();
    let mut toolpaths = Vec::new();
    for shape in shapes {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        toolpaths.extend(settings.shape_toolpaths(&mut generator, shape).toolpaths);
        done.fetch_add(1, Ordering::Relaxed);
    }
    Some(toolpaths)
}

/// A build on the worker thread.
#[derive(Debug)]
struct Running {
    /// The design revision it builds for.
    revision: u64,
    /// Its result; `None` when it was cancelled.
    receiver: Receiver<Option<Vec<Toolpath>>>,
    /// Set to stop it at the next shape.
    cancel: Arc<AtomicBool>,
    /// The shapes done so far.
    done: Arc<AtomicUsize>,
    /// The shapes it builds.
    total: usize,
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
    /// The build on the worker thread (at most one).
    running: Option<Running>,
    /// How many builds were started (for tests).
    started: u64,
    /// Test hook: builds panic while set.
    #[cfg(any(test, feature = "test-support"))]
    panic_builds: bool,
    /// Test hook: each build waits this long before its first shape.
    #[cfg(any(test, feature = "test-support"))]
    build_delay: Option<std::time::Duration>,
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

    /// The share of shapes the running build has done (0.0 to 1.0), or `None` when no build
    /// runs.
    pub fn progress(&self) -> Option<f32> {
        self.running.as_ref().map(|r| {
            let done = r.done.load(Ordering::Relaxed).min(r.total);
            done as f32 / r.total.max(1) as f32
        })
    }

    /// Cancel: stops the running build at its next shape and clears the preview; no
    /// build starts again until the design changes from `revision`. Does nothing when no
    /// build runs.
    ///
    /// # Arguments
    /// * `revision` - The design's current revision, which counts as previewed (empty).
    pub fn cancel(&mut self, revision: u64) {
        if let Some(running) = self.running.take() {
            running.cancel.store(true, Ordering::Relaxed);
            self.built_for = Some(revision);
            self.toolpaths.clear();
        }
    }

    /// Test hook: each build waits `delay` before its first shape, so a test sees it run.
    ///
    /// # Arguments
    /// * `delay` - The wait, or `None` for none.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_build_delay(&mut self, delay: Option<std::time::Duration>) {
        self.build_delay = delay;
    }

    /// Test hook: while `on`, every build panics on its worker thread.
    ///
    /// # Arguments
    /// * `on` - Whether builds panic.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_build_panics(&mut self, on: bool) {
        self.panic_builds = on;
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
            // The build's work is no longer wanted: stop it at its next shape.
            if let Some(running) = self.running.take() {
                running.cancel.store(true, Ordering::Relaxed);
            }
            self.built_for = None;
            return;
        }
        if let Some(Running {
            revision: key,
            receiver,
            ..
        }) = &self.running
        {
            match receiver.try_recv() {
                Ok(Some(toolpaths)) => {
                    self.toolpaths = toolpaths;
                    self.built_for = Some(*key);
                    self.running = None;
                }
                // Cancelled: `cancel` already took the build; nothing else cancels one.
                Ok(None) => self.running = None,
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    // The build panicked. Retrying the same design would panic again, every
                    // frame: mark this revision done (with nothing to show) and try again
                    // only once the design changes.
                    tracing::error!(
                        revision = *key,
                        "toolpath preview build stopped without a result"
                    );
                    self.toolpaths.clear();
                    self.built_for = Some(*key);
                    self.running = None;
                }
            }
        }
        if self.running.is_none() && !dragging && self.built_for != Some(revision) {
            let (sender, receiver) = channel();
            let shapes = visible(state, flags);
            let total = shapes.len();
            let settings = CamSettings::of(state);
            let cancel = Arc::new(AtomicBool::new(false));
            let done = Arc::new(AtomicUsize::new(0));
            let (worker_cancel, worker_done) = (Arc::clone(&cancel), Arc::clone(&done));
            #[cfg(any(test, feature = "test-support"))]
            let (panic_build, delay) = (self.panic_builds, self.build_delay);
            let spawned = std::thread::Builder::new()
                .name("designer-toolpath-preview".into())
                .spawn(move || {
                    #[cfg(any(test, feature = "test-support"))]
                    if panic_build {
                        panic!("toolpath preview build panic requested by test hook");
                    }
                    #[cfg(any(test, feature = "test-support"))]
                    if let Some(delay) = delay {
                        std::thread::sleep(delay);
                    }
                    // The receiver is gone when the preview was switched off or the build
                    // cancelled meanwhile.
                    let _ = sender.send(build(&shapes, &settings, &worker_cancel, &worker_done));
                });
            match spawned {
                Ok(_) => {
                    self.running = Some(Running {
                        revision,
                        receiver,
                        cancel,
                        done,
                        total,
                    });
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
