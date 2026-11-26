//! The visualizer's orbit camera: orbit, pan, zoom, fit to a box and the standard views.
//!
//! The camera keeps the GTK build's maths by wrapping the shared
//! [`gcodekit5_visualizer::Camera3D`] (Z up, yaw and pitch around a target): the drag and
//! wheel sensitivities and the standard view angles are those of the GTK
//! `ui/gtk/visualizer/mod.rs` and `ui/gtk/nav_cube.rs`. Like GTK, a standard view (within 5
//! degrees) is drawn with an orthographic projection and any other angle in perspective.
//!
//! Fitting differs from GTK on purpose: it fits the box's bounding sphere to the narrower of
//! the view's two sides, so a tall, narrow pane still shows the whole program.
//!
//! Dependencies: `glam`, `gcodekit5-visualizer` (camera maths).

use gcodekit5_visualizer::Camera3D;
use glam::{Mat4, Vec3};

/// Isometric pitch in degrees (`atan(1 / sqrt(2))`).
const ISO_PITCH: f32 = 35.264;

/// An axis-aligned box in millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    /// The corner with the smallest X, Y and Z.
    pub min: Vec3,
    /// The corner with the largest X, Y and Z.
    pub max: Vec3,
}

impl Aabb {
    /// The box spanning `min` to `max`.
    ///
    /// # Arguments
    /// * `min` - The smallest corner.
    /// * `max` - The largest corner.
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// The box's eight corners.
    pub fn corners(&self) -> [Vec3; 8] {
        let (a, b) = (self.min, self.max);
        [
            Vec3::new(a.x, a.y, a.z),
            Vec3::new(b.x, a.y, a.z),
            Vec3::new(a.x, b.y, a.z),
            Vec3::new(b.x, b.y, a.z),
            Vec3::new(a.x, a.y, b.z),
            Vec3::new(b.x, a.y, b.z),
            Vec3::new(a.x, b.y, b.z),
            Vec3::new(b.x, b.y, b.z),
        ]
    }
}

/// The views the nav cube and the View commands switch to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdView {
    /// Looking down the Z axis, +Y up the screen.
    Top,
    /// Looking up the Z axis.
    Bottom,
    /// Looking along +Y (from the front of the machine).
    Front,
    /// Looking along -X.
    Right,
    /// Looking along +X.
    Left,
    /// Isometric from the north-west corner.
    IsoNw,
    /// Isometric from the north-east corner.
    IsoNe,
    /// Isometric from the south-west corner.
    IsoSw,
    /// Isometric from the south-east corner (the default view).
    IsoSe,
}

impl StdView {
    /// Yaw and pitch in degrees, as the GTK nav cube sets them.
    pub fn angles(self) -> (f32, f32) {
        match self {
            StdView::Top => (-90.0, 90.0),
            StdView::Bottom => (-90.0, -90.0),
            StdView::Front => (-90.0, 0.0),
            StdView::Right => (0.0, 0.0),
            StdView::Left => (180.0, 0.0),
            StdView::IsoNw => (135.0, ISO_PITCH),
            StdView::IsoNe => (45.0, ISO_PITCH),
            StdView::IsoSw => (-135.0, ISO_PITCH),
            StdView::IsoSe => (-45.0, ISO_PITCH),
        }
    }
}

/// The visualizer's camera.
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    inner: Camera3D,
}

impl Default for Camera {
    fn default() -> Self {
        let mut camera = Self {
            inner: Camera3D::default(),
        };
        camera.view(StdView::IsoSe);
        camera
    }
}

impl Camera {
    /// The point the camera orbits around.
    pub fn target(&self) -> Vec3 {
        self.inner.target
    }

    /// Distance from the eye to the target in mm.
    pub fn distance(&self) -> f32 {
        self.inner.distance
    }

    /// The eye position.
    pub fn eye(&self) -> Vec3 {
        self.inner.get_eye_position()
    }

    /// Yaw and pitch in degrees.
    pub fn angles(&self) -> (f32, f32) {
        (self.inner.yaw.to_degrees(), self.inner.pitch.to_degrees())
    }

    /// Sets the view's width over height, used by [`Camera::fit`] and the projection.
    ///
    /// # Arguments
    /// * `aspect` - Width over height; ignored unless positive and finite.
    pub fn set_aspect(&mut self, aspect: f32) {
        if aspect.is_finite() && aspect > 0.0 {
            self.inner.aspect_ratio = aspect;
        }
    }

    /// Orbits around the target for a drag of `dx`, `dy` points, as GTK does: slower close
    /// up, faster far away.
    ///
    /// # Arguments
    /// * `dx` - Horizontal drag in points.
    /// * `dy` - Vertical drag in points.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        let orbit_scale = (self.inner.distance / 100.0).clamp(0.2, 5.0);
        let sensitivity = 0.005 * orbit_scale;
        self.inner.orbit(-dx * sensitivity, -dy * sensitivity);
    }

    /// Moves the target with a drag of `dx`, `dy` points.
    ///
    /// # Arguments
    /// * `dx` - Horizontal drag in points.
    /// * `dy` - Vertical drag in points.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.inner.pan(dx, dy);
    }

    /// Moves the eye `delta` mm toward the target (away when negative).
    ///
    /// # Arguments
    /// * `delta` - Millimetres to move closer.
    pub fn zoom(&mut self, delta: f32) {
        self.inner.zoom(delta);
    }

    /// Centres `bounds` and moves the eye back until the whole box is in view.
    ///
    /// # Arguments
    /// * `bounds` - The box to show.
    pub fn fit(&mut self, bounds: Aabb) {
        let radius = ((bounds.max - bounds.min).length() * 0.5).max(1.0) * 1.1;
        let half_fov = (self.inner.fov.to_radians() / 2.0).max(f32::EPSILON);
        let aspect = self.inner.aspect_ratio;
        // The narrower half-angle of the view: vertical, or horizontal in a narrow pane.
        let half_angle = if aspect < 1.0 {
            (half_fov.tan() * aspect).atan()
        } else {
            half_fov
        };
        self.inner.target = (bounds.min + bounds.max) * 0.5;
        self.inner.distance =
            (radius / half_angle.sin()).clamp(self.inner.min_distance, self.inner.max_distance);
    }

    /// Turns to a standard view, keeping the target and distance.
    ///
    /// # Arguments
    /// * `v` - The view.
    pub fn view(&mut self, v: StdView) {
        let (yaw, pitch) = v.angles();
        self.inner.set_view(yaw, pitch);
    }

    /// Turns to `yaw` and `pitch` degrees (as [`StdView::angles`] gives them), keeping the
    /// target and distance.
    ///
    /// # Arguments
    /// * `yaw` - Degrees around Z.
    /// * `pitch` - Degrees above the XY plane.
    pub fn look(&mut self, yaw: f32, pitch: f32) {
        self.inner.set_view(yaw, pitch);
    }

    /// Back to the default: isometric, 100 mm from the origin.
    pub fn reset(&mut self) {
        let aspect = self.inner.aspect_ratio;
        *self = Self::default();
        self.inner.aspect_ratio = aspect;
    }

    /// The view matrix (world to eye).
    pub fn view_matrix(&self) -> Mat4 {
        self.inner.get_view_matrix()
    }

    /// Projection times view for a view of `aspect` (width over height), with depth in
    /// `0..1` as wgpu expects.
    ///
    /// # Arguments
    /// * `aspect` - Width over height of the view.
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let mut camera = self.inner;
        if aspect.is_finite() && aspect > 0.0 {
            camera.aspect_ratio = aspect;
        }
        camera.get_projection_matrix() * camera.get_view_matrix()
    }
}
