# Tasks: 001-initial-plan

Phase 1: Setup (project initialization)

- [ ] T001 Initialize workspace CI to run crate-local tests and enforce clippy/formatting in CI (files: .github/workflows/*)
- [ ] T002 [P] Add documentation entry for running harnesses and env vars (file: README.md)
- [ ] T003 [P] Add GitHub Actions workflow to run harnesses and upload logs (file: .github/workflows/harnesses.yml)
- [ ] T004 Ensure `XDG_DATA_HOME` override is documented and used in tests (file: crates/utils/src/settings.rs)

Phase 2: Foundational (blocking prerequisites)

- [ ] T005 Implement atomic storage helpers with directory creation and safe rename semantics (file: crates/core/src/persistence.rs)
- [ ] T006 [P] Implement transport timeout defaults (30s) for network connect/read paths (files: crates/device-adapters/src/async_network.rs, crates/device-adapters/src/network.rs)
- [ ] T007 [P] Feature-gate optional websocket transports and ensure default build does not include them (files: crates/device-adapters/Cargo.toml, src/async_websocket.rs, src/websocket_sync.rs)
- [ ] T008 [P] Add tracing-based logging in core paths and replace `println!` in production code (files: crates/core/src/*.rs, crates/device-adapters/src/*.rs)

Phase 3: User Story 1 - Connect to a Device (US1) (Priority: P1)

Goal: Discover and connect to serial and network devices and surface firmware/version in UI.
Independent test criteria: A test that connects to an in-process simulated device and verifies connect->identify flow.

- [ ] T009 [US1] Create Device model and persistence support (file: crates/core/src/device.rs)
- [ ] T010 [US1] Implement serial transport adapter with discovery (file: crates/device-adapters/src/serial.rs)
- [ ] T011 [US1] Implement TCP/UDP transport adapters and connect semantics (file: crates/device-adapters/src/network.rs)
- [ ] T012 [US1] Implement DeviceManager connect/discover APIs and unit tests (file: crates/core/src/device_manager.rs, crates/core/tests/connect_device.rs)
- [ ] T013 [US1] Wire Device discovery into Slint UI connect dialog (file: crates/ui/src/ui_impl.rs)
- [ ] T014 [US1] Add integration test: simulated device + DeviceManager connect and identify (file: tests/connect_device.rs)

Phase 4: User Story 2 - Send G-code Files (US2) (Priority: P1)

Goal: Load a `.gcode` file, preview, and stream it line-by-line with pause/resume and progress.
Independent test criteria: Automated streamer tests that feed lines to a simulated device and assert progress and pause/resume behavior.

- [ ] T015 [US2] Add Job model and job history persistence hooks (file: crates/core/src/job.rs)
- [ ] T016 [US2] Implement Streamer core (send_line/read/ack) with pause/resume API (file: crates/core/src/streamer.rs)
- [ ] T017 [US2] Implement UI hooks for file open, preview, and streaming controls (file: crates/ui/src/ui_impl.rs)
- [ ] T018 [US2] Create integration tests for streamer behavior against simulated device (file: crates/core/tests/streamer_tests.rs)
- [ ] T019 [US2] Implement pause/resume semantics with ack/error handling and retries (file: crates/core/src/streamer.rs)

Phase 5: User Story 3 - Emergency Stop & Safety (US3) (Priority: P1)

Goal: Emergency Stop halts streaming immediately and returns device to safe state; measure latencies.
Independent test criteria: harness that records p50/p95/p99 from stop API invocation to simulated device ack.

- [ ] T020 [US3] Implement emergency-stop API in core and ensure atomic stop semantics (file: crates/core/src/emergency.rs)
- [ ] T021 [US3] Add instrumented simulated device to harnesses for latency measurement (file: crates/core/tests/emergency_stop_timing.rs)
- [ ] T022 [US3] Add measurement reporting (p50/p95/p99) and document environment in quickstart (file: specs/001-initial-plan/quickstart.md)
- [ ] T023 [US3] Add optional hardware E-stop integration hooks (file: crates/device-adapters/src/hw_estop.rs)

Phase 6: Cross-cutting & Polish

- [ ] T024 [P] Add structured logging instrumentation (tracing) for transports and streaming (files: crates/*/src/*.rs)
- [ ] T025 [P] Add clippy/format CI gate and fix any warnings (files: .github/workflows/*)
- [ ] T026 [P] Document device adapter contracts and example configurations (file: specs/001-initial-plan/contracts/README.md)
- [ ] T027 [P] Create a small perf harness CI job and documentation for repeating measurements (file: .github/workflows/harnesses.yml, specs/001-initial-plan/quickstart.md)

Dependencies (story completion order)

1. Phase 1 -> Phase 2 (setup precedes foundational)
2. Phase 2 -> US1 (foundational must be ready before connecting/wiring)
3. US1 -> US2 (device connect before streaming)
4. US1 -> US3 (device connect before emergency stop tests)

Parallel opportunities

- Tasks marked [P] are safe to run in parallel. Examples:
  - T002, T003, T006, T007, T008, T024, T025, T026, T027

Implementation strategy

- MVP scope: Focus on US1 (Connect to a Device) and US3 Emergency Stop harness for safety verification. Implement streaming (US2) after stable connections are in place.
- Deliver in small increments: finish Phase 1 & 2, then implement US1 fully (T009..T014) with tests, then US3 harness (T020..T022), then US2.

Tasks summary

- Total tasks: 27
- Tasks per story:
  - Phase 1: 4
  - Phase 2: 4
  - US1: 6
  - US2: 5
  - US3: 4
  - Cross-cutting: 4
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
 - [ ] T029 [P] Add performance harness for transport round-trip latency and throughput in `/home/thawkins/Projects/gcodekit6/tests/perf/transport_latency.rs` — measure p50/p95/p99 under simulated and local device conditions and record results in CI artifacts (maps to SC-001, SC-003)
 - [ ] T030 [P] Add emergency-stop timing integration test in `/home/thawkins/Projects/gcodekit6/tests/integration/emergency_stop_timing.rs` — measure elapsed time from UI-trigger/API call to confirmed device stop under simulated device; target <200ms (maps to FR-004, SC-003)

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
