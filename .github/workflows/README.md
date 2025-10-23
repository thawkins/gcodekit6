# Ignored Harnesses workflow

This workflow is used to run long-running or otherwise "ignored" test harnesses
manually on GitHub Actions and upload their logs for inspection.

Location
---------

The workflow file is: `.github/workflows/ignored-harnesses.yml`.

When to run
-----------

- Use this workflow when you want to execute integration/performance harnesses
  that are normally marked `#[ignore]` in the repository test suite. These
  harnesses can be long-running or require runner-specific resources.
- The workflow is only triggered manually via the Actions UI (workflow_dispatch).

What it does
-----------

- Checks out the repository.
- Installs Rust (stable) using `dtolnay/rust-toolchain-action@v1`.
- Builds the core crate to make sure tests compile.
- Runs a set of ignored harness tests (examples include
  `perf_transport_latency`, `emergency_stop_timing`, and `timeout_tests`) and
  captures their output into `artifacts/*.log` files.
- Uploads the log files as a workflow artifact named `harness-logs`.

How to run (from GitHub)
------------------------

1. Open the Actions tab in the repository on GitHub.
2. Select "Run ignored harnesses".
3. Click "Run workflow" and choose a branch (usually `main` or your feature
   branch).

How to run locally
------------------

You can run the same commands locally to reproduce or collect logs.

```bash
# Build the core crate (no tests run)
cargo test --manifest-path crates/core/Cargo.toml --no-run

# Run an ignored harness and capture output
mkdir -p artifacts
cargo test --manifest-path crates/core/Cargo.toml perf_transport_latency -- --ignored --nocapture 2>&1 | tee artifacts/perf_transport_latency.log || true

# Device-adapters timeout tests example
cargo test --manifest-path crates/device-adapters/Cargo.toml timeout_tests -- --nocapture 2>&1 | tee artifacts/timeout_tests.log || true
```

Artifacts
---------

- The job uploads all `artifacts/*.log` files as a single artifact named
  `harness-logs`. Download the artifact from the run details to inspect the
  logs.

Notes
-----

- The workflow runs on `ubuntu-latest` and assumes the runner has enough
  resources to run the harnesses. Some harnesses may require hardware or
  network access not available on CI — in that case run locally or on
  a machine that has the required environment.
- The workflow intentionally continues on per-harness failure (uses `|| true`
  in order to upload logs even when a harness fails).

Contact
-------

If you need help with a specific harness or want to add more harnesses to the
workflow, open an issue or a pull request and tag the maintainers.
