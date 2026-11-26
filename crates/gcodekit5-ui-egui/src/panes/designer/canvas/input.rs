//! Mouse and keyboard input of the design canvas, as the GTK canvas handles it:
//!
//! - Select tool: a press selects the shape under the pointer (Shift adds, Ctrl/Cmd toggles,
//!   empty space deselects); a plain click on a shape of a multi-selection keeps only that
//!   one; a drag from a corner handle resizes the selection, any other drag with a selection
//!   moves it (Shift locks the axis), and a drag without one selects with a marquee.
//! - Drawing tools: a drag draws the shape in the dragged box (Shift squares a rectangle or
//!   ellipse and snaps a line to 45 degrees). Polyline: clicks add points, Enter finishes an
//!   open path, a double-click or right-click a closed one, Escape cancels. Text: a click
//!   opens the Add Text dialog for the snapped point.
//! - Pan tool or middle drag pans; the wheel pans; Ctrl/Cmd+wheel or a pinch zooms about the
//!   pointer.
//! - With the canvas focused (after a press in it): Delete/Backspace delete the selection,
//!   Escape cancels a polyline or deselects. A text field elsewhere owns the keyboard
//!   instead, so these never fire while typing.
//! - Right-click with a selection: GTK's context menu with Cut, Copy, Paste, Delete, Mirror
//!   on X / Y, Array... (the pane opens the Array dialog), Rotate, Align, Group / Ungroup
//!   (also Ctrl+G / Ctrl+Shift+G), the boolean operations (Union, Diff, Inter) and Convert
//!   to Path / Rectangle; with only a
//!   clipboard, Paste. Cut, Copy and Paste use the design's own clipboard
//!   (`DesignerState::clipboard`), as GTK did, not the system clipboard; Paste centres the
//!   copies on the point right-clicked. Align also has GTK's Alt keys,
//!   through the app's keymap (`crate::commands`) while the canvas has the keyboard.
//!
//! Drawing tools and moves snap to the snap step; selection hit tests use the raw pointer.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model, undo commands), `gcodekit5-app`
//! (translations).

use egui::{Modifiers, PointerButton, Pos2, Response};
use gcodekit5_app::i18n::Translator;
use gcodekit5_designer::DesignerState;
use gcodekit5_designer::commands::{DesignerCommand, MoveShapes};
use gcodekit5_designer::model::Point;

use super::editing::{self, ResizeDrag};
use super::flags::ShapeFlags;
use super::picking;
use super::{CanvasView, DesignerTool};

/// A primary-button drag in progress.
#[derive(Clone, Debug)]
pub(crate) enum Drag {
    /// Moving the selection.
    Move {
        /// The raw press point.
        start: (f64, f64),
        /// The snapped point the selection was last moved to.
        last: (f64, f64),
        /// The movement applied so far, recorded as one undo step at the end.
        applied: (f64, f64),
    },
    /// Resizing the selection by a handle.
    Resize(ResizeDrag),
    /// Drawing a shape's box.
    Create {
        /// The snapped press point.
        start: (f64, f64),
        /// The snapped, constrained pointer point.
        current: (f64, f64),
    },
    /// Selecting with a marquee.
    Marquee {
        /// The press point.
        start: (f64, f64),
        /// The pointer point.
        current: (f64, f64),
    },
    /// Panning with the Pan tool.
    Pan,
}

/// The canvas's input state between frames.
#[derive(Clone, Debug, Default)]
pub struct CanvasInput {
    pub(crate) drag: Option<Drag>,
    /// The points of the polyline being drawn.
    pub(crate) polyline: Vec<Point>,
    /// The world point under the pointer, if it is over the canvas.
    pub(crate) hover: Option<(f64, f64)>,
    /// Where a Text tool click asked for text (snapped), until the pane opens the dialog.
    pub(crate) text_click: Option<(f64, f64)>,
    /// The context menu's "Array..." was chosen, until the pane opens the dialog.
    pub(crate) array_click: bool,
    /// The world point of the last right-click: where the context menu's Paste puts the
    /// clipboard (GTK's pointer position).
    pub(crate) menu_at: (f64, f64),
}

