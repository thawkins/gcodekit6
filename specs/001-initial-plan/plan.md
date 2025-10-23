# Implementation Plan: [FEATURE]

**Branch**: `[###-feature-name]` | **Date**: [DATE] | **Spec**: [link]
**Input**: Feature specification from `/specs/[###-feature-name]/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.specify/templates/commands/plan.md` for the execution workflow.

## Summary

Implement the initial MVP for GCodeKit6: a Rust-based desktop application (Slint UI) that discovers and connects to fabrication devices (serial + TCP/UDP), streams G-code files with pause/resume and progress reporting, and provides a reliable Emergency Stop with measured latency. The implementation will be partitioned into workspace crates (core device logic, device-adapters, utils, ui) and include deterministic harnesses for emergency-stop and perf testing.

## Technical Context

<!--
  ACTION REQUIRED: Replace the content in this section with the technical details
  for the project. The structure here is presented in advisory capacity to guide
  the iteration process.
-->

**Language/Version**: Rust (edition 2021). Recommend Rust 1.70+; project CI will use the latest stable Rust available on Ubuntu LTS runners.  
**Primary Dependencies**: Slint (UI), tokio (async runtime), tokio-tungstenite (async websockets, optional), tungstenite (sync websocket, optional), serialport or `serialport-rs`-compatible crate for serial communication, tracing (logging), anyhow/thiserror for error handling. Feature-gate optional transports (websocket) to keep default build minimal.  
**Storage**: Local file storage using platform-appropriate locations (XDG on Linux, App Support on macOS, %APPDATA% on Windows). Tests override via `XDG_DATA_HOME` for isolation. Use atomic write helpers (tmp+rename) for job history.  
**Testing**: `cargo test` for unit/integration; crate-local `crates/*/tests/` integration tests are permitted per constitution amendment. Harnesses for emergency-stop and perf measurements are included and can be triggered in CI via workflow_dispatch. Tests run with a 10-minute timeout in CI.  
**Target Platform**: Desktop (Linux, macOS, Windows). Primary CI target: Ubuntu-latest runner.  
**Project Type**: Multi-crate Rust workspace (core, device-adapters, utils, ui).  
**Performance Goals**: Emergency Stop response measurement targets: p50 < 50ms (simulated device), p95 < 200ms (nominal CI test target), p99 < 500ms (documented as upper bound). Streaming throughput: target to sustain device feedrates typical of GRBL (tens to hundreds of lines/sec) without buffer underrun.  
**Constraints**: Emergency Stop measured from API/button invocation to confirmed stop reported by device (or simulated ack). Tests must document environment (CPU/OS/transport) and measure p50/p95/p99. Network transports must enforce a 30s connect/read timeout to prevent indefinite blocking.  
**Scale/Scope**: MVP scope: single-user desktop app for controlling local or network-connected fabrication devices; anticipated codebase size ~ <50k LOC for MVP.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Gates evaluated against `.specify/memory/constitution.md`:

- Language requirement: Rust edition 2021 — SATISFIED (workspace uses Rust and Slint UI crate).  
- Testing requirement: Tests in `crates/*/tests/` permitted per 2025-10-23 amendment — SATISFIED. Ensure CI runs crate-local tests.  
- Logging: `tracing` usage recommended; avoid `println!` in production paths — PARTIAL (some tests use prints for diagnostics; acceptable for harnesses but aim to use `tracing` for production code).  
- Performance & Safety: Emergency Stop and streaming have safety constraints (response-time <200ms). This plan includes instrumented harnesses to measure latency and documents test harness environment — SATISFIED (design includes harnesses and measurement).  

No gate violations detected that block Phase 0. Continue to Phase 0 research and close any remaining clarifications.

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
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

**Structure Decision**: [Document the selected structure and reference the real
directories captured above]

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., 4th project] | [current need] | [why 3 projects insufficient] |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |
