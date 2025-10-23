# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]
- Initial constitution and templates
- Add initial specs and plan for GCodeKit6
- Add issue templates and README
- Scaffold Rust workspace and CI (in progress)
 - Open issues for top gaps: WebSocket transport (#3), GRBL char-counting (#4), arc expander (#5), Visualizer MVP (#6), UI wiring (#7), Macro storage (#8)

### Unreleased - 001-initial-plan updates

- Add WebSocket transport scaffolding (sync + async) behind `websocket` feature
- Enforce hard-coded 30s network connect/read timeout across sync and async transports
- Add performance harness (`crates/core/tests/perf_transport_latency.rs`) and emergency-stop timing test (`crates/core/tests/emergency_stop_timing.rs`) (ignored by default)
- Add device-adapters timeout unit test to assert connect-timeout behavior
- Add manual GitHub Actions workflow `ignored-harnesses.yml` which runs the harnesses on demand and uploads logs

- UI: Make Slint integration the default for the UI crate and add build-script
	support to run Slint code generation when `SLINT_INCLUDE_GENERATED=1` is set.
	The build script concatenates generated .rs files into `generated_ui.rs` and
	falls back to a committed `src/ui_generated.rs` stub when generation is not
	available. Added CI job to validate both fallback and generation builds.


