# Issue: WebSocket transport for remote devices

Summary
-------
Add a WebSocket transport adapter to `crates/device-adapters` to allow remote devices and browser-based UIs to communicate with gcodekit over WebSockets. This complements the existing TCP/UDP/Serial transports and enables hosting a WS endpoint inside the application or connecting to remote WS servers.

Motivation
----------
- Modern remote-control UIs (web apps) and remote devices often use WebSockets for bidirectional messaging.
- WebSocket transport enables remote streaming, telemetry, and control without exposing raw TCP/serial ports.

Proposed solution
-----------------
- Implement a new module `device_adapters::websocket` with both blocking and async variants (async-first, provide a small sync wrapper if needed).
- Use `tokio-tungstenite` for async WebSocket support and `tungstenite` for any sync needs.
- Provide a `Transport` adapter implementation that maps WebSocket text frames to lines and supports: send_line(), read_line(), flush(), is_alive(), and close().
- Add connection parsing: `ws://host:port/path` and `wss://...` for secure connections; support TLS via `native-tls` or `rustls` based on feature flags.
- Add unit tests and an integration test that starts a local WS server (tokio) and verifies round-trip streaming and emergency stop behavior.

Acceptance criteria
-------------------
- New files under `crates/device-adapters/src/websocket.rs` (and `async_websocket.rs` if split).
- Implement a `WebSocketTransport` struct that implements the project's `Transport` trait (or adapter trait used by `device-adapters`).
- Tests: `transport_trait_tests.rs` updated to include WebSocket transport scenario; local WS echo server test passes.
- Provide documentation comment and an example in `crates/device-adapters/README.md`.

Labels
------
- area:device-adapters
- feature
- priority:high

Estimate
--------
3-5 days (implementation, TLS config, tests, docs)

Subtasks (PR-sized)
--------------------
- [ ] Design API and feature flag for `websocket` support.
- [ ] Add optional dependencies to `crates/device-adapters/Cargo.toml` behind a feature flag (`websocket`).
- [ ] Implement `async_websocket.rs` using `tokio-tungstenite`.
- [ ] Export transport from `lib.rs` and provide a sync wrapper if needed.
- [ ] Add unit tests (connect/send/receive) and an integration test with a local WS server.
- [ ] Document usage in `crates/device-adapters/README.md` and link to issue #3.

Dependencies / blockers
----------------------
- Depends on async runtime (tokio) being available in test environment. Use feature flags for optional dependencies.
- TLS support increases complexity; initially support plain `ws://` and add `wss://` behind a feature flag.

Notes
-----
Prefer `tokio-tungstenite` and `tokio-native-tls` or `tokio-rustls` depending on the project's choice of TLS provider. Keep the adapter API small and consistent with existing transports.
