# Application Settings

## Overview

Access settings from the **Settings** panel. Changes take effect immediately unless noted otherwise.

## General Settings

| Setting | Description |
|---------|-------------|
| **Default Units** | Metric (mm) or Imperial (inches) for all displays |
| **Theme** | Light, Dark, or System theme |
| **Auto-connect** | Automatically connect to the last-used device on startup |
| **Reconnect Timeout** | Time in seconds before auto-reconnect attempts (default: 10s) |

## Connection Settings

| Setting | Description |
|---------|-------------|
| **Default Connection Type** | Serial, TCP/IP, or WebSocket |
| **Device Type** | CNC, Laser, or Other — affects laser mode ($32) and UI behavior |

## Display Settings

| Setting | Description |
|---------|-------------|
| **DRO Decimal Places** | Number of decimal places shown in the DRO |
| **Grid Spacing** | Default grid spacing for the Designer and Visualizer |
| **Unit Display** | Show units label next to values |

## Keyboard Shortcuts

The **Shortcuts** page of Preferences lists every command, where its keys work (everywhere, or
only while Machine Control, the Visualizer or the Designer has the keyboard) and its keys:

1. Type part of a command's name in **Filter:** to find it, and click it to choose it (the
   buttons under the list act on the chosen command)
2. Click **Change**, then press the new key combination (Escape cancels); it replaces the
   command's first key, or the key you clicked in its row. **Add** gives it one more key
3. Click **Apply**: the new key works at once and shows in the menus

**Clear** removes a command's keys, **Reset** gives one command its default keys back (unless
another command now uses one of them; the reason is shown), and **Reset All** every command. **Cancel** drops the changes.

A key is refused, with the reason shown above the list, when:

- it is the E-STOP (soft reset, Ctrl+Shift+K) or feed hold (`!`, Ctrl+Shift+H) key; those two
  commands cannot be changed;
- a command that moves the machine (jog, home, stream start, cycle start) would use the letter
  of one of those keys;
- a command that moves the machine, other than the jog keys of Machine Control, would get a key
  without Ctrl, Alt or a function key (it would run on every page);
- a jog key is not a plain letter, digit or symbol key (with Ctrl, Alt or a function key it would
  jog while you type in a text field);
- Alt and the letter open a menu in any of the languages;
- another command already uses it where both could run.

A command that moves the machine never runs from its key while a text field has the keyboard,
whatever the key; File > Run only previews the program and is not one of them.

Shortcuts are saved with the other settings. A saved shortcut that breaks one of these rules
(for example after editing the settings file by hand) is replaced by the command's default, and
a notice at start names the commands.

See [Keyboard Shortcuts Reference](90-shortcuts.md) for the full default list.

## Firmware Settings

When connected to a controller, you can view and modify firmware parameters:

### GRBL Settings

- View all `$` settings with descriptions (e.g., `$110` = X max rate)
- Modify individual settings and click **Apply**
- **Export** settings to a text file for backup
- **Import** settings from a file to restore configuration

### Laser Mode

When the device type is set to **Laser**, GCodeKit5 automatically configures `$32=1` (GRBL laser mode) to enable variable power during motion.

## See Also

- [Tool Library & Materials](60-tool-library.md) — Tool and material management
- [Keyboard Shortcuts](90-shortcuts.md) — Shortcut reference
- [Device Setup](04-device-setup.md) — Connection configuration
