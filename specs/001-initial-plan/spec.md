# Feature Specification: GCodeKit6 - Initial MVP

**Feature Branch**: `001-initial-plan`
**Created**: 2025-10-23
**Status**: Draft
**Input**: User description: "Create a desktop application that controls fabrication machines like CNC machines, laser engravers/cutters and 3D Printers. Use the contents of AGENTs.md to provide governance rules. Base the requirements on the specification of \"Universal Gcode Sender\""

## Clarifications

### Session 2025-10-23
- Q: Target firmware support → A: GRBL + Smoothieware + TinyG + G2core

## User Scenarios & Testing (mandatory)

### User Story 1 - Connect to a Device (Priority: P1)
A user can discover and connect to a serial/USB-attached GRBL-compatible device.

Why this priority: Without connecting to a device, sender functionality is useless.

Independent Test:
- Connect to a simulated device or a real device and verify connection status appears in UI.

Acceptance Scenarios:
1. Given no device connected, when user opens the Connect dialog and selects a serial port, then the app establishes a connection and displays device firmware/version.

### User Story 2 - Send G-code Files (Priority: P1)
A user can open a G-code file, preview it, and stream it to the connected device with progress reporting and pause/resume support.

Acceptance Scenarios:
1. Given a connected device, when user opens a `.gcode` file and clicks Send, then the file is streamed line-by-line to the device and progress is shown.

### User Story 3 - Emergency Stop & Safety (Priority: P1)
The app MUST provide an Emergency Stop button that immediately halts sending and issues device-stop commands.

Acceptance Scenarios:
1. When Emergency Stop clicked, streaming stops and device state is set to safe.

## Edge Cases
- Device disconnects during streaming
- Corrupt G-code lines
- High-feedrate commands causing buffer overruns

## Requirements (mandatory)

### Functional Requirements
- FR-001: System MUST allow users to discover and connect to serial/USB ports (P1)
- FR-002: System MUST allow loading and parsing of `.gcode` files (P1)
- FR-003: System MUST stream G-code reliably with pause/resume and progress (P1)
- FR-004: System MUST present Emergency Stop (P1)
- FR-005: System MUST log all communication with timestamped entries (P2)
- FR-006: System SHOULD support plugins for device-specific features (P3)
 - FR-007: System MUST support at minimum the following firmwares: GRBL, Smoothieware, TinyG, and G2core (P1)

### Key Entities
- Device: serial port, firmware, capabilities
- Job: G-code file, progress, history
- Settings: port configs, feed/speed defaults

## Success Criteria (mandatory)

### Measurable Outcomes
- SC-001: User can connect to a GRBL device within 2 minutes of starting the app
- SC-002: File streaming completes without errors in nominal test cases
- SC-003: Emergency Stop response time < 200ms under test

