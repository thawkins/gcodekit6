# gcodekit6

This repository contains multiple crates for the gcodekit6 application.

Running harness tests deterministically
-------------------------------------

There are two manual harnesses in `crates/core/tests/` useful for performance and emergency-stop timing evaluations:

- `perf_transport_latency` — performance harness that measures transport RTT.
- `emergency_stop_timing` — measures emergency-stop latency under a simulated device.

Both harnesses bind to an ephemeral port by default, which makes them safe to run in parallel and in CI. To run deterministically (fixed port), set the following environment variables before invoking `cargo test`:

- `GCK_PERF_PORT` — set a u16 port number to use for the perf harness (default: 0 = ephemeral).
- `GCK_EMERGENCY_PORT` — set a u16 port number to use for the emergency-stop harness (default: 0 = ephemeral).

Examples:

```bash
# Run perf harness using a deterministic port 40100
GCK_PERF_PORT=40100 cargo test --manifest-path crates/core/Cargo.toml perf_transport_latency -- --nocapture

# Run emergency-stop harness using a deterministic port 40200
GCK_EMERGENCY_PORT=40200 cargo test --manifest-path crates/core/Cargo.toml emergency_stop_timing -- --nocapture
```

You can also trigger the harnesses via the GitHub Actions workflow `Run harnesses` (workflow_dispatch) which will run both harnesses and upload logs as artifacts; see `.github/workflows/harnesses.yml`.

Logging / Tracing
-----------------

This project uses `tracing` for structured logging. To enable logs during local runs or tests, set the `RUST_LOG` environment variable (the default level is `info` when not set).

Examples:

```bash
# show info-level logs (default)
RUST_LOG=info cargo test -p gcodekit_core -- --nocapture

# show debug-level logs for more detail
RUST_LOG=debug cargo test -p gcodekit_core -- --nocapture
```

There is a helper to initialize the tracing subscriber programmatically. Call this early in your binary `main()` or at the start of harness tests:

```rust
// in tests or binaries
let _ = gcodekit_utils::logging::init_logging();
```

The harness tests in `crates/core/tests/` already call `init_logging()` so running them with `--nocapture` will produce timestamped, leveled logs that are captured by CI and printed to the console.

# GCodeKit6

GCodeKit6 is a desktop application to control fabrication machines (CNC, laser cutters/engravers, and 3D printers). It is implemented in Rust (edition 2021+) with a Slint UI and focuses on safety, real-time communication, and maintainability.

This repository contains the project constitution, development guidelines, and templates for contributing.

Key pointers:
- Constitution: `.specify/memory/constitution.md`
- Agent & runtime guidance: `AGENTS.md`
- Issue templates: `.github/ISSUE_TEMPLATE/`

Testing layout note
-------------------

Per the project constitution (see `.specify/memory/constitution.md`), integration tests may live either at the repository root `tests/` directory or inside crate-local `crates/*/tests/` directories. CI is expected to run both locations.

Examples — run all workspace tests (recommended):

```bash
# Run the entire workspace test suite (root + crate-local integration tests)
cargo test --all --all-features
```

Examples — run crate-local integration tests only:

```bash
# Run only the core crate integration tests
cargo test --manifest-path crates/core/Cargo.toml --all-features -- --nocapture
```

Getting started:
1. Install Rust and cargo (https://rustup.rs/)
2. Install Slint (https://slint-ui.com/)
3. Build: `cargo build`
4. Run tests: `cargo test` (tests may live at the repo root `tests/` or under `crates/*/tests/`)

License: TBD
