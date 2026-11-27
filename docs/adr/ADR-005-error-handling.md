# ADR-005: Error Handling Strategy

## Status
Accepted

Amended 2026-11-28: replaced example types that never existed with the real error types; the `unwrap()`/`expect()` rule now defers to AGENTS-LANGUAGE.md.

## Context

GCodeKit5 handles errors from many sources:

- Serial port communication failures
- G-code parsing errors
- File I/O errors
- Configuration validation errors
- Firmware protocol errors
- UI state errors

A consistent error handling strategy is needed to:
- Provide actionable error messages to users
- Enable proper error recovery
- Support debugging and logging
- Maintain clean code without excessive error handling boilerplate

## Decision

We adopt a **layered error handling strategy**:

### 1. Library Crates: Use `thiserror` for Typed Errors

The real typed errors include:

- `gcodekit5_core::Error` (`crates/gcodekit5-core/src/error.rs`), with `Result<T>` as its alias.
- `SettingsError` (`crates/gcodekit5-settings/src/migrate.rs`).
- `MachineError` (`crates/gcodekit5-app/src/machine/types.rs`).

```rust
// crates/gcodekit5-core/src/error.rs
#[derive(Error, Debug)]
pub enum Error {
    /// Standard I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic error
    #[error("{0}")]
    Other(String),
}
```

### 2. Application Layer: Use `anyhow` for Context

`anyhow` is a dependency of the communication, devicedb, camtools, designer, app and ui-egui crates and of the root binary. Add context where an error crosses a boundary:

```rust
// crates/gcodekit5-ui-egui/src/panes/editor/buffer.rs
pub fn load(path: &Path) -> anyhow::Result<Self> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    // ...
}
```

### 3. UI Layer: Convert to User-Friendly Messages

The front end shows a translated prefix followed by the error's `Display` text in a notice or status message, for example `format!("{}: {e}", tr.tr("Save Error"))` in `crates/gcodekit5-ui-egui/src/panes/cam_tools/requests.rs`.

### Guidelines

| Context | Approach | Crate |
|---------|----------|-------|
| Library public API | Typed errors with `thiserror` | thiserror |
| Internal functions | `Result<T, E>` with `?` propagation | std |
| Application/binary | `anyhow::Result` with context | anyhow |
| Unrecoverable bugs | `panic!` / `unreachable!` | std |
| Optional features | `Option<T>` | std |

### Banned Patterns

The rule for `unwrap()` and `expect()` is in the "Core Code Rules" section of [AGENTS-LANGUAGE.md](../../AGENTS-LANGUAGE.md), which is canonical. In short: neither is allowed on user-facing production paths; handle or propagate the error instead.

```rust
let value = map.get("key").ok_or_else(|| anyhow!("Missing key"))?;
```

## Consequences

### Positive
- Clear error types in library APIs
- Rich context in error messages
- Error chains preserve root cause
- Consistent patterns across codebase
- User-friendly error display

### Negative
- Two error crates to learn (thiserror + anyhow)
- Some boilerplate for typed errors
- Must remember to add context at boundaries

### Neutral
- Errors are not silently swallowed
- Stack traces available in debug builds

## Alternatives Considered

1. **Only anyhow**: Simpler but loses type information for matching.

2. **Only thiserror**: More boilerplate for application code.

3. **Custom error types everywhere**: Maximum control but significant boilerplate.

4. **error-chain (deprecated)**: Was popular but now unmaintained.

## References

- [thiserror Documentation](https://docs.rs/thiserror)
- [anyhow Documentation](https://docs.rs/anyhow)
- [Rust Error Handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- [Error Handling in a Correctness-Critical Rust Project](https://sled.rs/errors.html)
