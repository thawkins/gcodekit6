# Drill Press

Generates G-code that drills one hole. If the tool is as wide as the hole, the tool plunges
straight down. If the tool is smaller than the hole, the tool cuts the hole with a helix
(helical interpolation).

## Hole Geometry
- **Hole Diameter:** Diameter of the finished hole.
- **Center X:** / **Center Y:** Position of the hole centre.
- **Tool Diameter:** Diameter of the drill or end mill. It must not be larger than the hole.

## Depth and Pecking
- **Top Z (Surface):** Z of the material surface. Every cut starts here.
- **Bottom Z (Depth):** Z of the hole bottom. It must be below Top Z.
- **Peck Depth (0 for none):** Depth of each plunge. With 0 the tool drills in one plunge.
  A peck depth so small that the hole would take more than 10000 pecks is refused.
  - Straight drilling: after each peck the tool rapids up to Top Z to clear chips, then
    back down to 0.5 mm above the last cut.
  - Helical cutting: the peck depth is the drop per turn of the helix (1 mm when it is 0).
    After the last turn the tool cuts one more full circle at the bottom, then returns to
    the centre.

## Machine Settings
- **Plunge Rate (mm/min):** Feed for straight plunges.
- **Feed Rate (mm/min):** Feed for the helical turns.
- **Spindle Speed (RPM):** Spindle speed, started with `M3` and stopped with `M5` at the end.
- **Safe Z:** Height for travel moves and the final retract. It must be above Top Z.
- **Home Device Before Start:** Adds `$H` at the start of the job. The travel and retract
  moves then use machine coordinates (`G53`) so they never rise above the homed position.

## Buttons
- **Load** / **Save:** Load or save the page's settings as a preset.
- **Generate:** Create the G-code.
- **Send to Editor:** Replace the program in the G-Code Editor with the G-code.
- **Preview:** Replace the editor's program and show it in the Visualizer.

## Notes
- The tool refuses to run while the device is in laser mode (`$32=1`). Switch the device to
  CNC mode first.
- The tool needs a device with at least 3 axes (X, Y, Z).

## Related
- [CAM Tools](help:cam_tools)
- [Index](help:index)
