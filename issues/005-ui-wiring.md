# Issue: UI wiring — connect Slint UI to backend streamer & device manager

Summary
-------
Wire the Slint-based UI to the backend services: device discovery/connection, job upload, streamer controls (play/pause/stop), and telemetry display. Provide a clean API boundary between UI and core logic and add tests for surface-level UI actions.

Motivation
----------
- The UI is currently a stub. For users to interactively control devices and stream jobs, we need event handlers and IPC between Slint and the Rust backend.

Proposed solution
-----------------
- Define a minimal UI API contract (`ui::Api`) that exposes methods: list_devices(), connect(device), upload_job(file), start_job(job_id), pause(), resume(), stop(), get_job_status(job_id), and subscribe_telemetry(callback).
- Implement the contract in `crates/ui/src/ui_impl.rs` hooking into `crates/core::device_manager` and `crates/core::streamer`.
- Use Slint's event callbacks and thread-safe channels (tokio mpsc or crossbeam) to pass events.
- Add unit/integration tests that simulate UI actions and assert dispatcher calls the correct backend methods.

Acceptance criteria
-------------------
- `ui::Api` implemented and documented.
- Basic UI demo shows device list, connect/disconnect, and streamer controls that affect the backend (demo mode with simulated device if no real devices present).
- Tests for the API contract ensuring correct calls into backend when UI actions are triggered.

Labels
------
- area:ui
- feature
- priority:high

Estimate
--------
2-4 days for core wiring and tests; additional time for polishing UX.

Notes
-----
- Be mindful of thread-safety and blocking. Keep backend operations async and avoid long ops on the UI thread.

Subtasks (PR-sized)
--------------------
- [ ] Define `ui::Api` contract and document methods.
- [ ] Implement API bindings in `crates/ui/src/ui_impl.rs` connecting to device manager and streamer.
- [ ] Add tests that simulate UI events and assert backend calls.
- [ ] Provide a demo UI flow (connect -> upload -> start) using simulated transport.
