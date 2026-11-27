# Console

## Overview

The Console panel provides direct communication with your CNC controller. It displays messages from the device and allows you to send manual commands.

## Message Display

Messages are color-coded by level:

| Level | Description |
|-------|-------------|
| **Info** | Standard responses from the controller (default text color) |
| **Success** | Confirmation messages |
| **Warning** | Non-critical issues |
| **Error** | Failures, faults, or alarm messages |
| **Debug** | Diagnostic messages (dimmed) |

An `error:N` or `ALARM:N` reply is shown with the meaning of its code beside it. Status reports are not logged; they appear in the DRO and status bar instead. The console keeps the newest 5000 messages.

## Header Buttons

- **Clear console** empties the log.
- **Copy last error** copies the newest error or alarm line to the clipboard.

## Sending Commands

Type a command in the input field at the bottom of the console and press **Enter** to send it to the controller.

**Common commands for GRBL**:
- `$$` — View all GRBL settings
- `$#` — View coordinate offsets
- `$I` — View build info
- `?` — Request status report
- `$H` — Home all axes
- `$X` — Clear alarm lock

## Command History

Navigate through previously sent commands using the **Up** and **Down** arrow keys in the command input field, similar to a terminal shell.

## Auto-Scroll

The console auto-scrolls to show the newest messages. Scroll up manually to review history — auto-scroll pauses until you scroll back to the bottom.

## See Also

- [Machine Control](10-machine-control.md) — DRO and manual control
- [Troubleshooting](06-connection-troubleshooting.md) — Diagnosing connection issues
