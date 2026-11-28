# Tabbed Box Maker

Generates a finger-jointed box layout and outputs G-code for laser cutting or CNC routing.

The generator creates multiple 2D panels (sides, top/bottom, dividers) and arranges them into a flat layout for cutting.

## Key inputs
### Box dimensions
- **X (Width), Y (Depth), H (Height)**: Overall box size.
- **Outside dims**: If enabled, the dimensions are treated as the outside size; otherwise they’re inside size.

### Box configuration
- **Box type**: Allows removing specific faces (no top, no bottom, etc.).
- **Dividers X / Y**: Number of internal dividers.
- **Divider keying**: Which faces include slots/tabs for dividers.
- **Optimize Layout**: Packs the panels into a compact layout.

### Material settings
- **Thickness**: Stock thickness.
- **Burn / Tool dia**: Kerf/tool compensation term.

### Finger joint settings
- **Finger width / Space width**: Expressed as multiples of thickness.
- **Surrounding spaces**: Extra spacing near edges.
- **Play**: Fit tolerance.
- **Extra length**: Adds length to tabs for tighter fits.
- **Finger Style**: Rectangular, Springs, Barbs, Snap Fit or Dogbone. Dogbone cuts a small relief at each inside finger corner so a round tool leaves room for the square corner of the mating finger.

### Laser settings
- **Passes / Power (S) / Feed rate**: Cutting parameters (laser-style), used when producing the final G-code.
- **Z Step Down**: How far Z drops on each pass after the first.

### Work origin offsets
Offsets the entire layout, and optionally homes before start (**Home Device Before Start**).

## Optimize layout
The optimizer packs the panels close together, with a 5 mm gap between parts. Preview the program to check that it fits your machine.

## Workflow
1. Set box dimensions and material thickness.
2. Configure joints and divider options.
3. Click **Generate** to make the program, then **Send to Editor**, or **Preview** to see it in the Visualizer. **Generate Shapes** adds the panel outlines to the Designer instead.
4. Cut the panels and assemble.

## Related
- [CAM Tools](help:cam_tools)
- [Visualizer](help:visualizer)
- [Index](help:index)


