# CAM Tools

GCodeKit6 includes a suite of specialized CAM tools for generating G-code directly, without needing external CAM software.

## Using CAM Tools

1. Open the **CAM Tools** page, pick a tool from the list and click **Open**
2. Configure the parameters for your project
3. Click **Generate** to show the program on the page
4. Click **Send to Editor** to replace the editor's program with it, or **Preview** to do that and show it in the visualizer
5. Stream it from the editor or save it to a file

**Load** and **Save** read and write a tool's parameters as a JSON preset. Most tools also have a **Help** button in the page header. Generating is refused while a job is running.

The tools are: Tabbed Box Maker, Jigsaw Puzzle Generator, Laser Image Engraver, Speeds and Feeds Calculator, Spoilboard Surfacing, Create Spoilboard Grid, Gerber to G-Code, Drill Press and Probe Tools.

---

## Tabbed Box Maker

Generate laser or CNC cut boxes with finger joints (tab-and-slot construction).

**Parameters**:
- Box dimensions: X (Width), Y (Depth), H (Height), Outside Dims
- Material: Thickness, Burn / Tool Dia
- Finger joints (multiples of thickness): Finger Width, Space Width, Surrounding Spaces, Play (fit tolerance), Extra Length
- **Finger Style:** Rectangular, Springs, Barbs or Snap Fit
- **Box Type:** Full Box, No Top, No Bottom, No Sides, No Front/Back, No Left/Right
- Dividers X / Dividers Y, and **Divider Keying:** Walls and Floor, Walls Only, Floor Only, None
- Laser settings: Passes, Power (S), Feed Rate, Z Step Down; work origin Offset X / Offset Y

**Output**: G-code for cutting all box panels laid out flat. **Generate Shapes** adds the panel outlines to the Designer instead.

---

## Jigsaw Puzzle Generator

Create jigsaw puzzles with randomized interlocking tabs.

**Parameters**:
- Puzzle dimensions: Width, Height
- Pieces Across, Pieces Down
- Tab Size (%), Jitter (%), Corner Radius
- Random Seed (**Randomize** picks a new one)

**Output**: **Generate** adds the puzzle outlines to the Designer as grouped shapes and brings the Designer to the front. It does not produce G-code directly.

---

## Laser Image Engraver

Convert raster images (PNG, JPEG, BMP, GIF, TIFF) to G-code for laser engraving.

**Parameters**:
- **Image**: Image Path (Browse...), Width, Rotation (0, 90, 180, 270 degrees), Mirror X, Mirror Y, Invert
- **Halftoning**: Method (None, Threshold, Bayer 4x4, Floyd-Steinberg, Atkinson), Threshold
- **Laser**: Feed Rate, Travel Rate, Min Power (%), Max Power (%), Power Scale (S)
- **Scanning**: Pixels per mm, Line Spacing, Bidirectional, Scan Direction (Horizontal or Vertical)
- **Work Offsets**: Offset X, Offset Y

**Output**: G-code with variable laser power based on pixel intensity.

---

## Speeds and Feeds Calculator

Calculate cutting parameters from a tool, a material and a device.

**Inputs**:
- Tool (from the CNC Tools manager) and Material (from the Materials manager)
- Device (its spindle limits come from the Device Manager)
- Operation Type: Slotting, Pocketing, Profiling, Adaptive / HSM, Drilling, Plunging
- Depth of Cut, Stepover, Tool Stick-out, Coolant, Conservative Calculation

**Outputs** (after **Calculate**): spindle speed, feed rate, chip load, surface speed, material removal rate, spindle power and estimated deflection. **Reset** restores the defaults.

---

## Spoilboard Surfacing

Generate a parallel toolpath to flatten your CNC machine's spoilboard or wasteboard.

**Parameters**:
- Width, Height
- Tool Diameter, Stepover (%)
- Feed Rate, Spindle Speed, Cut Depth
- Safe Z Height

**Output**: G-code for back-and-forth parallel passes covering the entire surface area.

---

## Create Spoilboard Grid

Engrave a reference grid of lines on your spoilboard with a laser, for workpiece alignment.

**Parameters**:
- Width, Height
- Grid Spacing
- Feed Rate, Laser Power (S)
- Laser Mode: M3 (Constant Power) or M4 (Dynamic Power)

**Output**: G-code that engraves the grid lines.

---

## Gerber to G-Code (PCB Milling)

Convert one layer of a Gerber project folder to G-code. **Browse...** picks the folder; the layer files are found by name.

**Layer Type**:
- Top Copper, Bottom Copper
- Top Solder Mask, Bottom Solder Mask
- Top Screen Print, Bottom Screen Print
- Drill Holes (an Excellon drill file)
- Board Outline

**Parameters**:
- Board Width, Board Height, Offset X, Offset Y
- Tool Diameter, Isolation Width, Cut Depth, Safe Z, Feed Rate, Spindle Speed
- Remove Excess Copper, Use Board Outline for Rubout
- Alignment holes: Generate Alignment Holes, Hole Diameter, Margin, Drill Depth (Z)

**Output**: G-code for isolation milling around copper traces, the board outline, or drilling.

---

## Drill Press

Drill one hole: a single plunge, peck drilling, or helical milling for a hole larger than the tool.

**Parameters**:
- Hole Diameter, Center X, Center Y
- Tool Diameter
- Top Z (Surface), Bottom Z (Depth), Peck Depth (0 for none)
- Plunge Rate, Feed Rate, Spindle Speed, Safe Z

The Drill Press is refused when the device is in laser mode (`$32=1`).

---

## Probe Tools

Probe for Z, edges and corners on the machine. See [Probing](PROBING.md).

---

## Common Settings Across CAM Tools

Many G-code tools have a **Home Device Before Start** option that homes the machine (`$H`) at the start of the program.

All dimensional values are entered in mm (or inches, depending on your unit setting). Internal calculations always use mm.
