# research.md

Feature: GCodeKit6 - Initial MVP

Date: 2025-10-23

## Unknowns identified

1. Recommended minimum Rust compiler version
2. Realistic performance goals (latency/throughput) for serial command delivery
3. Best serial communication crate for cross-platform reliability
4. Recommended location for persistent app data across platforms


## Decision 1: Rust version
Decision: Use Rust stable, minimum supported version 1.70.0 and target edition 2021.

Rationale: Rust 1.70+ provides modern language features, stable ecosystem crates,
and wide availability on CI. Using stable ensures reproducible builds.

Alternatives considered:
- Rust 1.64: older but still maintained; rejected to avoid missing features in
  newer crates.


## Decision 2: Performance goals
Decision: Set initial p95 latency target < 50ms for serial command round-trips
in nominal conditions; emergency stop response < 200ms.

Rationale: These goals are achievable on common desktop hardware and provide
strict but realistic bounds for responsiveness. We will measure and refine
during Phase 1 integration tests.

Alternatives considered:
- More aggressive target (<20ms) considered but may be unrealistic on USB-serial
  adapters and low-power devices.


## Decision 3: Serial communication crate
Decision: Use the `serialport` crate (https://crates.io/crates/serialport) as the
primary cross-platform serial library.

Rationale: `serialport` is widely used, supports Windows/macOS/Linux, and
provides synchronous and asynchronous patterns when combined with `tokio`.

Alternatives considered:
- `mio-serial` — lower-level, more complex; rejected for initial MVP.


## Decision 4: Persistent data location
Decision: Use platform-appropriate locations via `dirs` crate (`dirs-next`):
- Linux: `~/.local/share/gcodekit6/`
- macOS: `~/Library/Application Support/gcodekit6/`
- Windows: `%APPDATA%\gcodekit6\`

Rationale: Using `dirs` ensures cross-platform correctness and user expectations.


## Research tasks (Phase 0)
- R001 Verify `serialport` compatibility with `tokio` and async patterns
- R002 Benchmark emergency stop latency on a USB-serial adapter (simulated)
- R003 Evaluate file format parsers for G-code previewing (SVG -> toolpath support if needed)


## Output
All NEEDS CLARIFICATION items resolved for Phase 1 design.
