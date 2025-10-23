 # Tasks: 001-initial-plan — GCodeKit6 initial MVP

 This file was generated from `/home/thawkins/Projects/gcodekit6/specs/001-initial-plan/plan.md`,
 `/home/thawkins/Projects/gcodekit6/specs/001-initial-plan/spec.md`,
 `/home/thawkins/Projects/gcodekit6/specs/001-initial-plan/data-model.md`, and
 the contracts in `/home/thawkins/Projects/gcodekit6/specs/001-initial-plan/contracts/`.

 Phase numbering follows: Phase 1 = Setup, Phase 2 = Foundational, Phase 3+ = User Stories (by priority), Final Phase = Polish.

 ## Phase 1 — Setup

 - [X] T001 Create workspace resolver entry in `/home/thawkins/Projects/gcodekit6/Cargo.toml` (setup)
 - [ ] T002 [P] Create `.gitignore` entries for `/home/thawkins/Projects/gcodekit6/target/` and `/home/thawkins/Projects/gcodekit6/target/tmp/` in `/home/thawkins/Projects/gcodekit6/.gitignore`
 - [X] T003 Initialize persistent data directory helpers in `/home/thawkins/Projects/gcodekit6/crates/utils/src/storage.rs`
 - [X] T004 Add basic logging initialization using `tracing` in `/home/thawkins/Projects/gcodekit6/crates/utils/src/logging.rs`
 - [X] T005 [P] Add CI workflow to run `cargo fmt`, `cargo clippy`, and `cargo test` in `/home/thawkins/Projects/gcodekit6/.github/workflows/ci.yml`

 ## Phase 2 — Foundational (blocking prerequisites)

 - [ ] T006 [P] Define `Device` and `Job` structs per `/home/thawkins/Projects/gcodekit6/specs/001-initial-plan/data-model.md` in `/home/thawkins/Projects/gcodekit6/crates/core/src/models.rs`
 - [ ] T007 Implement config and settings loader (serde + dirs-next) in `/home/thawkins/Projects/gcodekit6/crates/core/src/config.rs`
 - [ ] T008 [P] Implement basic error types using `thiserror` in `/home/thawkins/Projects/gcodekit6/crates/core/src/error.rs`
