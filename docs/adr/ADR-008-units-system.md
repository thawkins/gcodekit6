# ADR-008: Internal Units System

## Status
Accepted (amended)

Amended 2026-11-28: dimensions are `f64` mm (owner decision OD-2); the `gcodekit6_core::units` format, parse and feed-rate helpers take and return `f64` as well (audit item C-12); `parse_length` returns `Result<f64, String>`. The UI reads the unit setting each frame instead of listening for changes. This ADR is the canonical statement of the units rules.

## Date
2026-02-18

## Context
CNC machines operate with both metric (mm) and imperial (inch) measurement systems. G-code files may use either system (G21 for metric, G20 for imperial). Feed rates can be expressed in mm/min, mm/sec, in/min, or in/sec. Users expect the UI to display values in their preferred system while the application must perform consistent internal calculations.

Handling dual units throughout the codebase would double the testing surface and create conversion bugs at every calculation boundary.

## Decision
All internal calculations and storage use **millimeters (mm)** for dimensions and **mm/min** for feed rates. Conversion to/from the user's preferred display units happens exclusively at the UI boundary layer.

The `gcodekit6_core::units` module provides the conversion API:

```rust
pub enum MeasurementSystem { Metric, Imperial }
pub enum FeedRateUnits { MmPerMin, MmPerSec, InPerMin, InPerSec }

pub fn format_length(value_mm: f64, system: MeasurementSystem) -> String;
pub fn format_feed_rate(value_mm_per_min: f64, units: FeedRateUnits) -> String;
pub fn parse_length(input: &str, system: MeasurementSystem) -> Result<f64, String>;
```

Key rules:
- Machine positions and designer geometry are `f64` mm (`CNCPoint` in `crates/gcodekit6-core/src/data/mod.rs`, `Point` in `crates/gcodekit6-designer/src/model/mod.rs`); the `gcodekit6_core::units` display, parse and feed-rate conversion helpers take and return `f64` too (`format_length(value_mm: f64, ...)`)
- CAM tool parameters (Tabbed Box, bitmap engraving, Gerber, Excellon offsets, the CAM preset lengths), the designer stock (`StockMaterial`, the `.gckd` stock fields) and the Machine Control jog step, feed and travel are `f64` mm as well. `f32` remains only where the consumer is `f32`: egui and visualizer screen geometry, lyon paths (the Gerber rubout hatch), image and pixel maths, laser power values, and the controller's jog line and machine-reported positions (`MM_PER_INCH_F32`)
- `format_length` and `format_feed_rate` show 3 decimal places in either unit (`25.400` mm, `1.000` in); the Machine Control DRO and status bar use their own `format_length` (`crates/gcodekit6-ui-egui/src/panes/machine_control/dro.rs`), which shows mm to 2 decimal places and inches to 3
- DateTime values are stored in UTC, converted to locale in the UI layer
- Text strings are internally UTF-8
- The UI has no change listener: each frame it reads `config().ui.measurement_system` and `feed_rate_units` from the settings and formats values with them (for example the status bar in `crates/gcodekit6-ui-egui/src/app.rs`), so a change shows on the next frame
- G-code parser preserves the source unit mode but the visualizer works in mm

## Consequences

**Positive:**
- Single representation eliminates unit mismatch bugs in calculations
- Conversion logic is centralized and testable
- New features don't need to handle unit variants
- One place sets display precision for each kind of value (the `units` helpers, the DRO formatter)

**Negative:**
- Rounding errors accumulate during metric↔imperial roundtrips
- Must remember to convert at UI boundaries — forgetting causes display bugs
- G-code generation must re-apply the correct unit prefix (G20/G21)

## Alternatives Considered
- **Store in user's preferred units**: Simpler display but every calculation needs unit awareness
- **Use f32 for geometry**: Less memory, but not what the code does. Owner decision OD-2 (2026-11-28) records `f64` mm for positions, designer geometry and the `units` helpers
- **Unit-typed wrappers** (e.g., `Millimeters(f32)`): Type-safe but adds verbosity to every arithmetic operation
