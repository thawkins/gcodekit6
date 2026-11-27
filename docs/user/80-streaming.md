# Streaming G-Code

## Overview

Streaming sends your G-code program to the CNC controller line by line, managing the communication buffer for smooth execution.

## Starting a Job

1. Load a G-code file in the [Editor](20-gcode-editor.md) or generate G-code from a [CAM Tool](50-camtools.md)
2. Ensure the machine is **connected** and in **Idle** state
3. Click **Send** to begin streaming

## Job Controls

| Control | Action |
|---------|--------|
| **Pause** | Feed hold and stop sending lines. The machine decelerates to a stop and keeps its position |
| **Resume** | Cycle start and send lines again, from the pause point |
| **Stop** | Soft reset and drop the rest of the program |

The Pause, Resume and Stop buttons have no default key. **Feed Hold** is `!` or
**Ctrl+Shift+H** (Cmd on macOS) on every page. See [Keyboard Shortcuts](90-shortcuts.md).

## Progress Monitoring

While streaming, monitor progress through:

- **Job Control** — A progress bar of the lines the controller acknowledged, out of the total
- **Status bar** — Percentage complete, elapsed time and estimated remaining time
- **Visualizer** — Acknowledged lines drawn in the accent colour; the tool marker follows the
  reported position

## Feed Rate and Spindle Overrides

Adjust [Overrides](12-overrides.md) in real-time during streaming to modify feed rate, spindle speed, or rapid speed without pausing the job.

## Safety During Streaming

- Keep your hand near the **E-STOP** button or **Ctrl+Shift+K** during cutting
- Start with **Rapid Override at 50%** for first runs of new programs
- Jogging is disabled while a program is running

## See Also

- [Machine Control](10-machine-control.md) — Manual control and DRO
- [G-Code Editor](20-gcode-editor.md) — Viewing and editing programs
- [Overrides](12-overrides.md) — Real-time speed adjustments
- [Safety](85-safety-diagnostics.md) — E-STOP, feed hold and jog limits