/// The world point (mm) under `p`.
fn world(view: &CanvasView, p: Pos2) -> (f64, f64) {
    let w = view.screen_to_world(p);
    (f64::from(w.x), f64::from(w.y))
}

impl CanvasInput {
    /// The points of the polyline being drawn.
    pub fn polyline(&self) -> &[Point] {
        &self.polyline
    }

    /// The box being drawn or marquee-selected, if any, as two corners.
    pub(crate) fn preview_box(&self) -> Option<((f64, f64), (f64, f64))> {
        match &self.drag {
            Some(Drag::Create { start, current } | Drag::Marquee { start, current }) => {
                Some((*start, *current))
            }
            Some(Drag::Move { .. } | Drag::Resize(_) | Drag::Pan) | None => None,
        }
    }

    /// Whether a primary-button drag (move, resize, draw, marquee, pan) is in progress.
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Discards the polyline being drawn and any drag (the tool changed).
    pub(crate) fn reset(&mut self) {
        self.polyline.clear();
        self.drag = None;
        self.text_click = None;
    }

    /// Whether the context menu's "Array..." was chosen since the last call.
    pub(crate) fn take_array_click(&mut self) -> bool {
        std::mem::take(&mut self.array_click)
    }

    /// Takes the point a Text tool click placed, if any.
    pub(crate) fn take_text_click(&mut self) -> Option<(f64, f64)> {
        self.text_click.take()
    }

    /// Finishes the polyline being drawn (a path when it has two points or more).
    ///
    /// # Returns
    /// Whether a polyline was in progress.
    pub(crate) fn finish_polyline(&mut self, state: &mut DesignerState, closed: bool) -> bool {
        let points = std::mem::take(&mut self.polyline);
        editing::create_polyline(state, &points, closed);
        !points.is_empty()
    }

