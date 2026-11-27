# Jigsaw Puzzle Generator

Creates a jigsaw-style cutting pattern as shapes in the Designer.

This tool is designed primarily for **laser cutters** (or very small CNC tools) and produces 2D outlines of interlocking puzzle pieces.

## What it generates
- An outer rectangle (the overall puzzle boundary)
- Internal cut lines forming pieces, using a pseudo-random jigsaw connector pattern

## Key inputs
### Puzzle Dimensions
- **Width / Height**: Overall finished puzzle size (50 to 2000 mm).
- **Corner Radius**: Rounds the outside corners (0 to 10 mm).

### Grid Configuration
- **Pieces Across / Pieces Down**: How many pieces in each direction (2 to 20). More pieces means more internal cuts. Each piece must be at least 15 mm.

### Puzzle Parameters
- **Tab Size (%)**: Controls connector size (10 to 30).
- **Jitter (%)**: Controls connector variation (0 to 13).
- **Random Seed**: Changes the piece shapes while keeping the same grid layout. Use **Randomize** to quickly try new patterns.

## Workflow
1. Choose puzzle size and piece count.
2. Set the connector sizing.
3. Click **Generate**. The pieces are added to the Designer as grouped shapes and the Designer comes to the front.
4. In the Designer, set the laser parameters and use **Generate G-Code** to make the program.

## Notes
- For CNC cutting: allow for your tool diameter, and consider reducing piece count.
- Always preview the result before cutting.

## Related
- [CAM Tools](help:cam_tools)
- [Visualizer](help:visualizer)
- [Index](help:index)
