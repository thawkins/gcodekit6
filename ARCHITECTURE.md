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
| **core** | Shared types, traits, events, state | `MachineState`, `Position`, `GCodeLine`, `AppEvent` |
| **communication** | Serial/TCP/WebSocket protocols, firmware implementations | `Connection`, `GrblController`, `Protocol` |
| **visualizer** | G-code parsing for the toolpath view, viewport transforms | `ViewportTransform` |
| **designer** | Shape drawing, toolpath generation, CAD operations | `Shape`, `DesignerState`, `ToolpathGenerator` |
| **camtools** | Box maker, puzzle generator, image engraver | `TabbedBoxMaker`, `JigsawPuzzle`, `LaserEngraver` |
| **devicedb** | Device profile CRUD, persistence | `DeviceProfile`, `DeviceDatabase` |
| **settings** | Application preferences, serialization | `AppSettings`, `Preferences` |
| **app** | Toolkit-free application state: machine controller, console model, device and tool forms | `MachineController`, `MachineEvent` |
| **ui-egui** | egui front end: window, menus, panes, dialogs, wgpu visualizer | `GcodeKitApp`, `Machine` |

## Crate Dependency Graph

```
                              ┌─────────────────┐
                              │    gcodekit5    │
                              │   (binary)      │
                              └────────┬────────┘
                                       │
                              ┌────────▼────────┐
                              │ gcodekit5-ui-   │
                              │      egui       │
                              └────────┬────────┘
                                       │
         ┌─────────────┬───────────────┼───────────────┬─────────────┐
         │             │               │               │             │
         ▼             ▼               ▼               ▼             ▼
┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│  camtools   │ │  designer   │ │ visualizer  │ │     app     │ │communication│
└──────┬──────┘ └──────┬──────┘ └──────┬──────┘ └──────┬──────┘ └──────┬──────┘
       │               │               │               │               │
       │               │               │               │               │
       │        ┌──────┴───────┐       │               │               │
       │        │              │       │               │               │
       │        ▼              │       │               │               │
       │  ┌───────────┐        │       │               │               │
       │  │ devicedb  │        │       │               │               │
       │  └─────┬─────┘        │       │               │               │
       │        │              │       │               │               │
       │        │   ┌──────────┴───────┴───────────────┴───────────────┘
       │        │   │
       │        ▼   ▼
       │  ┌───────────┐
       │  │ settings  │
       │  └─────┬─────┘
       │        │
       └────────┼────────────────────────────────────────────┐
                │                                            │
                ▼                                            ▼
        ┌─────────────┐                              ┌─────────────┐
        │    core     │◄─────────────────────────────│  (all use)  │
        │ (foundation)│                              └─────────────┘
        └─────────────┘
```

### Dependency Rules

1. **core** has no internal dependencies (only external crates like `serde`, `thiserror`)
2. All crates depend on **core**
3. **ui-egui** depends on all other crates (aggregates everything); **app** holds the state it shows and has no GUI dependency
4. **designer** depends on **visualizer** (reuses rendering)
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

### 2. Event-Driven Architecture

Application events flow through a central event system:

```rust
// Core event types
pub enum AppEvent {
    // Machine events
    MachineStateChanged(MachineState),
    PositionUpdated(Position),
    
    // Connection events
    Connected(DeviceInfo),
    Disconnected,
    
    // G-code events
    StreamingStarted,
    StreamingProgress(usize, usize),
    StreamingComplete,
    
    // UI events
    FileOpened(PathBuf),
    SettingsChanged,
}

// Event dispatch
fn dispatch_event(event: AppEvent) {
    // Notify all registered listeners
    for listener in &self.listeners {
        listener.on_event(&event);
    }
}
```

### 3. Builder Pattern for Configuration

Used for complex object construction:

```rust
// Builder pattern example
let toolpath = ToolpathBuilder::new()
    .feed_rate(1000.0)
    .plunge_rate(500.0)
    .safe_height(5.0)
    .step_down(2.0)
    .build()?;
```

### 4. Strategy Pattern for Firmware Protocols

Different firmware types implement a common protocol trait:

```rust
pub trait FirmwareProtocol: Send + Sync {
    fn name(&self) -> &str;
    fn parse_status(&self, response: &str) -> Option<MachineStatus>;
    fn format_command(&self, cmd: &Command) -> String;
    fn supports_feature(&self, feature: Feature) -> bool;
}

// Implementations
struct GrblProtocol;
struct TinyGProtocol;
struct FluidNCProtocol;
```

### 5. Viewport Transform for Coordinate Conversion

Centralized coordinate transformation:

```rust
pub struct ViewportTransform {
    pub zoom: f64,
    pub pan_x: f64,
    pub pan_y: f64,
    pub width: f64,
    pub height: f64,
}

impl ViewportTransform {
    /// World (mm) to screen (pixels) with Y-flip
    pub fn world_to_screen(&self, x: f64, y: f64) -> (f64, f64) {
        let sx = (x - self.pan_x) * self.zoom + self.width / 2.0;
        let sy = self.height - ((y - self.pan_y) * self.zoom + self.height / 2.0);
        (sx, sy)
    }
}
```

See [Coordinate System Documentation](docs/COORDINATE_SYSTEM.md) for details.

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

| Crate | Error Type | Key Variants |
|-------|------------|--------------|
| **core** | `GCodeError` | `ParseError`, `ValidationError` |
| **communication** | `CommunicationError` | `PortNotFound`, `Timeout`, `ProtocolError` |
| **settings** | `SettingsError` | `IoError`, `SerializationError` |
| **devicedb** | `DeviceDbError` | `NotFound`, `DuplicateId` |

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
