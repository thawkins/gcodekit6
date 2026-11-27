# GCodeKit6 - Specification Document

**Version**: 2.0.0-alpha.6
**Last Updated**: 2026-11-28
**Status**: Alpha Release

This document describes what the shipped product does. Each statement can be traced to the
code. Release history: [CHANGELOG.md](CHANGELOG.md).

## 1. Product

GCodeKit6 is a desktop G-code sender for GRBL-family CNC and laser machines, with a 2D/3D
designer, CAM tool generators and a toolpath visualizer in one window. The UI is egui on
wgpu with the craft-libs widget crates ([ADR-012](docs/adr/ADR-012-egui-craft-libs.md)).
The binary is `gcodekit6` (`src/main.rs`); it opens any files named on the command line. `--version` prints the version and `--help` (or `-h`) a short usage, both without opening the window (`src/lib.rs` `parse_args`).

User guides: [docs/user/](docs/user/README.md), including
[6-axis machines](docs/user/6AXIS.md) and [probing](docs/user/PROBING.md).
UI field tracking: [docs/UI_FIELDS_MISSING.md](docs/UI_FIELDS_MISSING.md).

## 2. Crate Map

Workspace members (`cargo metadata --no-deps --format-version 1 | jq -r '.packages[].name'`):

| Crate | Role |
|-------|------|
| `gcodekit6-design` (root) | The `gcodekit6` binary: logging setup, then the egui front end |
| `gcodekit6-core` | Shared types, units, G-code words and arcs, probe types, tool and material data |
| `gcodekit6-communication` | Serial, TCP and WebSocket transports; GRBL and Repetier protocol support; firmware detection |
| `gcodekit6-camtools` | CAM generators (tabbed box, jigsaw, laser engraving, Gerber, Excellon, spoilboard, drill press, probe routines, speeds and feeds) |
| `gcodekit6-designer` | Designer model, shapes, geometry operations, pocketing and toolpath generation |
| `gcodekit6-visualizer` | G-code parsing for preview, toolpath geometry, 3D stock removal simulation |
| `gcodekit6-devicedb` | Device profiles |
| `gcodekit6-settings` | Application settings (`config.toml`) and the one-time import of older files |
| `gcodekit6-app` | Toolkit-free application state: machine worker, console, managers, translations |
| `gcodekit6-ui-egui` | The egui front end: pages, panes, dialogs, in-app help |

## 3. Controllers

Defined in `crates/gcodekit6-communication/src/firmware/protocol_registry.rs`:

- **Supported**: GRBL and Repetier.
- **Detected, not supported**: grblHAL, FluidNC, TinyG, g2core, Smoothieware and Marlin.
  The app recognises them but does not drive them.

## 4. Connections

- One `SerialCommunicator` drives all three `ConnectionDriver` kinds: Serial, TCP and
  WebSocket. The transports are blocking; there is no async runtime.
- **Serial**: USB/RS-232 ports, listed from the system.
- **TCP**: host and port from the device profile.
- **WebSocket**: plain `ws://<host>:<port>/` only (no TLS), carrying the same GRBL text
  protocol as serial. FluidNC's default port is 81. Uses `tungstenite`.
- **Auto Reconnect** (device profile checkbox): when an open connection drops (not a user
  Disconnect), the app retries up to 10 times, waiting 1 s, 2 s, then 5 s between attempts,
  and shows the attempt count (n/10). Disconnect cancels it. A job that was streaming is
  stopped and is not resumed after the reconnect.

## 5. UI Pages

The main window has 8 pages, in tab order (`crates/gcodekit6-ui-egui/src/workspace.rs`):

| Page | Contents |
|------|----------|
| Designer | Toolbox, canvas, and the inspector over the objects list |
| Visualizer | The toolpath visualizer |
| Machine Control | Machine controls, with the G-code editor above the console beside them |
| CAM Tools | The CAM tool generators |
| Device Manager | Device profiles |
| Device Config | The controller's firmware settings |
| CNC Tools | The CNC tool library |
| Materials | The material library |

Keyboard shortcuts: [docs/user/90-shortcuts.md](docs/user/90-shortcuts.md).

### 5.1 Designer

- Drawing tools: Select, Rectangle, Circle, Line, Ellipse, Polyline, Text, Triangle, Polygon,
  Gear, Sprocket and Pan. Raster images can be imported in Laser 2D mode.
- Machine modes: Laser 2D and CNC 3D.
- Import SVG and DXF; export G-code and SVG.
- Undo/redo, copy/paste, group/ungroup, align (left, right, top, bottom, centre X/Y),
  arrays (Linear, Grid, Circular), Convert to Path and Convert to Rectangle.
- Geometry operations on a shape: Offset and Chamfer.
- Pocket strategies (`PocketStrategy`): Raster (angle, bidirectional), Offset
  (contour-parallel, the default) and Adaptive (offset loops cut from the inside out).
- Pockets with holes: paths (including imported SVG/DXF), gears, sprockets and text are
  pocketed with the even-odd rule. Each region is offset as a whole, so the tool keeps a
  tool-radius clearance from the outer wall and from every hole. All three strategies
  support holes; text pockets leave letter counters standing.

### 5.2 Visualizer

- 2D/3D toggle in the sidebar: 2D locks the camera to the orthographic top view (left drag
  pans); 3D orbits freely. The navigation cube switches to standard views.
