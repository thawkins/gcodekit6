# ADR-009: Communication Protocol Architecture

## Status
Accepted (amended 2026-11)

Amended 2026-11-28: kept the trait-abstraction decision; replaced the design with the one in the code (blocking I/O on a worker thread, one communicator with serial, TCP and WebSocket port backends).

## Date
2026-02-18

## Context
GCodeKit6 must communicate with CNC controllers over multiple transport layers (USB serial, TCP/IP, WebSocket) and support multiple firmware protocols (GRBL, potentially others in the future). The communication must be non-blocking to keep the UI responsive, handle connection loss gracefully, and support command buffering for streaming G-code files.

## Decision
Implement a **trait-based abstraction** in the `gcodekit6-communication` crate, driven from a worker thread so the UI never blocks on I/O.

### Transport Layer
`crates/gcodekit6-communication/src/communication/mod.rs`:

```rust
pub trait Communicator: Send + Sync {
    fn connect(&mut self, params: &ConnectionParams) -> gcodekit6_core::Result<()>;
    fn disconnect(&mut self) -> gcodekit6_core::Result<()>;
    fn is_connected(&self) -> bool;
    fn send(&mut self, data: &[u8]) -> gcodekit6_core::Result<usize>;
    fn receive(&mut self) -> gcodekit6_core::Result<Vec<u8>>;
    // plus send_command, connection_params, set_connection_params and others
}
```

**Implementations:**
- `SerialCommunicator`: the real transport. It opens a port backend chosen by `ConnectionParams::driver`: `RealSerialPort` (USB serial via the `serialport` crate, `serial.rs`), `RealTcpPort` (`tcp.rs`) or `RealWebSocketPort` (`websocket.rs`). Build TCP and WebSocket parameters with `ConnectionParams::tcp` and `ConnectionParams::websocket`.
- `NoOpCommunicator`: testing and simulation.

### Acknowledgements
`ReplyLedger` (`communication/reply_ledger.rs`) records one entry per line written, so each `ok` or `error:N` is matched to the line it answers.

### Firmware
- `firmware/protocol_registry.rs` maps a controller type to its protocol profile (init commands, detection banners, whether it is supported).
- GRBL and Repetier handlers live under `firmware/`. GRBL response lines (`ok`, `error:N`, `ALARM:N`, `<status>`) are classified in `crates/gcodekit6-app/src/machine/grbl_stream.rs`.

### Threading
- `MachineController` (`crates/gcodekit6-app/src/machine/`) runs all I/O on one `std::thread` worker that owns the communicator (`worker.rs`).
- Commands go to the worker and `MachineEvent`s come back over `std::sync::mpsc` channels; the UI polls the events each frame.

## Consequences

**Positive:**
- New transports only need a port backend or a `Communicator` implementation
- New firmware protocols only need a parser module
- `NoOpCommunicator` enables full integration testing without hardware
- Non-blocking design keeps UI responsive during long streaming operations

**Negative:**
- Trait objects add a layer of indirection
- A worker thread and channels add complexity compared with I/O on the UI thread
- Serial port handling varies across OS (Linux udev, macOS IOKit, Windows COM)
- Connection state machine must handle many edge cases (disconnect during stream, alarm during jog)

## Alternatives Considered
- **Direct serial I/O** (no trait abstraction): Simpler but locks out TCP/WebSocket support
- **Blocking I/O on the UI thread**: Simpler code but freezes the UI during communication
- **Plugin-based firmware** (dynamic loading): Maximum flexibility but overkill for current needs
