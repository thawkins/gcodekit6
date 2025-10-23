# Implementation Plan: GCodeKit6 - Desktop Fabrication Control (initial plan)

**Branch**: `001-initial-plan` | **Date**: 2025-10-23 | **Spec**: `/specs/001-initial-plan/spec.md`
**Input**: Feature specification from `/specs/001-initial-plan/spec.md`

**Note**: This plan implements the initial design for a Rust + Slint desktop
application to control fabrication machines (CNC, laser, 3D printers). This file
was scaffolded by `.specify/scripts/bash/setup-plan.sh` and is now being filled
as part of the `/speckit.plan` workflow.

## Summary

Implement a cross-platform desktop application (GCodeKit6) that sends G-code
to fabrication machines (GRBL, TinyG, G2core, Smoothieware), provides a Slint
UI for job control and visualization, and enforces safety, observability, and
robust communication per the project constitution.

Primary approach: Rust-based core for device communication and safety,
Slint for UI, structured logging via `tracing`, modular architecture with a
clear separation between communication, parsing, UI, and persistence layers.

## Technical Context

**Language/Version**: Rust edition 2021 (minimum); recommend Rust 1.70+ (stable)

**Primary Dependencies**:
- `slint` — UI framework (desktop)
- `tokio` — async runtime for serial and I/O
- `serialport` or `jserialcomm`-equivalent Rust crate (`serialport-rs`) for
  device communication
- `tracing` + `tracing-subscriber` — structured logging
- `anyhow`, `thiserror` — error handling
- `serde` + `serde_json` — config and runtime serialization

**Storage**: Local file storage for project files, job history, and settings
(TOML/JSON files inside `target/temp/` during development; persistent data in
`~/.local/share/gcodekit6/` or platform-appropriate location)

**Testing**: `cargo test` with tests located in `tests/` (contract/,
integration/, unit/). Use `#[test]` and `#[tokio::test]` where appropriate. All
tests MUST run with a 10-minute timeout in CI.

**Target Platform**: Desktop cross-platform (Linux, macOS, Windows). Primary
development host: Linux.

**Project Type**: Single desktop application with modular crates (workspace)

**Performance Goals**: Real-time responsiveness for command delivery to device
— typical p95 latency < 50ms for local serial command round-trip in normal
conditions; job streaming throughput sufficient for common G-code feedrates.
These goals are initial and NEEDS CLARIFICATION during Phase 0 research.

**Constraints**: Safety-first; no unsafe code without justification. All
device-facing code must be robust to transient communication errors and
implement emergency stop handling.

**Scale/Scope**: Project intended as desktop application with modest codebase
(initially one workspace with several crates: core, ui, device-adapters,
utils). Expected scope: MVP to support GRBL + Smoothieware + basic sender
features.

## Constitution Check

GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.

Checks derived from `.specify/memory/constitution.md`:

- Language MUST be Rust edition 2021 or greater — OK (plan uses Rust)
- UI MUST use Slint — OK (plan specifies Slint)
- Tests MUST be in `tests/` folder and use cargo test — OK (plan follows this)
- Logging MUST use `tracing` — OK (plan specifies `tracing`)

No blocking constitution violations detected at plan level. Any deviation
during design or implementation must be documented and justified in the
complexity tracking section.

## Project Structure

### Documentation (this feature)

```text
specs/001-initial-plan/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)
<!--
  ACTION REQUIRED: Replace the placeholder tree below with the concrete layout
  for this feature. Delete unused options and expand the chosen structure with
  real paths (e.g., apps/admin, packages/something). The delivered plan must
  not include Option labels.
-->

```text
# [REMOVE IF UNUSED] Option 1: Single project (DEFAULT)
src/
├── models/
├── services/
├── cli/
└── lib/

tests/
├── contract/
├── integration/
└── unit/

# [REMOVE IF UNUSED] Option 2: Web application (when "frontend" + "backend" detected)
backend/
├── src/
│   ├── models/
│   ├── services/
│   └── api/
└── tests/

frontend/
├── src/
│   ├── components/
│   ├── pages/
│   └── services/
└── tests/

# [REMOVE IF UNUSED] Option 3: Mobile + API (when "iOS/Android" detected)
api/
└── [same as backend above]

ios/ or android/
└── [platform-specific structure: feature modules, UI flows, platform tests]
```

**Structure Decision**: Single Rust workspace at repository root with the
following layout (starter):

```text
Cargo.toml                # workspace manifest
crates/
  core/                  # core device communication, parsing, safety logic
  ui/                    # Slint UI crate
  device-adapters/       # adapters for GRBL, TinyG, Smoothieware, etc.
  utils/                 # helpers, logging setup
tests/
  contract/
  integration/
  unit/
```

This layout satisfies the constitution's requirement for modular, testable
crates and keeps the UI separate from device-facing code.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., 4th project] | [current need] | [why 3 projects insufficient] |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |
