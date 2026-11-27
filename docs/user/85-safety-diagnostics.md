# Safety

## Overview

GCodeKit5 has three safety controls: E-STOP, Feed Hold and jog travel limits. Their keys are
fixed and cannot be changed in Settings. `Ctrl` is `Cmd` on macOS.

## E-STOP

Click the **E-STOP** button or press **Ctrl+Shift+K** to send a soft reset to the controller.
This halts all motion and stops any running stream.

- The key works on every page, also while a text field, a menu or a window has the keyboard
- Escape is not bound to E-STOP, so closing a dropdown never resets the machine
- To recover, click **Unlock** (**Ctrl+U**). You may need to re-home the machine afterwards

## Feed Hold

Feed hold pauses motion and keeps the machine position, so the job can resume cleanly.

- Press `!` or **Ctrl+Shift+H** on any page. `Ctrl+Shift+H` also works while a text field has
  the keyboard; `!` is typed into the field instead
- Resume with **Resume** in Job Control, or **Shift+`** (Cycle Start) while Machine Control has
  the keyboard

## Jog Travel Limits

A continuous jog is clamped to the travel left in that direction. With no known machine
position, or no travel left, nothing moves and the pane says why. Jogging is enabled only while
the machine is connected, not streaming, and reports `Idle` or `Jog`.

Soft limits in the firmware itself are set with `$20` and `$130`-`$132` on GRBL, in
**Firmware Settings**.

## See Also

- [Machine Control](10-machine-control.md) — DRO, jogging, and alarm handling
- [Streaming G-Code](80-streaming.md) — Running programs safely
- [Keyboard Shortcuts](90-shortcuts.md) — Complete shortcut reference
- [Console](15-console.md) — Device communication log
