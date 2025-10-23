# gcodekit_ui

This crate contains the UI for gcodekit6. It uses Slint for the UI layout
and code generation.

Build notes
- Default features: `with-slint`, `slint_generated` — this enables the
  Slint runtime and attempts to run the Slint code generator during the
  crate build when `SLINT_INCLUDE_GENERATED=1` is set.
- To build without Slint and generation, disable default features:

```bash
cargo build --no-default-features
```

- To force Slint generation locally (requires slint-build available):

```bash
SLINT_INCLUDE_GENERATED=1 cargo build -p gcodekit_ui
```

Fallback behavior
- If Slint-generated artifacts are not present or generation fails, the
  crate falls back to a committed `src/ui_generated.rs` stub so the workspace
  remains buildable on CI and developer machines.
# gcodekit_ui — running the UI

This document explains how to build and run the `gcodekit_ui` crate both with and without Slint enabled. The crate is designed to compile and run even when Slint-generated code is not present; enabling the `with-slint` feature activates the full GUI path.

Prerequisites
- Rust toolchain (stable, 1.70+ recommended)
- Cargo (comes with Rust)
- Optional: Slint build/runtime if you want the native GUI. See the Slint project for platform-specific install instructions.

Run without Slint (default)

This is the simplest path and is suitable for CI or headless runs where you only need the Rust-side helpers.

From the workspace root:

```bash
cargo run -p gcodekit_ui
```

This will compile and run the UI crate using the fallback generated shim. The app will call into the Rust helpers but will not display a native Slint window unless built with the Slint feature.

Run with Slint (native GUI)

To enable the Slint UI, build and run the crate with the `with-slint` feature. Note: you must have the Slint Rust components available in your toolchain (the crate pulls the Slint runtime and build-time codegen). On many systems you can simply enable the feature and let Cargo fetch the Slint crate:

```bash
cargo run -p gcodekit_ui --features with-slint
```

If Slint codegen is required and missing, follow Slint's installation docs for your platform (it may require the `slint` CLI or additional system packages). If you run into build-time errors referencing Slint codegen, ensure `slint` is available or use the default (non-Slint) mode.

Environment variables
- `RUST_LOG` — set the log level (e.g., `RUST_LOG=info`)
- `GCK_LOG_FILE` — if set, `init_logging_prod()` writes rotated logs to this directory
- `GCK_LOG_JSON=1` — when compiled with the `json-logs` feature in `gcodekit_utils`, enables JSON-formatted logs
- `XDG_DATA_HOME` — override the platform data directory for tests or the running app

Examples

Run the UI with verbose logging and a local artifacts directory for logs:

```bash
export RUST_LOG=debug
export GCK_LOG_FILE="$PWD/artifacts/logs"
cargo run -p gcodekit_ui --features with-slint
```

Run the UI in headless/shim mode (no native GUI):

```bash
cargo run -p gcodekit_ui
```

Troubleshooting
- If you see compile errors from `crates/utils/src/logging.rs` complaining about borrowing/moves of `EnvFilter`, make sure you're on the latest workspace changes. The logging initializer clones the filter where needed to avoid move errors.
- If the Slint-enabled build fails with missing generated symbols, ensure you enabled the `with-slint` feature and that the Slint compiler/codegen ran successfully during the build.
- If the Connect button doesn't appear to call into core, confirm you built with `--features with-slint` and that the runtime UI is visible.

Next steps
- Hook the `ui_list_devices_detailed()` model into the Slint view (the example `ui.slint` file includes a `deviceModel` property and a Connect button wired to `connectRequested`).
- If you want, I can add a small script that launches the UI with common environment variables for development.
