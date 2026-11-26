//! The design canvas: shapes drawn on a millimetre grid with `egui::Painter`, edited with the
//! mouse and keyboard.
//!
//! [`CanvasView`] maps between screen points and world millimetres (world Y up, origin at the
//! bottom left). Every hit test runs in egui points, so a click selects the same shape at any
//! pixels-per-point. The zoom and pan are the model's [`Viewport`], with the canvas size in
//! points.
//!
//! [`DesignerTool`] is the current tool; the toolbox sets it. [`CanvasInput`] holds the drag, polyline and context-menu state between frames
//! ([`input`]); [`editing`] has the edits (shape creation, resize, rotate, snap) and
//! [`rendering`] the painting.
//!
//! [`ShapeFlags`] are the layers panel's hidden and locked shapes ([`flags`]). The Text tool's
//! Add Text dialog is [`text_tool`]; text shapes are painted as their glyph outlines
//! ([`text_outline`]). The context menu's "Array..." opens [`array_dialog`]; the toolbox's
//! "Show Toolpaths" draws [`toolpath_preview`].
//!
//! Dependencies: `egui`, `glam`, `gcodekit5-designer` (model, viewport, undo commands).

pub mod array_dialog;
pub mod editing;
pub mod flags;
pub mod input;
pub mod overlay;
pub mod picking;
pub mod rendering;
pub mod text_outline;
pub mod text_tool;
pub mod toolpath_preview;

use egui::{Pos2, Rect, pos2};
use gcodekit5_designer::viewport::Viewport;
use glam::Vec2;

pub use flags::ShapeFlags;
pub use input::CanvasInput;

/// The canvas widget's id. The canvas takes the keyboard focus when pressed, for its own
/// keys (Delete, Escape, Enter); the app's keymap treats that focus as the Designer pane
/// having the keyboard, unlike a text field's.
pub fn canvas_id() -> egui::Id {
    egui::Id::unique("gcodekit5-designer-canvas")
}

/// The tool the canvas's primary button uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DesignerTool {
    /// Select, move and resize shapes.
    #[default]
    Select,
    /// Drag to pan the view.
    Pan,
    /// Drag a rectangle.
    Rectangle,
    /// Drag the box a circle fits in.
    Circle,
    /// Drag a line.
    Line,
    /// Drag the box an ellipse fills.
    Ellipse,
    /// Drag the box of a right triangle.
    Triangle,
    /// Drag the box a hexagon fits in.
    Polygon,
    /// Drag the box a gear fits in.
    Gear,
    /// Drag the box a sprocket fits in.
    Sprocket,
    /// Click points; Enter finishes an open path, double-click a closed one.
    Polyline,
    /// Click to place text (the Add Text dialog asks for it).
    Text,
}

impl DesignerTool {
    /// Every tool, in tool-strip order.
    pub const ALL: [DesignerTool; 12] = [
        DesignerTool::Select,
        DesignerTool::Pan,
        DesignerTool::Rectangle,
        DesignerTool::Circle,
        DesignerTool::Line,
        DesignerTool::Ellipse,
        DesignerTool::Triangle,
        DesignerTool::Polygon,
        DesignerTool::Gear,
        DesignerTool::Sprocket,
        DesignerTool::Polyline,
        DesignerTool::Text,
    ];

