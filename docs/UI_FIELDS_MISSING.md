# UI Fields Missing for Feeds and Speeds Calculator

## Tools Manager - Missing Fields to Add

### ToolCuttingParams (on the Geometry tab or a new Cutting Params tab)
- `min_doc: f32` - Minimum depth of cut (mm)
- `max_doc_percent: f32` - Max depth as % of diameter

### ToolGeometry (NEW TAB: "Advanced Geometry")
- `helix_angle: Option<f32>` - Helix angle (degrees)
- `core_diameter: Option<f32>` - Core diameter (mm)
- `rake_angle: Option<f32>` - Rake angle (degrees)
- `relief_angle: Option<f32>` - Relief/clearance angle (degrees)

### ToolLimits (NEW TAB: "Limits & Features")
- `max_rpm_rating: Option<u32>` - Max RPM rating
- `stick_out_max: Option<f32>` - Max stick-out length (mm)
- `max_chip_load: Option<f32>` - Max chip load (mm/tooth)
- `chip_breaker: bool` - Has chip breaker
- `through_coolant: bool` - Supports through-coolant

## Materials Manager - Missing Fields to Add

### Physical Properties (on the Properties tab)
- `brinell_hardness: Option<f32>` - Brinell hardness (HB)
- `rockwell_hardness: Option<f32>` - Rockwell hardness C (HRC)
- `thermal_conductivity: Option<f32>` - Thermal conductivity (W/(m·K))
- `specific_cutting_force: Option<f32>` - Kc value (N/mm²)

### Cutting Parameters (NEW TAB: "Cutting Parameters" - per tool type)
- `sfm_range: Option<(f32, f32)>` - SFM range (min, max)
- `chipload_range: Option<(f32, f32)>` - Chip load range (min, max)
- `power_factor: Option<f32>` - Power factor multiplier
- `ramp_angle_max: Option<f32>` - Max ramp angle (degrees)
- `adaptive_stepover_max: Option<f32>` - Max adaptive stepover (%)

## Files to Modify

1. `crates/gcodekit5-ui-egui/src/windows/tools_manager/`
   - `view.rs`: add the new fields to `geometry()` and add tab functions for
     Advanced Geometry and Limits
   - `mod.rs`: add the new tabs to `Tab`
   - `crates/gcodekit5-app/src/tool_form.rs`: add the fields to `ToolForm`,
     `from_tool()` and `build()`

2. `crates/gcodekit5-ui-egui/src/windows/materials_manager/`
   - `view.rs`: add the new fields to `properties()` and add a tab function for
     cutting parameters per tool type
   - `mod.rs`: add the new tab to `Tab`
   - `crates/gcodekit5-app/src/material_form.rs`: add the fields to
     `MaterialForm`, `from_material()` and `build()`

## Implementation Priority

1. **High Priority** - Tool Geometry (helix angle affects chip load calculations)
2. **High Priority** - Tool Limits (max_rpm, max_chip_load for validation)
3. **Medium Priority** - Material hardness (for SFM adjustments)
4. **Medium Priority** - Material cutting params (sfm_range, chipload_range)
5. **Lower Priority** - Advanced material properties (thermal_conductivity, etc.)
