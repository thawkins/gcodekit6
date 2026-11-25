//! Per-shape view flags: which shapes the layers panel has hidden or locked.
//!
//! The design model has no visibility or lock state, so the flags live beside it, in the
//! Designer pane. A hidden shape is neither painted nor hit-testable; a locked shape is
//! painted but cannot be picked (by a click or the marquee), so the canvas cannot move,
//! resize or delete it. A group is picked whole or not at all, so one hidden or locked member
//! makes the group unpickable (`picking`).
//!
//! The flags are view state: not saved with the design, not undoable, and they do not mark
//! it modified. The model knows nothing of them, so every G-code export must go through
//! `DesignerPane::generate_gcode`, which leaves hidden shapes out; calling the model's
//! `generate_gcode` directly would cut them.
//!
//! Dependencies: none beyond `std`.

use std::collections::BTreeSet;

/// The hidden and locked shape ids.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShapeFlags {
    hidden: BTreeSet<u64>,
    locked: BTreeSet<u64>,
}

impl ShapeFlags {
    /// Whether shape `id` is hidden.
    pub fn is_hidden(&self, id: u64) -> bool {
        self.hidden.contains(&id)
    }

    /// Whether shape `id` is locked.
    pub fn is_locked(&self, id: u64) -> bool {
        self.locked.contains(&id)
    }

    /// Whether the canvas may pick shape `id`: it is neither hidden nor locked.
    pub fn is_pickable(&self, id: u64) -> bool {
        !self.is_hidden(id) && !self.is_locked(id)
    }

    /// Hides or shows shape `id`.
    ///
    /// # Arguments
    /// * `id` - The shape.
    /// * `hidden` - Whether it is hidden from now on.
    pub fn set_hidden(&mut self, id: u64, hidden: bool) {
        if hidden {
            self.hidden.insert(id);
        } else {
            self.hidden.remove(&id);
        }
    }

    /// Locks or unlocks shape `id`.
    ///
    /// # Arguments
    /// * `id` - The shape.
    /// * `locked` - Whether it is locked from now on.
    pub fn set_locked(&mut self, id: u64, locked: bool) {
        if locked {
            self.locked.insert(id);
        } else {
            self.locked.remove(&id);
        }
    }

    /// Forgets every flag (a new design).
    pub fn clear(&mut self) {
        self.hidden.clear();
        self.locked.clear();
    }
}
