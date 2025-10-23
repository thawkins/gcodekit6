# Project SPEC: GCodeKit6

Version: 0.11.0
Last Updated: 2025-10-23
Status: Active development (branch: 001-initial-plan)

GCodeKit6 is a Rust-based, Slint UI desktop application to control fabrication
machines (CNC, laser cutters/engravers, 3D printers). This document summarizes
project goals, current scope, and a prioritized plan to close gaps between the
implementation and the generated feature specification snapshot at
`target/tmp/SPEC.md`.

Quick pointers
- Feature specs live in `specs/` (e.g. `specs/001-initial-plan/`)
- Implementation branch for the initial plan: `001-initial-plan`

Core goals
- Reliable, safe device control for GRBL, TinyG, Smoothieware, g2core, FluidNC
- Cross-platform desktop app with responsive UI and robust communication
- Modular Rust workspace with test-first development and CI coverage

Primary (implemented) requirements
- Device discovery and connection (serial, TCP)
- G-code parsing, preview, and streaming with pause/resume semantics
- Emergency stop wiring and structured logging
- Persistent job history (saved to platform data dir via `utils::storage`)

Gaps & missing features (summary from `target/tmp/SPEC.md`)
1. Connectivity: WebSocket transport and full WebSocket API surface are not yet implemented in `device-adapters`. Implement reconnection/backoff and message queuing. See issue #3 for the WebSocket transport plan.
2. Communication: GRBL character-counted streaming (precise credit accounting) and a buffered communicator are partially implemented; extend tests and edge-case handling. See issue #4 for the GRBL character-counted streaming work.
3. G-Code processing: Advanced preprocessors (arc expansion, mesh leveling, coordinate transforms, line splitting) are minimal or missing in the `gcode` module. See issue #5 for the arc expander preprocessor.
4. Firmware support: Per-firmware settings managers and protocol adapters (GRBL settings parsing, TinyG/g2core JSON protocols) need completion and conformance tests.
5. Visualizer: 3D toolpath visualization (wgpu-based renderer) is specified but not present in the codebase. See issue #6 for the Visualizer MVP.
6. UI wiring: Slint UI exists as scaffolding; full integration of panels (Visualizer, DRO, Overrides, Macro editor) and their event bindings are pending. See issue #7 for UI wiring to streamer & device manager.
7. Remote APIs: REST/WebSocket APIs for remote control and streaming are planned but not implemented.
8. Macros & scripting: Macro storage, variable substitution, and execution engine are not implemented. See issue #8 for macro & script storage and execution.
9. Performance: Non-functional targets (parse/stream throughput, memory budgets) require benchmarks and CI gates.
10. Security & packaging: TLS for network transports, settings file permissions, and packaging/release workflows require work.

Prioritized implementation plan (short horizon)

Phase A — Stabilize core runtime (2–3 sprints)
- Finalize GRBL character-counted streaming and add unit/integration tests for credit accounting.
- Implement WebSocket transport in `device-adapters` with reconnection/backoff and message queuing.
- Extend `gcode` module with an arc expander (configurable segment length), and robust comment/whitespace preprocessors.
- Increase integration test coverage for communicator, streamer_worker, persistence, and emergency-stop scenarios.

Phase B — UI integration and visualization (2–4 sprints)
- Wire Slint UI panels to the core event system: connection panel, DRO, control panel, console.
- Implement a minimal 3D visualizer backed by wgpu for low‑resolution toolpath preview.
- Add UI-driven macro editing and persistent macro storage.

Phase C — Firmware feature completeness & tooling (ongoing)
- Implement firmware-specific parsers/managers (GRBL settings parsing, TinyG/g2core JSON parsing).
- Provide capability detection and capability flags per controller.
- Add conformance test harnesses using mocked firmware responses and integration tests.

Phase D — Ops, packaging, and security (1–2 sprints)
- Add TLS support for network transports and secure defaults for remote connections.
- Implement packaging for Linux/Windows/macOS and release CI workflows.
- Add performance benchmarks and CI gates for parsing/streaming throughput.

Deliverables and acceptance criteria
- Each phase ships with tests (unit + integration), CI green, and updated docs in `specs/` and `CHANGELOG.md`.
- Feature flags for risky changes to keep main branch stable during incremental rollout.

Immediate next actions (this week)
1. Commit this updated `SPEC.md`.
2. Create issues for the top gaps: WebSocket transport (#3), GRBL char-counting (#4), arc expander (#5), visualizer MVP (#6), UI wiring for connection/DRO/console (#7), macro storage (#8).
3. Add a minimal integration test that exercises GRBL streaming flow using a mocked Transport implementation.

Where to find detailed specs
- Full generated spec snapshot: `target/tmp/SPEC.md`
- Feature tasks and plans: `specs/001-initial-plan/tasks.md`