    /// Handles this frame's input on the canvas.
    ///
    /// # Arguments
    /// * `ui` - The canvas's `Ui`.
    /// * `response` - The canvas's response (click and drag sense).
    /// * `view` - The view the frame started with.
    /// * `state` - The design.
    /// * `flags` - The hidden and locked shapes, which clicks and the marquee skip.
    /// * `tool` - The current tool.
    /// * `tr` - Translator for the context menu.
    #[allow(clippy::too_many_arguments)] // the frame's whole input context, called once
    pub fn handle(
        &mut self,
        ui: &egui::Ui,
        response: &Response,
        view: &CanvasView,
        state: &mut DesignerState,
        flags: &ShapeFlags,
        tool: DesignerTool,
        tr: &Translator,
    ) {
        let (modifiers, any_pressed, primary_pressed, press_origin) = ui.input(|i| {
            (
                i.modifiers,
                i.pointer.any_pressed(),
                i.pointer.button_pressed(PointerButton::Primary),
                i.pointer.press_origin(),
            )
        });
        self.hover = response.hover_pos().map(|p| world(view, p));
        if any_pressed && response.contains_pointer() {
            response.request_focus();
        }
        if response.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter {
                        escape: true,
                        ..Default::default()
                    },
                );
            });
            self.keys(ui, state, tool);
        }
        if primary_pressed
            && response.contains_pointer()
            && let Some(origin) = press_origin
            && tool == DesignerTool::Select
        {
            press_select(state, flags, view, world(view, origin), modifiers);
        }
        self.primary_drag(ui, response, view, state, flags, tool, modifiers);
        if response.dragged_by(PointerButton::Middle) {
            let d = response.drag_delta();
            state.canvas.pan_by(f64::from(d.x), -f64::from(d.y));
        }
        if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            let double = response.double_clicked();
            self.click(state, flags, tool, world(view, p), double, modifiers);
        }
        self.wheel(ui, response, view, state);
        if response.secondary_clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.menu_at = world(view, p);
        }
        let finished = response.secondary_clicked()
            && tool == DesignerTool::Polyline
            && self.finish_polyline(state, true);
        // GTK: a menu for a selection, or for Paste while the clipboard holds shapes.
        if !finished && (state.selected_count() > 0 || !state.clipboard.is_empty()) {
            let at = self.menu_at;
            response.context_menu(|ui| {
                self.array_click |= context_menu(ui, state, tr, at);
            });
        }
    }

    /// Delete, Escape and Enter while the canvas has the keyboard.
    fn keys(&mut self, ui: &egui::Ui, state: &mut DesignerState, tool: DesignerTool) {
        let (delete, escape, enter) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::Enter),
            )
        });
        // Not mid-drag: the drag still holds the shapes it started from.
        if delete && self.drag.is_none() && state.selected_count() > 0 {
            state.delete_selected();
        }
        if escape && self.polyline.is_empty() {
            state.deselect_all();
        } else if escape {
            self.polyline.clear();
        }
        if enter && tool == DesignerTool::Polyline {
            self.finish_polyline(state, false);
        }
    }

    /// A click (press and release without a drag).
    fn click(
        &mut self,
        state: &mut DesignerState,
        flags: &ShapeFlags,
        tool: DesignerTool,
        at: (f64, f64),
        double: bool,
        modifiers: Modifiers,
    ) {
        match tool {
            DesignerTool::Polyline if double => {
                self.finish_polyline(state, true);
            }
            DesignerTool::Polyline => {
                let (x, y) = editing::snap(state, at.0, at.1);
                // A click on the last point adds no zero-length segment.
                if self.polyline.last() != Some(&Point::new(x, y)) {
                    self.polyline.push(Point::new(x, y));
                }
            }
            // GTK: a click places text at the snapped point, through the Add Text dialog.
            DesignerTool::Text => {
                self.text_click = Some(editing::snap(state, at.0, at.1));
            }
            DesignerTool::Select if !modifiers.shift && !modifiers.command => {
                // A plain click on a selected shape keeps only that shape selected.
                let tolerance = editing::hit_tolerance_mm(state.canvas.zoom());
                let point = Point::new(at.0, at.1);
                if let Some(id) = picking::topmost_shape_at(state, flags, &point, tolerance)
                    && state.canvas.get_shape(id).is_some_and(|o| o.selected)
                {
                    state.canvas.deselect_all();
                    state.canvas.select_shape(id, false);
                }
            }
            _ => {}
        }
    }

    /// Starts, continues and ends a primary-button drag.
    #[allow(clippy::too_many_arguments)] // the frame's whole input context, called once
    fn primary_drag(
        &mut self,
        ui: &egui::Ui,
        response: &Response,
        view: &CanvasView,
        state: &mut DesignerState,
        flags: &ShapeFlags,
        tool: DesignerTool,
        modifiers: Modifiers,
    ) {
        if response.drag_started_by(PointerButton::Primary)
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        {
            self.drag = begin_drag(state, tool, world(view, origin), view.zoom());
        }
        if response.dragged_by(PointerButton::Primary)
            && let Some(p) = response.interact_pointer_pos()
        {
            let at = world(view, p);
            match &mut self.drag {
                Some(Drag::Move {
                    start,
                    last,
                    applied,
                }) => {
                    let constrained = axis_lock(*start, at, modifiers.shift);
                    let to = editing::snap(state, constrained.0, constrained.1);
                    let (dx, dy) = (to.0 - last.0, to.1 - last.1);
                    if dx != 0.0 || dy != 0.0 {
                        state.canvas.move_selected(dx, dy);
                        *last = to;
                        applied.0 += dx;
                        applied.1 += dy;
                    }
                }
                Some(Drag::Resize(resize)) => {
                    let current = editing::snap(state, at.0, at.1);
                    resize.apply(state, current, modifiers.shift);
                }
                Some(Drag::Create { start, current }) => {
                    let constrained = constrain(tool, *start, at, modifiers.shift);
                    *current = editing::snap(state, constrained.0, constrained.1);
                }
                Some(Drag::Marquee { current, .. }) => *current = at,
                Some(Drag::Pan) => {
                    let d = response.drag_delta();
                    state.canvas.pan_by(f64::from(d.x), -f64::from(d.y));
                }
                None => {}
            }
        }
        if response.drag_stopped_by(PointerButton::Primary) {
            match self.drag.take() {
                Some(Drag::Move { applied, .. }) => record_move(state, applied),
                Some(Drag::Resize(resize)) => resize.commit(state),
                Some(Drag::Create { start, current }) => {
                    editing::create_from_drag(state, tool, start, current);
                }
                Some(Drag::Marquee { start, current }) => {
                    if state.canvas.selection_manager.selected_id().is_none() {
                        picking::marquee_select(state, flags, start, current);
                    }
                }
                Some(Drag::Pan) | None => {}
            }
        }
    }

    /// Wheel pans; Ctrl/Cmd+wheel and pinch zoom about the pointer.
    fn wheel(
        &mut self,
        ui: &egui::Ui,
        response: &Response,
        view: &CanvasView,
        state: &mut DesignerState,
    ) {
        let Some(p) = response.hover_pos() else {
            return;
        };
        let (zoom, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta()));
        if (zoom - 1.0).abs() > f32::EPSILON {
            editing::zoom_at(state, view, p, f64::from(zoom));
        }
        if scroll != egui::Vec2::ZERO {
            state
                .canvas
                .pan_by(f64::from(scroll.x), -f64::from(scroll.y));
        }
    }
}

