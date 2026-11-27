# Rust-Specific Project Guidelines

When loaded, first say: "Rust language instructions loaded."

## MANDATORY: `cargo fmt` after every `.rs` edit

After you create or edit ANY `.rs` file, run `cargo fmt` before doing anything else (at minimum once before
you finish; it is idempotent and safe). Never hand-format Rust. The CI gate is `cargo fmt --all -- --check`;
a task is NOT complete while any edited `.rs` file is unformatted.

## Toolchain and Build

- Rust edition 2024 (`edition = "2024"`, workspace-inherited); minimum toolchain `rust-version = "1.95"`
  (root `Cargo.toml`).
- `cargo build` (debug; the default), `cargo build --release` (only when asked for a release build),
  `cargo check`. Allow up to 1500 seconds for a build.
- Lint: `cargo clippy`. Format: `cargo fmt` / `cargo fmt --all -- --check`.

## Workspace Layout

- All crates live in `crates/<crate-name>/` (e.g. `crates/gcodekit5-core`), each with its own metadata,
  dependencies and targets. Prefer small, single-responsibility crates.
- The root `Cargo.toml` is the workspace manifest (explicit `members = [...]` list, `resolver = "2"`) and also
  hosts the primary binary package (`[package] name = "gcodekit5-design"`, `[[bin]] name = "gcodekit5"`).
  Do not put library-crate metadata or dependencies there.
- Organize modules as named files with inline `mod` declarations; reserve `mod.rs` for legacy or deeply
  nested modules.

## Code Rules

- **No `unsafe`.** Every crate root (`crates/*/src/lib.rs`) and `src/main.rs` carries `#![deny(unsafe_code)]`;
  there are no approved production sites. A future site needs a scoped `#[allow(unsafe_code)]`, a
  `// SAFETY:` comment stating the invariant, and an entry here saying why no safe API exists. Tests may use
  a file-level `#![allow(unsafe_code)]` only for the Rust 2024 `std::env::set_var`/`remove_var` migration.
- **No `.unwrap()` or `.expect(` on user-facing production paths**; propagate or handle `Result`/`Option`.
  CI denies `.unwrap()` in every target; tests use `.expect("why")`. CI command:
  `cargo clippy --workspace --all-targets -- -D warnings -W clippy::unwrap_used`.
- Zero warnings: code must compile cleanly and pass `cargo clippy` on every change.
- Match enums and results exhaustively; no wildcard `_` fallback except for non-exhaustive external types.
- Call `.clone()` explicitly on non-`Copy` types; no hidden or cascading clones in hot iterators/closures.
- Prefer iterator adapters (`map`, `filter`, `fold`) over index-based loops; take `&str` rather than `String`
  when ownership is not needed.
- Errors: `Result<T, E>` with `?`; `thiserror` for custom (library) errors, `anyhow::Result` in `main`.
- Concurrency: there is no async runtime. Device I/O runs on a worker thread fed by `std::sync::mpsc`
  (gcodekit5-app `MachineController`).
- Logging: the `tracing` crate with structured logging, not `println!`/`eprintln!` in application code;
  `debug!()` for non-hot paths, `trace!()` for debug scenarios. Remove temporary `debug!()` calls once the
  issue is resolved.
- Style: rustfmt defaults (4 spaces, max width 100, Unix newlines); imports grouped std, external, local;
  `snake_case` functions/variables/modules, `PascalCase` types/traits/enums, `SCREAMING_SNAKE_CASE`
  constants; prefer explicit types and type aliases for complex ones.
- Docs: `//!` for crate/module docs, `///` for public APIs, `//` for internal comments.
- No wildcard imports in production code (`use super::*;` in test modules and `pub use foo::*` re-exports
  are fine). Cognitive complexity <= 30 and missing-docs warnings are review targets, not enforced limits.
- No emojis or non-ASCII in comments and identifiers; prefer plain ASCII (`-`, `|`, `+`) over box-drawing
  characters in terminal output.

## Tests

Commands (the workspace uses `cargo nextest`, configured in `.config/nextest.toml`):
- A full suite takes up to 40 minutes: run it rarely, save its output to a file under `target/temp/`, and
  re-read that file instead of re-running.
- Wrap every run as `timeout 1500 cargo nextest run ...`; nextest also kills any single test after 150 s.
- `cargo nextest run [-p <crate>] [-E 'test(<name>)']`; `-E 'test(test_foo::)'` isolates one
  `tests/test_foo.rs` module, `-E 'test(a) + test(b)'` unions filters. CI shape:
  `cargo nextest run --workspace --no-fail-fast`. Also `cargo nextest list`, `--success-output immediate`,
  `-j 1` (diagnose interleaving).
- `cargo test --doc` runs doc tests (nextest does not). `cargo test -p <crate> --test all test_foo::` is
  the in-process form.

Layout:
- All tests live in a crate's `tests/` directory (workspace-root tests in the root `tests/`). Do not add new
  inline `#[cfg(test)]` modules to library source; move inline tests out where practical.
- Use `#[test]` (no async tests). Name tests `test_<component>_<scenario>`. Import the crate under test by
  its public name (e.g. `gcodekit5_core`).
- One file per test unit, one binary per crate: each crate sets `autotests = false` and one
  `[[test]] name = "all"` with `path = "tests/all.rs"`. Add a file as one more
  `#[path = "test_x.rs"] mod test_x;` line in `tests/all.rs`, never as a new top-level target. Test names keep
  the `test_x::` prefix. Convert CI `--test <file>` uses to `-E 'test(<file>::)'`.
- Private items: widen to `pub(crate)` and re-import via `#[path = "../src/<module>.rs"] mod <module>;`;
  `crate::` inside it resolves at the `all.rs` root, so add a `pub use gcodekit5_<crate>::<x>;` shim there
  (or use the crate-absolute path). Complex cases put this in the source file:
  `#[cfg(test)] #[path = "../../tests/test_<module>.rs"] mod test_<module>;`. A narrow `#[allow(clippy::duplicate_mod)]` on `all.rs` is
  expected when children share `support/mod.rs`.
- Isolation: a test that mutates or observes a process-global (cwd, env vars, a `static`) takes
  `crate::global_env_lock()` (defined in every `tests/all.rs`), the same lock for every such test, never a
  file-local mutex, and never twice in one test (it self-deadlocks). Prefer a per-test directory
  (`target/temp/<unique-name>`) over locking. Cross-process exclusion (repo fixtures, shared `target/temp/`
  paths, tree scans) needs a `[test-groups]` entry in `.config/nextest.toml` plus
  `[[profile.default.overrides]]` and a `profile.ci` twin (the `gpu` group shows the shape). Never weaken an
  assertion or add a target to regain isolation.
- Tests must not write into the repository: write under `target/temp/`, resolved from
  `env!("CARGO_MANIFEST_DIR")`; pass the root as a parameter (or a `#[doc(hidden)]` `OnceLock` setter)
  instead of reading `std::env::current_dir()`. Never "fix" leaks with `git restore`.