    /// The app SVG (in `resources/icons/`) on the tool's toolbox button, as GTK.
    pub fn icon(self) -> &'static str {
        match self {
            DesignerTool::Select => "select.svg",
            DesignerTool::Pan => "grab.svg",
            DesignerTool::Rectangle => "rectangle.svg",
            DesignerTool::Circle => "circle.svg",
            DesignerTool::Line => "line.svg",
            DesignerTool::Ellipse => "ellipse.svg",
            DesignerTool::Triangle => "triangle.svg",
            DesignerTool::Polygon => "poligon.svg",
            DesignerTool::Gear => "gear.svg",
            DesignerTool::Sprocket => "sprocket.svg",
            DesignerTool::Polyline => "polyline.svg",
            DesignerTool::Text => "text.svg",
        }
    }

    /// The English msgid of the tool's name, as listed in `po/gcodekit5.pot`.
    pub fn label_msgid(self) -> &'static str {
        match self {
            DesignerTool::Select => n_!("Select"),
            DesignerTool::Pan => n_!("Pan"),
            DesignerTool::Rectangle => n_!("Rectangle"),
            DesignerTool::Circle => n_!("Circle"),
            DesignerTool::Line => n_!("Line"),
            DesignerTool::Ellipse => n_!("Ellipse"),
            DesignerTool::Triangle => n_!("Triangle"),
            DesignerTool::Polygon => n_!("Polygon"),
            DesignerTool::Gear => n_!("Gear"),
            DesignerTool::Sprocket => n_!("Sprocket"),
            DesignerTool::Polyline => n_!("Polyline"),
            DesignerTool::Text => n_!("Text"),
        }
    }

    /// Whether the tool creates a shape by dragging its box.
    pub fn drags_shape(self) -> bool {
        match self {
            DesignerTool::Rectangle
            | DesignerTool::Circle
            | DesignerTool::Line
            | DesignerTool::Ellipse
            | DesignerTool::Triangle
            | DesignerTool::Polygon
            | DesignerTool::Gear
            | DesignerTool::Sprocket => true,
            DesignerTool::Select
            | DesignerTool::Pan
            | DesignerTool::Polyline
            | DesignerTool::Text => false,
        }
    }
}

/// A corner handle of the selection box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeHandle {
    /// Minimum X, maximum Y.
    TopLeft,
    /// Maximum X, maximum Y.
    TopRight,
    /// Minimum X, minimum Y.
    BottomLeft,
    /// Maximum X, minimum Y.
    BottomRight,
}

/// How the canvas maps world millimetres to screen points on one frame.
///
/// `screen.x = rect.left + world.x * zoom + pan.x` and
/// `screen.y = rect.bottom - (world.y * zoom + pan.y)`: the model's [`Viewport`] formula with
/// the canvas rect's origin added.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasView {
    rect: Rect,
    zoom: f32,
    pan: Vec2,
}

impl Default for CanvasView {
    fn default() -> Self {
        Self::from_parts(Rect::NOTHING, 1.0, Vec2::ZERO)
    }
}

impl CanvasView {
    /// The view of `viewport` drawn into `rect`.
    ///
    /// # Arguments
    /// * `rect` - The canvas, in screen points.
    /// * `viewport` - The model's zoom (points per mm) and pan (points).
    pub fn new(rect: Rect, viewport: &Viewport) -> Self {
        Self::from_parts(
            rect,
            viewport.zoom() as f32,
            Vec2::new(viewport.pan_x() as f32, viewport.pan_y() as f32),
        )
    }

    /// A view from its parts.
    ///
    /// # Arguments
    /// * `rect` - The canvas, in screen points.
    /// * `zoom` - Points per millimetre.
    /// * `pan` - Points from the canvas's bottom-left corner to world (0, 0), Y up.
    pub fn from_parts(rect: Rect, zoom: f32, pan: Vec2) -> Self {
        Self { rect, zoom, pan }
    }

    /// The world point (mm) under screen point `p`.
    ///
    /// # Arguments
    /// * `p` - A screen point, in egui points.
    pub fn screen_to_world(&self, p: Pos2) -> Vec2 {
        Vec2::new(
            (p.x - self.rect.left() - self.pan.x) / self.zoom,
            (self.rect.bottom() - p.y - self.pan.y) / self.zoom,
        )
    }

    /// The screen point (egui points) of world point `p`.
    ///
    /// # Arguments
    /// * `p` - A world point, in millimetres.
    pub fn world_to_screen(&self, p: Vec2) -> Pos2 {
        pos2(
            self.rect.left() + p.x * self.zoom + self.pan.x,
            self.rect.bottom() - (p.y * self.zoom + self.pan.y),
        )
    }

    /// The screen point of world `(x, y)` mm given as model `f64`s.
    pub(crate) fn to_screen(self, x: f64, y: f64) -> Pos2 {
        self.world_to_screen(Vec2::new(x as f32, y as f32))
    }

    /// Points per millimetre.
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// The canvas, in screen points.
    pub fn rect(&self) -> Rect {
        self.rect
    }
}