/// GTK's press with the Select tool: on a resize handle, or on an already selected shape
/// without modifiers, the selection waits for the drag or release; otherwise the shape under
/// the pointer is selected (Shift adds, Ctrl/Cmd toggles) and empty space deselects.
fn press_select(
    state: &mut DesignerState,
    flags: &ShapeFlags,
    view: &CanvasView,
    at: (f64, f64),
    modifiers: Modifiers,
) {
    let zoom = f64::from(view.zoom());
    if let Some(bounds) = editing::selection_box(state)
        && editing::handle_at(at.0, at.1, bounds, zoom).is_some()
    {
        return;
    }
    let (shift, ctrl) = (modifiers.shift, modifiers.command);
    let tolerance = editing::hit_tolerance_mm(zoom);
    let point = Point::new(at.0, at.1);
    if let Some(id) = picking::topmost_shape_at(state, flags, &point, tolerance)
        && state.canvas.get_shape(id).is_some_and(|o| o.selected)
        && !shift
        && !ctrl
    {
        return;
    }
    picking::select_at(state, flags, &point, tolerance, shift, ctrl);
}

/// The drag a primary press at `at` starts with `tool`.
fn begin_drag(
    state: &DesignerState,
    tool: DesignerTool,
    at: (f64, f64),
    zoom: f32,
) -> Option<Drag> {
    match tool {
        DesignerTool::Select => {
            if let Some(bounds) = editing::selection_box(state)
                && let Some(handle) = editing::handle_at(at.0, at.1, bounds, f64::from(zoom))
            {
                return Some(Drag::Resize(ResizeDrag::new(state, handle, bounds)));
            }
            if state.canvas.selection_manager.selected_id().is_some() {
                Some(Drag::Move {
                    start: at,
                    last: editing::snap(state, at.0, at.1),
                    applied: (0.0, 0.0),
                })
            } else {
                Some(Drag::Marquee {
                    start: at,
                    current: at,
                })
            }
        }
        DesignerTool::Pan => Some(Drag::Pan),
        DesignerTool::Polyline | DesignerTool::Text => None,
        DesignerTool::Rectangle
        | DesignerTool::Circle
        | DesignerTool::Line
        | DesignerTool::Ellipse
        | DesignerTool::Triangle
        | DesignerTool::Polygon
        | DesignerTool::Gear
        | DesignerTool::Sprocket => {
            let start = editing::snap(state, at.0, at.1);
            Some(Drag::Create {
                start,
                current: start,
            })
        }
    }
}

