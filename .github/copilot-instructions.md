# gcodekit6 Development Guidelines

Auto-generated from all feature plans. Last updated: 2025-10-23

## Active Technologies
- Rust (edition 2021). Recommend Rust 1.70+; project CI will use the latest stable Rust available on Ubuntu LTS runners. + Slint (UI), tokio (async runtime), tokio-tungstenite (async websockets, optional), tungstenite (sync websocket, optional), serialport or `serialport-rs`-compatible crate for serial communication, tracing (logging), anyhow/thiserror for error handling. Feature-gate optional transports (websocket) to keep default build minimal. (001-initial-plan)
- Local file storage using platform-appropriate locations (XDG on Linux, App Support on macOS, %APPDATA% on Windows). Tests override via `XDG_DATA_HOME` for isolation. Use atomic write helpers (tmp+rename) for job history. (001-initial-plan)

- Rust edition 2021 (minimum); recommend Rust 1.70+ (stable) (001-initial-plan)

## Project Structure

```text
src/
tests/
```

## Commands

cargo test [ONLY COMMANDS FOR ACTIVE TECHNOLOGIES][ONLY COMMANDS FOR ACTIVE TECHNOLOGIES] cargo clippy

## Code Style

Rust edition 2021 (minimum); recommend Rust 1.70+ (stable): Follow standard conventions

## Recent Changes
- 001-initial-plan: Added Rust (edition 2021). Recommend Rust 1.70+; project CI will use the latest stable Rust available on Ubuntu LTS runners. + Slint (UI), tokio (async runtime), tokio-tungstenite (async websockets, optional), tungstenite (sync websocket, optional), serialport or `serialport-rs`-compatible crate for serial communication, tracing (logging), anyhow/thiserror for error handling. Feature-gate optional transports (websocket) to keep default build minimal.

- 001-initial-plan: Added Rust edition 2021 (minimum); recommend Rust 1.70+ (stable)

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
