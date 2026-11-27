//! Picking: which shape a click or the marquee selects, skipping hidden and locked shapes.
//!
//! A shape is pickable when neither it nor any member of its group is hidden or locked: a
//! group is picked whole or not at all, so a drag never moves part of a group.
//!
//! Dependencies: `gcodekit5-designer` (design state, model).

use gcodekit5_designer::DesignerState;
use gcodekit5_designer::model::Point;

use super::ShapeFlags;

/// Whether shape `id`, and every other member of its group, may be picked.
fn pickable(state: &DesignerState, flags: &ShapeFlags, id: u64) -> bool {
    if !flags.is_pickable(id) {
        return false;
    }
    match state.canvas.get_shape(id).and_then(|o| o.group_id) {
        Some(gid) => state
            .canvas
            .shapes()
            .filter(|o| o.group_id == Some(gid))
            .all(|o| flags.is_pickable(o.id)),
        None => true,
    }
}

/// The topmost shape (in draw order) within `tolerance` of `point`.
///
/// Hidden and locked shapes are skipped.
///
/// # Arguments
/// * `state` - The design.
/// * `flags` - The hidden and locked shapes.
/// * `point` - A world point.
/// * `tolerance` - Hit distance in millimetres.
pub fn topmost_shape_at(
    state: &DesignerState,
    flags: &ShapeFlags,
    point: &Point,
    tolerance: f64,
) -> Option<u64> {
    let store = &state.canvas.shape_store;
    store.draw_order_iter().rev().find(|id| {
        pickable(state, flags, *id)
            && store
                .get(*id)
                .is_some_and(|obj| obj.contains_point(point, tolerance))
    })
}

/// Selects the shape (or whole group) under `point`, like the model's `select_at` but
/// skipping hidden and locked shapes. Plain click replaces the selection, Shift adds, and
/// Ctrl/Cmd toggles; a click on empty space deselects unless Shift or Ctrl/Cmd is held.
///
/// # Arguments
/// * `state` - The design.
/// * `flags` - The hidden and locked shapes.
/// * `point` - A world point.
/// * `tolerance` - Hit distance in millimetres.
/// * `shift` - Add to the selection.
/// * `ctrl` - Toggle the shape in the selection.
pub fn select_at(
    state: &mut DesignerState,
    flags: &ShapeFlags,
    point: &Point,
    tolerance: f64,
    shift: bool,
    ctrl: bool,
) {
    let hit = topmost_shape_at(state, flags, point, tolerance);
    if !shift && !ctrl {
        state.canvas.deselect_all();
    }
    let Some(id) = hit else {
        return;
    };
    let group = state.canvas.get_shape(id).and_then(|o| o.group_id);
    let ids: Vec<u64> = match group {
        Some(gid) => state
            .canvas
            .shapes()
            .filter(|o| o.group_id == Some(gid))
            .map(|o| o.id)
            .collect(),
        None => vec![id],
    };
    let select = if ctrl && !shift {
        !ids.iter()
            .all(|i| state.canvas.get_shape(*i).is_some_and(|o| o.selected))
    } else {
        true
    };
    for i in &ids {
        if let Some(obj) = state.canvas.get_shape_mut(*i) {
            obj.selected = select;
        }
    }
    let primary = if select {
        Some(id)
    } else {
        state.canvas.shapes().find(|o| o.selected).map(|o| o.id)
    };
    state.canvas.selection_manager.set_selected_id(primary);
}

/// Selects the shapes whose bounds touch the box from `a` to `b` (the marquee: it replaces
/// the selection, and leaves it empty when nothing is touched).
///
/// # Arguments
/// * `state` - The design.
/// * `flags` - The hidden and locked shapes, which the marquee skips.
/// * `a`, `b` - Opposite corners, world millimetres.
pub fn marquee_select(state: &mut DesignerState, flags: &ShapeFlags, a: (f64, f64), b: (f64, f64)) {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    let hits: Vec<u64> = state
        .canvas
        .shape_store
        .draw_order_iter()
        .filter(|id| {
            pickable(state, flags, *id)
                && state.canvas.get_shape(*id).is_some_and(|obj| {
                    let (sx0, sy0, sx1, sy1) = obj.get_total_bounds();
                    !(sx1 < x0 || sx0 > x1 || sy1 < y0 || sy0 > y1)
                })
        })
        .collect();
    if hits.is_empty() {
        return;
    }
    for obj in state.canvas.shape_store.iter_mut() {
        obj.selected = hits.contains(&obj.id);
    }
    state
        .canvas
        .selection_manager
        .set_selected_id(hits.first().copied());
}