/// With `shift`, keeps only the larger axis of the move from `start` to `at`.
fn axis_lock(start: (f64, f64), at: (f64, f64), shift: bool) -> (f64, f64) {
    if !shift {
        at
    } else if (at.0 - start.0).abs() > (at.1 - start.1).abs() {
        (at.0, start.1)
    } else {
        (start.0, at.1)
    }
}

/// With `shift`, squares a rectangle or ellipse box and snaps a line to 45 degrees.
fn constrain(tool: DesignerTool, start: (f64, f64), at: (f64, f64), shift: bool) -> (f64, f64) {
    let (dx, dy) = (at.0 - start.0, at.1 - start.1);
    match tool {
        DesignerTool::Rectangle | DesignerTool::Ellipse if shift => {
            let side = dx.abs().max(dy.abs());
            (start.0 + side * dx.signum(), start.1 + side * dy.signum())
        }
        DesignerTool::Line if shift => {
            let step = std::f64::consts::FRAC_PI_4;
            let angle = (dy.atan2(dx) / step).round() * step;
            let length = dx.hypot(dy);
            (
                start.0 + length * angle.cos(),
                start.1 + length * angle.sin(),
            )
        }
        _ => at,
    }
}

/// Records a finished move of the selection by `applied` as one undo step.
fn record_move(state: &mut DesignerState, applied: (f64, f64)) {
    if applied.0.abs() <= 0.01 && applied.1.abs() <= 0.01 {
        return;
    }
    let ids: Vec<u64> = state
        .canvas
        .shapes()
        .filter(|s| s.selected)
        .map(|s| s.id)
        .collect();
    if !ids.is_empty() {
        state.record_command(DesignerCommand::MoveShapes(MoveShapes {
            ids,
            dx: applied.0,
            dy: applied.1,
        }));
    }
}

/// The rotations of the context menu: label msgid and degrees (counter-clockwise positive).
const ROTATIONS: [(&str, f64); 5] = [
    (n_!("90° CW"), -90.0),
    (n_!("90° CCW"), 90.0),
    (n_!("45° CW"), -45.0),
    (n_!("45° CCW"), 45.0),
    (n_!("180°"), 180.0),
];

/// A designer model operation on the selection.
type SelectionOp = fn(&mut DesignerState);

/// The Align submenu: label msgid and the model's operation, in GTK's order.
const ALIGNMENTS: [(&str, SelectionOp); 6] = [
    (
        n_!("Align Left"),
        DesignerState::align_selected_horizontal_left,
    ),
    (
        n_!("Align Right"),
        DesignerState::align_selected_horizontal_right,
    ),
    (n_!("Align Top"), DesignerState::align_selected_vertical_top),
    (
        n_!("Align Bottom"),
        DesignerState::align_selected_vertical_bottom,
    ),
    (
        n_!("Align Center X"),
        DesignerState::align_selected_horizontal_center,
    ),
    (
        n_!("Align Center Y"),
        DesignerState::align_selected_vertical_center,
    ),
];

/// The boolean operations: GTK's short labels and the model's operation. Each replaces the
/// selected shapes by one new shape (the operation's result), as one undo step.
const BOOLEANS: [(&str, SelectionOp); 3] = [
    (n_!("Union"), DesignerState::perform_boolean_union),
    (n_!("Diff"), DesignerState::perform_boolean_difference),
    (n_!("Inter"), DesignerState::perform_boolean_intersection),
];

/// GTK's conversions: each replaces the selected shapes by one new shape, a path through
/// all their outlines or the rectangle of their bounds, as one undo step.
const CONVERSIONS: [(&str, SelectionOp); 2] = [
    (
        n_!("Convert to Path"),
        DesignerState::convert_selected_to_path,
    ),
    (
        n_!("Convert to Rectangle"),
        DesignerState::convert_selected_to_rectangle,
    ),
];

