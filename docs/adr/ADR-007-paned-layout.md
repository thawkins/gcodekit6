# ADR-007: Paned Layout for Resizable Panels

## Status
Superseded by ADR-012

Amended 2026-11-28: the GTK layout manager was removed with the GTK front end. Pages in `crates/gcodekit5-ui-egui/src/workspace.rs` now use resizable egui side and bottom panels. The Decision section records both.

## Date
2026-02-18

## Context
GCodeKit5 has many UI panels (Connection, DRO, Jog, Editor, Console, Visualizer, Overrides, Macros, Settings) that need to be visible simultaneously on varying screen sizes. Users operate on screens from 1080p at 125% scaling to 4K monitors, and need the ability to resize panels to prioritize different workflows (e.g., more space for the visualizer during preview, more space for the editor during G-code editing).

Early approaches used fixed layouts, which wasted space on large screens and cramped content on smaller ones.

## Decision
Original (GTK, removed): a central layout manager tracked each panel's visibility, region (left, center, right, bottom), dock state and size, drew the dividers with GTK `Paned` widgets, saved the state between sessions, and offered predefined layouts.

Current (egui): each page in `crates/gcodekit5-ui-egui/src/workspace.rs` lays out its panes with `egui::Panel::right(...)` and `egui::Panel::bottom(...)`, both `.resizable(true)`, with a default size and, where needed, a size range. Which hideable panels are shown is saved in the settings' `panel_visibility` map. There are no predefined layouts and panels do not move between regions.

## Consequences

**Positive:**
- Users can resize panels to match their workflow and screen size
- The hideable panels' visibility is kept between sessions
- Toolkit panels avoid custom layout engine complexity

**Negative:**
- Panel minimum sizes must be carefully managed to prevent unusable states
- More complex state management than fixed layouts
- Layout serialization adds a potential point of failure on startup

## Alternatives Considered
- **Fixed CSS Grid layout**: Simpler but inflexible for different screen sizes
- **Floating/detachable windows**: More flexible but poor UX with tiling window managers
- **Tab-based panels**: Hides panels, requiring clicks to switch — bad for monitoring workflows