- Fit to Content, Reset View and Fit to Device Working Area.
- **3D stock removal simulation**: a height map with one material height per XY cell
  (0.19 mm cells), cut by a flat end mill. Up to 2^27 cells (512 MiB), enough for a
  1220 x 2440 mm sheet; larger stock is refused before allocating. "Stock Z0 at:" and
  "Stock XY0 at:" place the stock in work coordinates. It runs on a background thread
  with progress and cancel.

### 5.3 CAM Tools

The picker lists 9 tools (`crates/gcodekit6-ui-egui/src/panes/cam_tools/tools.rs`):

- **Jigsaw Puzzle Generator**.
- **Tabbed Box Maker**: box types Full Box, No Top, No Bottom, No Sides, No Front/Back and
  No Left/Right; X and Y dividers with divider keying (Walls and Floor, Walls Only, Floor
  Only, None); finger styles Rectangular, Springs, Barbs and Snap Fit; Optimize Layout
  packs the parts.
- **Laser Image Engraver**: raster images to laser G-code.
- **Speeds and Feeds Calculator**: uses the tool and material libraries.
- **Spoilboard Surfacing** and **Create Spoilboard Grid**.
- **Gerber to G-Code**: PCB isolation milling. G36/G37 regions are filled (linear and
  circular edges, with cut-in holes). `%AM` apertures are flashed for primitives 1, 4, 5,
  7, 20 and 21, with `$n` variables and arithmetic. Clear polarity (`%LPC`) cuts its
  objects out of the dark copper before it, in file order. Step and repeat (`%SR`) and
  moire primitives are not drawn, and the page warns about them.
- **Excellon drilling** (the Gerber page's Drill Holes layer): reads Excellon drill files
  (`M48` header, metric or inch, tool table, routed slots) and drills them in one program,
  tools smallest first, with an `M0` tool-change pause between tools. Drill files are
  found by `.drl`/`.xln`/`.exc` extension, a name containing `drill`, or a `.txt` file
  whose first command is `M48`.
- **Drill Press**: peck drilling, and helical boring for holes larger than the tool.
- **Probe Tools**: Z touch, edge find, corner find, bore centre, boss centre and tool
  length (`ProbeRoutine`).

### 5.4 Settings and Data

- Settings are saved in `config.toml` (craft-settings). The older `config.json` and data
  files (such as `devices.json`) are imported once on first start.
- The settings folder is `gcodekit6` in the platform config folder. On first start a
  pre-rename `gcodekit5` folder (config, local data, or home fallback) is copied to `gcodekit6`
  once when `gcodekit6` does not exist; the old folder is left unchanged.
- Translations: `po/*.po` (de, es, fr, it, pt). The build script embeds the catalogs, so
  there is no gettext at run time.

## 6. G-code the Visualizer Interprets

The preview parser (`crates/gcodekit6-visualizer/src/visualizer/parse.rs`) acts on these
words. Other words do not change the preview.

| Words | Effect in the preview |
|-------|-----------------------|
| G0, G1 | Rapid and linear moves |
| G2, G3 | Clockwise and counter-clockwise arcs |
| G38.2 to G38.5 | Probe moves |
| G4 | Dwell |
| G10, G28.1, G30.1, G43.1 | Axis words that are not motion |
| G17, G18, G19 | Arc plane XY, ZX, YZ |
| G20, G21 | Inches, millimetres |
| G28, G30 | Home moves |
| G53 | Machine coordinates for the line |
| G80 | Cancel motion mode |
| G90, G91 | Absolute, relative |
| G92 | Set position |
| M3, M4, M5 | Spindle on, spindle off |
| X Y Z A B C I J K R S P | Axis, arc and parameter words |

## Audit Follow-up (November 2026)
- **Streaming**: GRBL character-counting flow control (bytes in flight ≤ 127). The job sender polls status (`?`, 200 ms) itself so the DRO, alarm state and overrides stay live; an `ALARM:`/`error:` reply stops the job with the reason.
- **Connections**: device profiles carry a TCP host and port; connecting with a TCP or WebSocket profile opens a TCP stream or a `ws://` WebSocket through the same communicator as serial. Auto Reconnect reopens a dropped connection (see section 4).
- **Machine Control**: rotary A/B/C axes shown from the profile/status; Pause/Resume/Stop only during a job; inch jog presets (0.0001–1 in); probe failures restore G90 once idle.
- **Stop and busy guard**: Stop sends feed hold (`!`), waits for `Hold:0` (up to 10 s), then soft-resets (0x18), so the position is kept; E-Stop still resets at once. While a job streams, console lines (except `!`, `~`, `?`) and `$$`/`$n=v` are refused with a message; `$` commands also need Idle or Alarm. Pause state clears when the machine reports Run/Jog
- **Designer**: property fields commit on Enter/focus-out (one undo step per edit); every property edit is undoable; unsaved-changes prompt (Save / Discard / Cancel) on New/Open/quit/language restart; the same prompt guards hand edits in the G-code editor (New/Open/quit/language restart) and unsaved tool edits in the Tools Manager; SVG transforms and viewBox units; DXF `$INSUNITS`.
- **CAM probe tool**: routines use relative (G91) moves from the current position with probe depth, tip-radius and plate-thickness compensation; "Probe" runs the routine and reports the result; "Apply to WCS" sends `G10 L2` (or `G43.1` for tool length).
- **Bitmap engraving**: dark pixels receive more power by default; the Invert option engraves light areas.
