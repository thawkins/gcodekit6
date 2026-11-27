# ADR-003: Modular Crates Structure

## Status
Accepted

Amended 2026-11-28: replaced the hand-written crate tree with the command that lists the members; removed the deleted `ui` and `gcodeeditor` crates and an unmeasured build-time figure.

## Context

As GCodeKit6 grew, the single-crate structure became problematic:

- **Compile Times**: Full rebuilds took several minutes due to the large codebase
- **Code Organization**: Related functionality was scattered across files
- **Testing**: Integration tests were slow and coupled
- **Reusability**: No way to use core functionality in other projects
- **Cognitive Load**: New contributors struggled to understand the codebase

## Decision

We restructured GCodeKit6 as a **Cargo workspace** with multiple focused crates under `crates/`, plus the root `gcodekit6-design` package that builds the `gcodekit6` binary from `src/main.rs`.

The member list comes from this command, not from this file:

```bash
cargo metadata --no-deps --format-version 1 | jq -r '.packages[].name'
```

### Crate Responsibilities (as of 2026-11-28)

| Crate | Responsibility |
|-------|----------------|
| `gcodekit6-core` | Core types, traits, units and errors |
| `gcodekit6-communication` | Serial, TCP and WebSocket transports; firmware protocols |
| `gcodekit6-visualizer` | Toolpath interpretation, camera and stock-removal simulation |
| `gcodekit6-designer` | Shape drawing and toolpath generation |
| `gcodekit6-camtools` | CAM tools (box maker, puzzle, engraver) |
| `gcodekit6-devicedb` | Device profiles and persistence |
| `gcodekit6-settings` | Application settings |
| `gcodekit6-app` | Toolkit-free application state (machine controller, console model) |
| `gcodekit6-ui-egui` | egui front end (see ADR-012) |

## Consequences

### Positive
- **Faster Incremental Builds**: Only changed crates recompile
- **Clear Boundaries**: Each crate has defined responsibilities
- **Independent Testing**: Unit tests run per-crate in parallel
- **Reusability**: Core crates could be used in CLI tools or other UIs
- **Onboarding**: New contributors can focus on one crate

### Negative
- **Initial Setup Complexity**: More Cargo.toml files to maintain
- **Cross-Crate Changes**: Features spanning crates need coordinated updates
- **Version Management**: All crates share a version (workspace inheritance)
- **Circular Dependencies**: Must carefully design to avoid cycles

### Neutral
- Public API surface increases (more `pub` items)
- Documentation must cover inter-crate relationships

## Alternatives Considered

1. **Single Crate with Modules**: Simpler but doesn't solve compile time issues.

2. **Feature Flags**: Could conditionally compile parts, but adds complexity and doesn't improve organization.

3. **Separate Repositories**: Maximum isolation but coordination overhead and harder to make cross-cutting changes.

## References

- [Cargo Workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [Large Rust Codebase Patterns](https://matklad.github.io/2021/09/05/Rust-100k.html)
