//! Finger shapes for the Springs, Barbs and Snap joint styles.
//!
//! The shapes follow the `style` choices of boxes.py's `FingerJointSettings`. Only the
//! fingers (the `f` edges) change; the notches they go into stay plain, so a styled finger
//! still fits the same slot.
//!
//! Every shape is returned in the finger's own frame: `u` runs along the edge from the
//! finger's left side (0) to its right side (`f`), `v` runs outward from the panel edge
//! (0 is the edge, `h` the finger tip, negative values are inside the panel). All sizes are
//! derived from the material thickness `t`, the finger width `f` and the finger length `h`.
//!
//! Dependencies: none.

/// Springs: the leaf next to each finger side is this fraction of the finger length wide.
const SPRING_LEAF: f32 = 0.3;
/// Springs: radius of the rounded leaf corner, as a fraction of the finger length.
const SPRING_RADIUS: f32 = 0.2;
/// Springs: the slit behind the leaf stops this fraction of the finger length short of
/// the panel edge, so the leaf stays attached at its root.
const SPRING_SLIT_STOP: f32 = 0.1;
/// Springs: slit width in thicknesses (boxes.py cuts a zero-width line; a small width keeps
/// the outline a simple polygon).
const SPRING_SLIT: f32 = 0.1;
/// Line segments used for the rounded leaf corner.
const SPRING_ARC_STEPS: usize = 4;

/// Barbs: distance between barbs along the finger side, in thicknesses.
pub(super) const BARB_PITCH: f32 = 0.3;
/// Barbs: how far each barb stands out of the finger side, in thicknesses.
pub(super) const BARB_DEPTH: f32 = 0.1;

/// Snap: depth of the relief slits into the panel, in thicknesses.
pub(super) const SNAP_RELIEF: f32 = 2.0;
/// Snap: width of the slit outside the arm, in thicknesses.
pub(super) const SNAP_OUTER_SLIT: f32 = 0.1;
/// Snap: how far the hook stands out of the arm, in thicknesses.
const SNAP_HOOK: f32 = 0.5;
/// Snap: length of the hook's lead-in ramp beyond the finger tip, in thicknesses.
const SNAP_RAMP: f32 = 1.0;
/// Snap: arm width at the tip, in thicknesses (it widens to twice this at its root).
const SNAP_ARM: f32 = 0.5;
/// Snap: distance from the hooked finger side to the far wall of the slit behind the arm,
/// in thicknesses.
pub(super) const SNAP_SLIT_WALL: f32 = 1.2;
/// Snap: the narrowest finger that takes a hook, in thicknesses (as boxes.py).
pub(super) const SNAP_MIN_FINGER: f32 = 1.9;

/// Mirrors a left-to-right shape about the finger's centre line, keeping the traversal
/// direction left to right.
fn mirrored(shape: &[(f32, f32)], f: f32) -> Vec<(f32, f32)> {
    shape.iter().rev().map(|&(u, v)| (f - u, v)).collect()
}

/// A finger whose two sides are flexible leaves: a slit runs from the tip down beside each
/// side, leaving a leaf attached only near the panel edge that springs against the slot.
///
/// # Arguments
/// * `f` - Finger width.
/// * `h` - Finger length.
/// * `t` - Material thickness.
///
/// # Returns
/// The outline from the left root `(0, 0)` to the right root `(f, 0)`.
pub(super) fn springs(f: f32, h: f32, t: f32) -> Vec<(f32, f32)> {
    let r = SPRING_RADIUS * h;
    let leaf = SPRING_LEAF * h;
    let slit = SPRING_SLIT * t;
    let corner_v = h - r;
    let mut left = vec![(0.0, 0.0), (0.0, corner_v)];
    // Quarter circle from the side (180 degrees) to the tip (90 degrees) about (r, corner_v).
    for step in 1..=SPRING_ARC_STEPS {
        let angle = std::f32::consts::PI * (1.0 - 0.5 * step as f32 / SPRING_ARC_STEPS as f32);
        left.push((r + r * angle.cos(), corner_v + r * angle.sin()));
    }
    left.extend([
        (leaf, h),
        (leaf, SPRING_SLIT_STOP * h),
        (leaf + slit, SPRING_SLIT_STOP * h),
        (leaf + slit, h),
    ]);
    let right = mirrored(&left, f);
    left.extend(right);
    left
}

/// The narrowest finger [`springs`] can draw: both leaves and slits plus some tip.
///
/// # Arguments
/// * `h` - Finger length.
/// * `t` - Material thickness.
pub(super) fn springs_min_finger(h: f32, t: f32) -> f32 {
    2.0 * (SPRING_LEAF * h + SPRING_SLIT * t)
}

/// A finger with saw-tooth barbs on both sides: each barb has a gentle ramp toward the tip,
/// so the finger slides in, and a steep face toward the root, so it grips.
///
/// # Arguments
/// * `f` - Finger width.
/// * `h` - Finger length.
/// * `t` - Material thickness.
///
/// # Returns
/// The outline from the left root `(0, 0)` to the right root `(f, 0)`.
pub(super) fn barbs(f: f32, h: f32, t: f32) -> Vec<(f32, f32)> {
    let pitch = BARB_PITCH * t;
    let depth = BARB_DEPTH * t;
    let n = barb_count(h, t);
    let straight = h - n as f32 * pitch;
    let mut left = vec![(0.0, 0.0), (0.0, straight)];
    for k in 0..n {
        let v0 = straight + k as f32 * pitch;
        left.push((-depth, v0 + depth));
        left.push((0.0, v0 + pitch));
    }
    let right = mirrored(&left, f);
    left.extend(right);
    left
}

/// Barbs per finger side (boxes.py: `(h - 0.1 t) // (0.3 t)`).
///
/// # Arguments
/// * `h` - Finger length.
/// * `t` - Material thickness.
pub(super) fn barb_count(h: f32, t: f32) -> usize {
    ((h - BARB_DEPTH * t) / (BARB_PITCH * t)).floor().max(0.0) as usize
}

/// A finger with a snap-fit hook on one side: relief slits free an arm that runs from
/// inside the panel to beyond the finger tip, where a hook catches behind the mating panel.
/// The arm flexes into the tapered slit behind it while the finger is pushed home.
///
/// # Arguments
/// * `f` - Finger width.
/// * `h` - Finger length (the hook catches at the tip).
/// * `t` - Material thickness.
/// * `hook_right` - Put the hook on the right side instead of the left.
///
/// # Returns
/// The outline from the left root to the right root; it starts (or ends) one outer slit
/// width outside the finger, on the panel edge.
pub(super) fn snap(f: f32, h: f32, t: f32, hook_right: bool) -> Vec<(f32, f32)> {
    let d = SNAP_RELIEF * t;
    let sw = SNAP_OUTER_SLIT * t;
    let top = h + SNAP_RAMP * t;
    let arm = SNAP_ARM * t;
    let wall = SNAP_SLIT_WALL * t;
    let left = vec![
        (-sw, 0.0),
        (-sw, -d),
        (0.0, -d),
        (0.0, h),
        (-SNAP_HOOK * t, h),
        (0.0, top),
        (arm, top),
        (2.0 * arm, -d),
        (wall, -d),
        (wall, top),
        (f - arm, top),
        (f, h),
        (f, 0.0),
    ];
    if hook_right { mirrored(&left, f) } else { left }
}