/// The msgids of the context menu.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        n_!("Cut"),
        n_!("Copy"),
        n_!("Paste"),
        n_!("Delete"),
        n_!("Group"),
        n_!("Ungroup"),
        n_!("Mirror on X"),
        n_!("Mirror on Y"),
        n_!("Rotate"),
        n_!("Array..."),
        n_!("Align"),
    ];
    ids.extend(ROTATIONS.iter().map(|(label, _)| *label));
    ids.extend(ALIGNMENTS.iter().map(|(label, _)| *label));
    ids.extend(BOOLEANS.iter().map(|(label, _)| *label));
    ids.extend(CONVERSIONS.iter().map(|(label, _)| *label));
    ids
}

/// The right-click menu, in GTK's order: Cut, Copy, Paste (while the clipboard holds
/// shapes), Delete, Group and Ungroup (when the selection can be), for two shapes or more
/// Union, Diff and Inter, Mirror on X and Y, Array..., Rotate by a fixed angle, for two
/// shapes or more Align, then Convert to Path and Convert to Rectangle. Without a selection
/// only Paste is offered. Each edit is one undo step.
///
/// # Arguments
/// * `at` - The world point right-clicked, where Paste centres the clipboard.
///
/// # Returns
/// Whether "Array..." was chosen.
fn context_menu(
    ui: &mut egui::Ui,
    state: &mut DesignerState,
    tr: &Translator,
    at: (f64, f64),
) -> bool {
    let selection = state.selected_count() > 0;
    if selection && ui.button(tr.tr("Cut")).clicked() {
        editing::cut_selected(state);
        ui.close();
    }
    if selection && ui.button(tr.tr("Copy")).clicked() {
        state.copy_selected();
        ui.close();
    }
    if !state.clipboard.is_empty() && ui.button(tr.tr("Paste")).clicked() {
        state.paste_at_location(at.0, at.1);
        ui.close();
    }
    if !selection {
        return false;
    }
    if ui.button(tr.tr("Delete")).clicked() {
        state.delete_selected();
        ui.close();
    }
    ui.separator();
    let can_group = state.can_group();
    let can_ungroup = state.can_ungroup();
    if can_group && ui.button(tr.tr("Group")).clicked() {
        state.group_selected();
        ui.close();
    }
    if can_ungroup && ui.button(tr.tr("Ungroup")).clicked() {
        state.ungroup_selected();
        ui.close();
    }
    if can_group || can_ungroup {
        ui.separator();
    }
    // GTK offers the boolean operations for two shapes or more.
    if state.can_perform_boolean_op() {
        for (label, op) in BOOLEANS {
            if ui.button(tr.tr(label)).clicked() {
                op(state);
                ui.close();
            }
        }
        ui.separator();
    }
    if ui.button(tr.tr("Mirror on X")).clicked() {
        state.mirror_selected_x();
        ui.close();
    }
    if ui.button(tr.tr("Mirror on Y")).clicked() {
        state.mirror_selected_y();
        ui.close();
    }
    let array = ui.button(tr.tr("Array...")).clicked();
    if array {
        ui.close();
    }
    ui.menu_button(tr.tr("Rotate"), |ui| {
        for (label, degrees) in ROTATIONS {
            if ui.button(tr.tr(label)).clicked() {
                editing::rotate_selected_by(state, degrees);
                ui.close();
            }
        }
    });
    // GTK offers Align for two shapes or more.
    if state.selected_count() >= 2 {
        ui.menu_button(tr.tr("Align"), |ui| {
            for (label, align) in ALIGNMENTS {
                if ui.button(tr.tr(label)).clicked() {
                    align(state);
                    ui.close();
                }
            }
        });
    }
    ui.separator();
    for (label, convert) in CONVERSIONS {
        if ui.button(tr.tr(label)).clicked() {
            convert(state);
            ui.close();
        }
    }
    array
}
