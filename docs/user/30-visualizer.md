# Toolpath Visualizer

## Overview

The 3D Visualizer renders your G-code toolpath and provides stock removal simulation. It draws with wgpu for hardware-accelerated rendering.

## Toolpath Display

When a G-code file is loaded, the visualizer shows:

- **Rapid moves (G0)** — Drawn in grey, as are moves with the spindle or laser off
- **Cutting moves (G1)** — Coloured by spindle or laser power
- **Arc moves (G2/G3)** — Rendered as smooth curves
- **Current tool position** — Shown as a marker during connection or streaming

## Camera Controls

### Mouse Controls

| Action | Control |
|--------|---------|
| **Orbit** | Left button + drag |
| **Pan** | Shift + left button + drag |
| **Zoom** | Scroll wheel |

### NavCube

The NavCube in the corner of the visualizer provides quick access to standard views:

| View | Description |
|------|-------------|
| **Top** / **Bottom** | Camera above or below, looking at the XY plane |
| **Front** | Camera in front, looking at the XZ plane |
| **Left** / **Right** | Camera at the side, looking at the YZ plane |
| **Isometric** | Four corner views (NW, NE, SW, SE) showing all axes |

Click any face of the NavCube to snap to that view.

### Keyboard Controls

These keys work while the Visualizer has the keyboard.

| Key | Action |
|-----|--------|
| **F** | Zoom to fit |
| **+** / **-** | Zoom in / out |
| **Ctrl+0** (Cmd+0 on macOS) | Reset view |

## Display Options

Toggle visibility of visual elements:

| Option | Description |
|--------|-------------|
| **Show Rapid Moves** | Rapids and moves with the spindle or laser off |
| **Show Cutting Moves** | Cutting moves |
| **Show Laser/Spindle** | The tool position marker |
| **Show Grid** | Reference grid on the XY plane |
| **Show Machine Bounds** | The active device's work area |
| **Show Stock Removal** | The stock removal simulation (see below) |

Grid spacing is set with **Grid spacing** in the same sidebar.

## Stock Removal Simulation

The visualizer can simulate material removal:

- Displays a height-map rendering showing depth of cuts
- Color gradient indicates material depth
- Progress percentage shown during simulation
- Updates as the program streams to the machine

## Execution Visualization

While streaming a program:

- The **executed portion** of the toolpath is highlighted differently from the remaining path
- The **tool marker** moves in real-time to show the current machine position
- Progress is visible at a glance

## Performance

The visualizer is optimized for large files. Files with 100,000+ lines render within a few seconds. For very large files, consider zooming in to a region of interest for smoother interaction.

## See Also

- [G-Code Editor](20-gcode-editor.md) — Edit your G-code files
- [Streaming G-Code](80-streaming.md) — Running programs
