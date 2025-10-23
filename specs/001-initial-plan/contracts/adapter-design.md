# Adapter Design: Network Device Adapters

This document outlines implementation notes, reliability strategies, buffering and flow-control decisions for network-based device adapters (TCP/UDP).

Goals
- Provide reliable streaming of G-code over network transports while preserving
  real-time responsiveness and emergency stop semantics.
- Keep adapter API simple for device-specific logic; use shared transport helpers.

API Surface (Rust)
- trait Transport {
  - fn connect(&self) -> Result<Box<dyn Connection>, Error>
  - fn disconnect(&self)
- }

- trait Connection {
  - fn send_line(&mut self, line: &str) -> Result<(), Error>
  - fn flush(&mut self) -> Result<(), Error>
  - fn is_alive(&self) -> bool
  - fn emergency_stop(&mut self) -> Result<(), Error>
- }

Reliability Strategies
- TCP: use built-in congestion control/retransmit. Keep a small write buffer and
  rely on backpressure from the TCP stack. Implement application-level write
  timeouts and connection health checks (heartbeat/ping).
- UDP: UDP is unreliable — implement optional application-level sequence numbers
  and per-packet retries. Use `udp_options.sequence_numbers` and `udp_options.retries`.
  For performance, use a sliding window with sequence acking for critical commands.

Buffering & Flow Control
- Maintain a configurable send-window (default 8 lines) per connection to avoid
  overwhelming device buffers. The window size may be tuned per firmware.
- For TCP: when write returns WouldBlock or BufFull, pause streaming until writable.
- For UDP: maintain sequence numbers and retransmit policy; do not advance window
  until acked if `udp_options.sequence_numbers` is enabled.

Emergency Stop Handling
- Emergency Stop MUST immediately stop sending further lines and attempt to send
  a stop command (if defined for the device) over the current transport.
- For hardware E-stop integration, expose a separate GPIO/IO input in the adapter
  or accept an external signal to the core controller.

Testing & Validation
- Unit tests for connection lifecycle, send_line, and emergency_stop semantics.
- Integration tests using a simulated TCP server and UDP echo server to validate
  ordering, retransmit, and emergency stop responsiveness.

Operational Notes
- Document platform-specific differences (e.g., socket options) in the adapter
  docs. Provide examples of `systemd` service configs for networked deployments.

