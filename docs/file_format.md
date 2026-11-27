# GCodeKit5 Design File Format (.gckd)

The designer saves designs as JSON files with the `.gckd` extension. Open also accepts `.gck5`
files. The format stores shapes, viewport, toolpath parameters and metadata.

The full schema is the `DesignFile` and `ShapeData` structs in
`crates/gcodekit5-designer/src/serialization.rs`. This page covers the stable core and the
defaults. Lengths are millimetres (`f64` in the shape data).

## File Structure

```json
{
  "version": "1.1",
  "design_mode": "2d",
  "metadata": {
    "name": "Design Name",
    "created": "2026-01-30T10:00:00Z",
    "modified": "2026-01-30T12:00:00Z",
    "author": "",
    "description": ""
  },
  "viewport": { "zoom": 1.0, "pan_x": 0.0, "pan_y": 0.0 },
  "shapes": [],
  "default_properties": null,
  "toolpath_params": {}
}
```

| Field | Description |
|-------|-------------|
| `version` | Format version written by the app: `"1.1"` |
| `design_mode` | `"2d"` (default) or `"3d"` |
| `metadata` | `name`, `created` and `modified` (UTC, ISO 8601); optional `author`, `description` |
| `viewport` | `zoom` and pan offset `pan_x`, `pan_y` |
| `shapes` | Array of `ShapeData` objects |
| `default_properties` | Optional `ShapeData` used as the template for new shapes |
| `toolpath_params` | Global toolpath settings (below) |

## Shapes

Every shape has `id`, `shape_type`, `x`, `y`, `width`, `height` and `selected`, plus optional
`name`, `rotation` (degrees) and `group_id`. Operation fields include `operation_type`
(`"profile"` or `"pocket"`), `pocket_depth`, `start_depth`, `step_down`, `step_in` and
`pocket_strategy`.

### Shape Types

Written by the app: `rectangle`, `circle`, `line`, `ellipse`, `path`, `text`, `triangle`,
`polygon`, `gear`, `sprocket`, `raster_image`. The reader also accepts the legacy `polyline` type.

### Pocket Strategies

`pocket_strategy` is one of:

| Value | Description |
|-------|-------------|
| `"ContourParallel"` | Offset contours (default) |
| `{"Raster": {"angle": <deg>, "bidirectional": <bool>}}` | Raster (zig-zag) lines at an angle |
| `"Adaptive"` | Adaptive clearing |

### Shape Defaults

| Field | Default |
|-------|---------|
| `raster_fill_ratio` | 0.5 |
| `lock_aspect_ratio` | false |

Other missing shape fields take their type's zero value or the default named in `ShapeData`.

## Toolpath Parameters

| Field | Default |
|-------|---------|
| `feed_rate` | 1000.0 (mm/min) |
| `spindle_speed` | 3000.0 (RPM) |
| `tool_diameter` | 3.175 |
| `cut_depth` | -5.0 |
| `step_down` | 1.0 |
| `stock_width` | 200.0 |
| `stock_height` | 200.0 |
| `stock_thickness` | 10.0 |
| `safe_z_height` | stock thickness + 5.0 (15.0) |

## Loading and Compatibility

- Missing fields use their serde defaults; unknown fields are ignored.
- Invalid JSON or a file I/O failure returns an error (`anyhow::Result` with context).
- An unknown `shape_type` is an error in the shape converter (`DesignFile::to_drawing_object`).
  The app's load path (`DesignerState::load_from_file_report`) skips such a shape and lists it in
  the `LoadReport`; a later save writes the file without it.

## Related Files

- `crates/gcodekit5-designer/src/serialization.rs` - Format and conversion
- `crates/gcodekit5-designer/src/designer_state/file_io.rs` - Save and load on `DesignerState`
- `crates/gcodekit5-designer/tests/test_design_file_round_trip.rs` - Round-trip tests
