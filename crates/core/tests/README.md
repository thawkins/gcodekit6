Performance & emergency-stop test harnesses

This directory includes manual, ignored tests that act as measurement harnesses.

How to run

Run the perf harness (measures RTT percentiles) manually:

```bash
cargo test --manifest-path crates/core/Cargo.toml perf_transport_latency -- --ignored --nocapture
```

Run the emergency-stop timing harness manually:

```bash
cargo test --manifest-path crates/core/Cargo.toml emergency_stop_timing -- --ignored --nocapture
```

Notes

- Both tests are ignored by default and intended for manual use.
- They open local TCP sockets; change ports in the test files if needed.
- Running these on CI is optional — see `.github/workflows/ignored-harnesses.yml` for a manual workflow.
