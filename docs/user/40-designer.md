# Designer (CAD)

## Overview

The Designer is a built-in CAD tool for creating 2D and 3D designs that can be converted to G-code toolpaths. It supports drawing shapes, importing files, and generating cutting/engraving operations.

## Drawing Shapes

Select a shape tool from the toolbox and click/drag on the canvas to create it.

### Available Shapes

| Shape | Description |
|-------|-------------|
| **Rectangle** | Rectangular shape with width and height |
| **Circle** | Circle with a specified radius |
| **Ellipse** | Ellipse with major and minor axes |
| **Line** | Straight line between two points |
| **Polyline** | Multi-segment path (click points, double-click to finish) |
| **Triangle** | Three-point triangular shape |
| **Polygon** | Regular polygon with configurable number of sides |
| **Text** | Text rendered as outlines (configurable font, size, bold/italic) |

### Parametric Generators

| Generator | Description |
|-----------|-------------|
| **Gear** | Involute spur gear with configurable module and tooth count |
| **Sprocket** | Chain sprocket with configurable pitch and tooth count |

### Fast Shape Gallery

A gallery of pre-built parametric shape templates for quick insertion.

## Shape Manipulation

### Selection
- **Click** a shape to select it (displays handles)
- **Shift+Click** adds to the selection; **Ctrl+Click** adds or removes a shape
- **Drag** a selection box to select multiple shapes

### Transform Operations
- **Move** — Drag a selected shape to reposition (Shift locks the axis)
- **Resize** — Drag corner handles to scale
- **Rotate** — Right-click > **Rotate** by a fixed angle, or enter an exact **Rotation:** in Properties
- **Mirror** — Right-click > **Mirror on X** / **Mirror on Y**
- **Exact size** — Enter width and height in Properties. With **Lock Aspect:** on, editing the width sets height = width / ratio (and the reverse)

### Alignment
Align two or more selected shapes from the right-click **Align** menu:
- Align Left, Align Right, Align Top, Align Bottom
- Align Center X, Align Center Y

### Arrays
Duplicate shapes in patterns with right-click > **Array...**:
- **Linear array** — Copies along a line
- **Circular array** — Copies around a center point
- **Grid array** — Copies in a rectangular grid

### Grouping
- **Ctrl+G** — Group selected shapes
- **Ctrl+Shift+G** — Ungroup

## Non-Destructive Geometry Operations

These operations modify shape geometry without destroying the original:

| Operation | Description |
|-----------|-------------|
| **Offset:** | Expand or contract the shape boundary by a distance (Properties > Geometry Operations) |
| **Chamfer:** | Bevel corners at a specified distance (Properties > Geometry Operations) |
| **Corner > Radius:** | Round a rectangle's corners or fillet a path's corners |

## Objects List

The Layers pane lists the design's shapes in drawing order (there are no separate layers):
- Click a row to select the shape on the canvas
- Toggle a shape's visibility, or lock it so it cannot be selected
- Rename a shape
- Move shapes up and down the drawing order, or group and ungroup them

## Importing Files

# Import external designs via **File → Import**:

| Format | Description |
|--------|-------------|
| **SVG** | Scalable Vector Graphics — paths, circles, rectangles, ellipses |
| **DXF** | Drawing Exchange Format — lines, circles, arcs, polylines |
| **STL** | 3D models for CAM processing |

Imported geometry is scaled correctly to millimeters.

STL import is offered only when **Preferences > Advanced > STL File Import** is on.

# Import external designs via **File → Import Image**:

| Format | Description |
|--------|-------------|
| **JPG** | Raster Image |
| **PNG** | Raster Image |
| **BMP** | Raster Image |
| **TIF** | Raster Image |
| **WebP** | Raster Image |

Image import is available in **Laser 2D** machine mode only.

- Import image and then select it. With the image selected, the Properties Inspector will display its size and position, as well as the engraving settings: speed, laser power, resolution, etc.
- Once all properties are configured, click **Frame** to generate the G-code for the rectangle the image will occupy.
- Then, with the image still selected, click the **Generate G-Code** button to generate  G-code for the image itself.
- In Viewer, you will see the image that will be engraved based on the generated G-code. Note that there are two 2.5mm margins on each side if you choose horizontal engraving, or above and below the image if you choose vertical engraving. These are inertial adjustments to increase the engraving speed and ensure continuity. To prevent the laser from going into a negative position, if the image is positioned at X0, Y0, the image will automatically shift an offset of X2.5, Y2.5.

## Toolpath Generation

Convert designer shapes into G-code toolpaths:

### Operations

Each shape's **CAM Properties** (CNC 3D mode) set its **Operation:**:

| Operation | Description |
|-----------|-------------|
| **Profile** | Cut along the outline of a shape |
| **Pocket** | Remove material from inside a closed shape |

A pocket also has **Step In:** and a **Strategy:** of Raster, Offset or Adaptive. Every operation steps down in passes (**Step Down:**) and has a **Ramp Angle:**. Turn off **Use global values** to edit these per shape.

### Toolpath Parameters

The toolbox's Tool Settings and Stock Settings hold the global values:
- CNC 3D: Feed Rate, Spindle Speed, Tool Dia, Step Down, Continuous Z
- Laser 2D: Laser Speed, Laser Power, Laser Width, Passes
- Stock: Stock Width, Stock Height, Stock Thickness, Safe Z Height

## Undo / Redo

- **Ctrl+Z** — Undo the last action
- **Ctrl+Y** or **Ctrl+Shift+Z** — Redo
- The last 50 actions can be undone

## Project Files

Save your designs as `.gckd` design files, which preserve all shapes, properties, and toolpath settings. **Open** also reads older `.gck5` files.

## See Also

- [CAM Tools](50-camtools.md) — Specialized G-code generators
- [Tool Library & Materials](60-tool-library.md) — Tool and material management
