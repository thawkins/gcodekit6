# Quickstart: Running the MVP locally

1. Build the workspace

```bash
cargo build
```

2. Run tests (unit + integration)

```bash
# Run all tests (10-minute timeout recommended in CI)
cargo test
```

3. Run a harness deterministically (example)

```bash
# Perf harness
GCK_PERF_PORT=40100 cargo test --manifest-path crates/core/Cargo.toml perf_transport_latency -- --nocapture

# Emergency-stop harness
GCK_EMERGENCY_PORT=40200 cargo test --manifest-path crates/core/Cargo.toml emergency_stop_timing -- --nocapture
```

4. Configure the UI

- The Slint UI is in `crates/ui/`. Build and run the UI using `cargo run -p gcodekit_ui --manifest-path crates/ui/Cargo.toml` (future improvements will provide packaged installers).
# quickstart.md

## Quickstart: Build and run (Linux)

1. Install Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
2. Install Slint: follow instructions at https://slint-ui.com/
3. From repository root:

```bash
cargo build
cargo test
```

4. Run the application (once scaffolded):

```bash
cargo run -p ui
```

## First Run
- Open Connect dialog and select detected serial port
- Load a `.gcode` file and press Send
- Use Emergency Stop to immediately halt operations