- [X] T006 [P] Define `Device` and `Job` structs per `/home/thawkins/Projects/gcodekit6/specs/001-initial-plan/data-model.md` in `/home/thawkins/Projects/gcodekit6/crates/core/src/models.rs`
- [X] T007 Implement config and settings loader (serde + dirs-next) in `/home/thawkins/Projects/gcodekit6/crates/core/src/config.rs`
- [X] T008 [P] Implement basic error types using `thiserror` in `/home/thawkins/Projects/gcodekit6/crates/core/src/error.rs`
 - [ ] T009 [P] Create a `device-adapters` module interface in `/home/thawkins/Projects/gcodekit6/crates/device-adapters/src/lib.rs` documenting required adapter functions (`connect`, `send_line`, `emergency_stop`)
 - [ ] T010 [P] Add unit-test scaffolding under `/home/thawkins/Projects/gcodekit6/tests/unit/` and contract tests under `/home/thawkins/Projects/gcodekit6/tests/contract/`

 ## Phase 3 — [US1] Connect to a Device (Priority: P1)

 Story goal: User can discover and connect to a serial/USB or network-attached device and see firmware/version.

 Independent test criteria:
 - Can list serial ports (or network endpoints) via API and CLI
 - Can open a connection to a simulated device and receive firmware/version

 - [ ] T011 [US1] Implement serial discovery function in `/home/thawkins/Projects/gcodekit6/crates/device-adapters/src/serial.rs`
 - [ ] T012 [US1] Implement network discovery/connect in `/home/thawkins/Projects/gcodekit6/crates/device-adapters/src/network.rs`
 - [ ] T013 [US1] Implement `connect` API in `/home/thawkins/Projects/gcodekit6/crates/core/src/device_manager.rs` that uses adapters
 - [X] T011 [US1] Implement serial discovery function in `/home/thawkins/Projects/gcodekit6/crates/device-adapters/src/serial.rs`
 - [X] T012 [US1] Implement network discovery/connect in `/home/thawkins/Projects/gcodekit6/crates/device-adapters/src/network.rs`
 - [X] T013 [US1] Implement `connect` API in `/home/thawkins/Projects/gcodekit6/crates/core/src/device_manager.rs` that uses adapters
 - [ ] T014 [US1] Add simulated device test server in `/home/thawkins/Projects/gcodekit6/tests/integration/simulated_serial.rs`
 - [ ] T015 [US1] Add automated test: connect to simulated device and assert status `connected` in `/home/thawkins/Projects/gcodekit6/tests/integration/connect_device.rs`

 ## Phase 4 — [US2] Send G-code Files (Priority: P1)

 Story goal: User can open a G-code file, preview it, and stream it to the connected device with progress and pause/resume.

 Independent test criteria:
 - Can parse `.gcode` files into line sequences
 - Can stream lines to a simulated device with pause/resume and report progress

 - [ ] T016 [US2] Implement G-code file loader/parser in `/home/thawkins/Projects/gcodekit6/crates/core/src/gcode/parser.rs`
 - [ ] T017 [US2] Implement job queueing and progress tracking in `/home/thawkins/Projects/gcodekit6/crates/core/src/job.rs`
 - [ ] T018 [US2] Implement streaming worker that sends lines via adapter `send_line` in `/home/thawkins/Projects/gcodekit6/crates/core/src/streamer.rs`
 - [ ] T019 [US2] Add pause/resume control and tests in `/home/thawkins/Projects/gcodekit6/tests/integration/stream_pause_resume.rs`
 - [ ] T020 [US2] Add `.gcode` preview minimal UI hook in `/home/thawkins/Projects/gcodekit6/crates/ui/src/ui.slint` (placeholder)

 ## Phase 5 — [US3] Emergency Stop & Safety (Priority: P1)

 Story goal: Emergency Stop must immediately halt streaming and put device in safe state.

 Independent test criteria:
 - Emergency Stop invocation halts streamer within <200ms in simulated tests
 - Device receives stop sequence and no further lines are sent

 - [ ] T021 [US3] Implement `emergency_stop` API in `/home/thawkins/Projects/gcodekit6/crates/device-adapters/src/lib.rs` and per-adapter implementations
 - [ ] T022 [US3] Wire `emergency_stop` through `/home/thawkins/Projects/gcodekit6/crates/core/src/streamer.rs` to interrupt streaming
 - [ ] T023 [US3] Add integration test `/home/thawkins/Projects/gcodekit6/tests/integration/emergency_stop.rs` that starts streaming and triggers emergency stop, asserting no further sends
 - [ ] T024 [US3] Document hardware E-stop wiring and optional adapter support in `/home/thawkins/Projects/gcodekit6/specs/001-initial-plan/quickstart.md`

 ## Final Phase — Polish & Cross-cutting Concerns

 - [ ] T025 [P] Add structured logging to all transport send/receive points in `/home/thawkins/Projects/gcodekit6/crates/device-adapters/src/` and `/home/thawkins/Projects/gcodekit6/crates/core/src/`
 - [ ] T026 [P] Implement job history persistence in `/home/thawkins/Projects/gcodekit6/crates/core/src/persistence.rs`
 - [X] T026 [P] Implement job history persistence in `/home/thawkins/Projects/gcodekit6/crates/core/src/persistence.rs`
 - [ ] T027 [P] Add UI tests / smoke checks in `/home/thawkins/Projects/gcodekit6/crates/ui/tests/` (if Slint enabled in CI)
 - [ ] T028 [P] Review and add more firmware-specific adapters (GRBL, Smoothieware, TinyG, G2core) under `/home/thawkins/Projects/gcodekit6/crates/device-adapters/src/` as separate modules

 ## Dependencies (User story completion order)

 1. Phase 1 (Setup) must complete before Phase 2 (Foundational)
 2. Phase 2 must complete before Phase 3 (US1)
 3. US1 must be implemented before US2 and US3 for integration testing (connect before streaming/stop)

 ## Parallel execution examples

 - While `/home/thawkins/Projects/gcodekit6/crates/core/src/models.rs` (T006) is in progress, `/home/thawkins/Projects/gcodekit6/crates/utils/src/logging.rs` (T004) and `/home/thawkins/Projects/gcodekit6/crates/core/src/error.rs` (T008) can be implemented in parallel. Marked with [P].
 - Adapter implementations for different transports (serial vs network) are parallelizable: `/home/thawkins/Projects/gcodekit6/crates/device-adapters/src/serial.rs` (T011) and `/home/thawkins/Projects/gcodekit6/crates/device-adapters/src/network.rs` (T012) are [P].

 ## Implementation strategy (MVP first)

 - MVP scope suggestion: Complete Phase 1 + Phase 2 + Phase 3 + minimal Phase 4 streaming (basic pause/resume), emergency stop basic behavior in Phase 5 limited to software stop.
 - Deliver incrementally: land the device-adapters and core plumbing first, then UI hooks and polish.

 ## Output summary

 - Path: `/home/thawkins/Projects/gcodekit6/specs/001-initial-plan/tasks.md`
 - Total tasks: 28
 - Tasks per story:
   - Setup/Foundation: 10
   - US1: 5
   - US2: 5
   - US3: 4
   - Polish: 4
 - Parallel opportunities: adapter modules, logging, error types, firmware-specific adapters
 - MVP suggestion: US1 + core plumbing + minimal streaming worker (T011-T018)

 All tasks follow the checklist format and include absolute file paths where applicable.
