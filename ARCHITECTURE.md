# GCodeKit5 Architecture Overview

This document describes the high-level architecture of GCodeKit5, including crate dependencies, data flow, design patterns, and threading model.

## Table of Contents

- [Crate Structure](#crate-structure)
- [Crate Dependency Graph](#crate-dependency-graph)
- [Data Flow](#data-flow)
- [Key Design Patterns](#key-design-patterns)
- [Threading Model](#threading-model)
- [Error Handling Strategy](#error-handling-strategy)
- [Related Documentation](#related-documentation)

## Crate Structure

GCodeKit5 is organized as a Cargo workspace with 9 library crates and 1 binary crate. The front end is egui on wgpu with the craft-libs widget crates ([ADR-012](docs/adr/ADR-012-egui-craft-libs.md)):

```
gcodekit5/
├── src/main.rs              # Binary entry point
└── crates/
    ├── gcodekit5-core/           # Foundation types and traits
    ├── gcodekit5-communication/  # Device protocols
    ├── gcodekit5-settings/       # Configuration
    ├── gcodekit5-devicedb/       # Device profiles
    ├── gcodekit5-visualizer/     # Toolpath parsing and geometry
    ├── gcodekit5-designer/       # CAD/CAM design
    ├── gcodekit5-camtools/       # CAM operations
    ├── gcodekit5-app/            # Toolkit-free application state
    └── gcodekit5-ui-egui/        # egui + craft-libs front end
```

### Crate Responsibilities

| Crate | Purpose | Key Types |
|-------|---------|-----------|
| **core** | Shared types, traits, events, state | `Position`, `MachineStatus`, `Error` |
| **communication** | Serial/TCP/WebSocket transports, firmware protocol registry | `Communicator`, `ControllerProtocol`, `ProtocolProfile` |
| **visualizer** | G-code parsing for the toolpath view, 3D camera, stock simulation | `Visualizer`, `Camera`, `StockSimulator3D` |
| **designer** | Shape drawing, toolpath generation, CAD operations | `Shape`, `DesignerState`, `ToolpathGenerator` |
| **camtools** | Box maker, puzzle generator, image engraver | `TabbedBoxMaker`, `JigsawPuzzleMaker`, `BitmapImageEngraver` |
| **devicedb** | Device profile CRUD, persistence | `DeviceProfile`, `DeviceManager` |
| **settings** | Application preferences, serialization | `Config`, `SettingsError` |
| **app** | Toolkit-free application state: machine controller, console model, device and tool forms | `MachineController`, `MachineEvent` |
| **ui-egui** | egui front end: window, menus, panes, dialogs, wgpu visualizer | `GcodeKitApp`, `Machine` |

## Crate Dependency Graph

Internal normal (non-dev) dependencies, generated from Cargo. Regenerate it rather than editing it by hand:

```bash
cargo metadata --format-version 1 --no-deps | jq -r '.packages[] | .name as $n | .dependencies[] | select(.name|startswith("gcodekit5")) | select(.kind==null) | "\($n) -> \(.name)"'
```

```
gcodekit5-design (binary gcodekit5) -> ui-egui
ui-egui       -> app, camtools, communication, core, designer, devicedb, settings, visualizer
app           -> communication, core, devicedb, settings
designer      -> camtools, core
camtools      -> core, devicedb
visualizer    -> core
communication -> core
devicedb      -> core
settings      -> core
core          -> (none)
```

The `gcodekit5-` prefix is dropped above. The root package `gcodekit5-design` builds the `gcodekit5` binary.

### Dependency Rules

1. **core** has no internal dependencies (only external crates like `serde`, `thiserror`)
2. Every other library crate depends on **core**
3. **ui-egui** depends on all other library crates; **app** holds the state it shows and has no GUI dependency
4. **settings** is used only by **app** and **ui-egui**
5. No circular dependencies allowed

## Data Flow

### 1. G-Code Loading and Display

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  File Open  │────▶│ Editor text │────▶│   Parser    │────▶│ Toolpath    │
│  (UI)       │     │ (ui-egui)   │     │  (core)     │     │ (visualizer)│
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
                           │                                       │
                           ▼                                       ▼
                    ┌─────────────┐                         ┌─────────────┐
                    │ Syntax      │                         │ wgpu        │
                    │ Highlighting│                         │ Rendering   │
                    └─────────────┘                         └─────────────┘
```

### 2. Machine Control Flow

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  UI Button  │────▶│  Command    │────▶│ Connection  │────▶│  Serial     │
│  (Jog/Home) │     │  Builder    │     │  Manager    │     │  Port       │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
                                                                   │
                                                                   ▼
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  UI Update  │◀────│  State      │◀────│  Response   │◀────│  CNC        │
│  (DRO/Status)     │  Manager    │     │  Parser     │     │  Controller │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
```

### 3. G-Code Streaming Flow

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Send       │────▶│ Stream      │────▶│ Character   │
│  Button     │     │ Manager     │     │ Counting    │
└─────────────┘     └─────────────┘     │ Protocol    │
                           │            └──────┬──────┘
                           │                   │
                    ┌──────▼──────┐            │
                    │ Progress    │            │
                    │ Tracking    │◀───────────┘
                    └──────┬──────┘      (ok responses)
                           │
                    ┌──────▼──────┐
                    │ UI Progress │
                    │ Bar Update  │
                    └─────────────┘
```

### 4. Designer to G-Code Flow

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Shape      │────▶│ Toolpath    │────▶│ G-Code      │────▶│ Editor      │
│  Drawing    │     │ Generator   │     │ Emitter     │     │ Text        │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
       │                   │
       │                   ▼
       │            ┌─────────────┐
       │            │ Pocket/     │
       │            │ Profile/    │
       │            │ Engrave     │
       │            └─────────────┘
       │
       ▼
┌─────────────┐
│ Visualizer  │
│ Preview     │
└─────────────┘
```

## Key Design Patterns

### 1. Immediate-Mode UI over Plain State

egui redraws the UI every frame from plain Rust state, so panes borrow `&mut` state directly and no `Rc<RefCell<T>>` is needed for callbacks. State shared with a worker thread uses `Arc<Mutex<T>>` and channels.

```rust
// Pattern: a pane reads and edits its state in one function per frame
pub fn ui(&mut self, ui: &mut egui::Ui) {
    if ui.button("Zero").clicked() {
        self.pending = Some(Action::ZeroAxis(Axis::X));
    }
}
```

See [ADR-004: Interior Mutability](docs/adr/ADR-004-interior-mutability.md) for the history of the GTK-era pattern.

### 2. Machine Worker and Event Channel

`MachineController` (`crates/gcodekit5-app/src/machine/controller.rs`) starts a worker thread that owns the transport. The UI sends commands over one `std::sync::mpsc` channel and receives `MachineEvent`s over another.
The UI drains pending events once per frame with `MachineController::poll`.

### 3. Firmware Protocol Registry

Firmware variants are the `ControllerProtocol` enum (`crates/gcodekit5-communication/src/firmware/mod.rs`). Per-protocol behaviour is a `ProtocolProfile` looked up with `protocol_profile` in `firmware/protocol_registry.rs`; transports implement the `Communicator` trait.

### 4. Geometry Kernels

The designer and camtools use `csgrs` for 2D booleans (`perform_boolean` in `crates/gcodekit5-designer/src/ops.rs`, Gerber layers in `crates/gcodekit5-camtools/src/gerber.rs`) and `cavalier_contours` for offsetting.

- `cavalier_contours` can panic on degenerate input: repeated consecutive vertices, or a closed polyline whose last vertex repeats the first.
- Clean a polyline with `clean_polyline` (`crates/gcodekit5-designer/src/ops.rs`, `crates/gcodekit5-camtools/src/gerber.rs`) before `parallel_offset` or a boolean.
- Risky calls are wrapped in `catch_unwind` (for example `safe_parallel_offset` in `crates/gcodekit5-designer/src/geometry.rs`); find them with `rg -l catch_unwind crates --type rust`.
- Prefer bulge arcs over linearised segments for rounded shapes: fewer vertices and fewer offset panics.

### 5. Coordinate Conversion

CNC coordinates are millimetres with Y up; egui screen coordinates are points with Y down. The designer canvas converts between them in `CanvasView`; the visualizer uses a 3D orbit `Camera`. See [Coordinate System Documentation](docs/COORDINATE_SYSTEM.md) for details.

## Threading Model

### Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        UI Thread (eframe / egui)                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │
│  │   Panes     │  │ Frame loop  │  │ wgpu render │  │  User Input │    │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘    │
└─────────────────────────────────────────────────────────────────────────┘
         │ commands (mpsc)          ▲ events (mpsc) + repaint hook
         ▼                          │
┌─────────────────────────────────────────────────────────────────────────┐
│              Machine worker thread (owns the serial transport)          │
└─────────────────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────┐
│  Serial Port    │
│  Read/Write     │
└─────────────────┘
```

### Thread Types

| Thread | Purpose | Communication |
|--------|---------|---------------|
| **UI (eframe)** | Frame loop, widgets, rendering | Reads `MachineEvent`s once per frame |
| **Machine worker** | Serial I/O, GRBL streaming, status polling | `std::sync::mpsc` commands in, events out |
| **Background jobs** | Stock removal, toolpath preview, large file work | Spawned thread, result sent back, repaint requested |

### Thread Safety Rules

1. The transport is owned by the machine worker alone; no lock is held across I/O
2. The UI thread never blocks on I/O: it sends a command and reads events with `MachineController::poll`
3. A worker wakes the UI with `egui::Context::request_repaint` (the controller's repaint hook)
4. Use channels (`std::sync::mpsc`) for thread communication
5. Use `Arc<Mutex<T>>` for shared state across threads; the lock is held for one call only

### Example: Machine Events on the UI Thread

```rust
// Once per frame, from GcodeKitApp
for event in controller.lock().unwrap_or_else(PoisonError::into_inner).poll() {
    machine.apply(event); // fold into MachineStatus
}
// Any worker thread that needs a redraw calls the hook
ctx.request_repaint();
```

## Error Handling Strategy

### Layered Approach

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           UI Layer                                      │
│                    anyhow::Error → User Dialog                          │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        Application Layer                                │
│              anyhow::Result with .context() for chaining                │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         Library Crates                                  │
│         thiserror-derived error types for specific errors               │
└─────────────────────────────────────────────────────────────────────────┘
```

### Error Types by Crate

| Crate | Error Type | Defined in |
|-------|------------|------------|
| **core** | `Error` | `crates/gcodekit5-core/src/error.rs` |
| **settings** | `SettingsError` | `crates/gcodekit5-settings/src/migrate.rs` |
| **app** | `MachineError` | `crates/gcodekit5-app/src/machine/types.rs` |

See [ADR-005: Error Handling](docs/adr/ADR-005-error-handling.md) for details.

## Related Documentation

- [ADR-012: egui and craft-libs Front End](docs/adr/ADR-012-egui-craft-libs.md)
- [ADR-001: GTK4 UI Framework](docs/adr/ADR-001-gtk4-ui-framework.md) (superseded)
- [ADR-002: Coordinate System](docs/adr/ADR-002-coordinate-system.md)
- [ADR-003: Modular Crates](docs/adr/ADR-003-modular-crates.md)
- [ADR-004: Interior Mutability](docs/adr/ADR-004-interior-mutability.md)
- [ADR-005: Error Handling](docs/adr/ADR-005-error-handling.md)
- [Coordinate System Documentation](docs/COORDINATE_SYSTEM.md)
- [Developer Setup Guide](DEVELOPMENT.md)
- [Contributing Guidelines](CONTRIBUTING.md)
