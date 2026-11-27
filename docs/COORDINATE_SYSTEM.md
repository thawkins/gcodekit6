# Coordinate System Documentation

How GCodeKit5 maps CNC coordinates to the screen.

## Conventions

| System | Units | Y direction | Used by |
|--------|-------|-------------|---------|
| **CNC / world** | millimetres | Up (away from the operator); Z up | G-code, machine commands, designer model |
| **egui screen** | points | Down; origin at the top-left of the window | Painting, pointer events |

Store and compute in world coordinates. Convert to screen coordinates only when painting or reading the pointer.

## Designer Canvas (2D)

The designer canvas converts with `CanvasView`
(`crates/gcodekit5-ui-egui/src/panes/designer/canvas/mod.rs`). `zoom` is points per millimetre;
`pan` is the offset in points from the canvas's bottom-left corner to world (0, 0), Y up. The
zoom and pan come from the designer model's `Viewport` (`crates/gcodekit5-designer/src/viewport.rs`).

```
screen.x = rect.left + world.x * zoom + pan.x
screen.y = rect.bottom - (world.y * zoom + pan.y)
```

The subtraction from `rect.bottom` is the Y-flip: world +Y draws upwards on screen.
`CanvasView::screen_to_world` is the inverse and maps a pointer position back to millimetres.

## Visualizer (3D)

The G-code visualizer uses a 3D orbit camera, `Camera`
(`crates/gcodekit5-visualizer/src/visualizer/camera.rs`), with Z up. It has a target, distance,
yaw and pitch, and preset views (top, front, side, isometric). There is no 2D Y-flip in the
visualizer; the camera's view and projection matrices map world coordinates to the screen.

## Related Documentation

- [ADR-002: Coordinate System](adr/ADR-002-coordinate-system.md) - Architecture decision record
- `crates/gcodekit5-visualizer/src/visualizer/viewport.rs` - `Bounds`, the toolpath bounding box
