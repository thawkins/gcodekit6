# Application Settings

## Overview

Open **Edit > Preferences**. The window has six pages: General, Machine, User Interface, File Processing, Shortcuts and Advanced. **Apply** saves the changes and closes the window; **Cancel** drops them. A Language or "Use system title bar" change takes effect after a restart.

## General

| Setting | Description |
|---------|-------------|
| **Measurement System** | Metric or Imperial for all displays |
| **Feed Rate Units** | The units feed rates are shown in |
| **Default Directory** | The folder file dialogs start in |

## Machine

| Setting | Description |
|---------|-------------|
| **Jog Step Size (Linear)** | Default jog step, mm |
| **Jog Feed Rate** | Jog speed, mm/min |
| **X Axis Limit**, **Y Axis Limit**, **Z Axis Limit** | Machine travel, mm |

## User Interface

| Setting | Description |
|---------|-------------|
| **Theme** | System, Light or Dark |
| **Language** | The interface language, or System |
| **Show About on Startup** | Show the About dialog when the app starts |
| **Grid Major Line Width (px)**, **Grid Minor Line Width (px)** | Grid line widths |
| **Use system title bar** | Use the platform's window title bar (shown only where it changes something) |

## File Processing

| Setting | Description |
|---------|-------------|
| **Preserve Comments** | Keep comments when processing G-code |
| **Arc Segment Length** | Length of the segments arcs are drawn with, mm |
| **Max Line Length** | Maximum G-code line length |

## Advanced

| Setting | Description |
|---------|-------------|
| **3D Stock Removal Visualization** | Enable the 3D stock removal view |
| **STL File Import** | Offer STL files in the Designer's import dialog |

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
- a command that moves the machine and works everywhere (Home, Stream Start) would get a key
  without Ctrl, Alt or a function key (it would run on every page); the jog keys and Cycle
  Start's `~` work only while Machine Control has the keyboard;
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

The **Firmware Settings** command opens the Device Config page, where you can view and modify the controller's parameters:

### GRBL Settings

- **Retrieve** reads all `$` settings from the controller, with descriptions (e.g., `$110` = X max rate)
- Click a setting to edit it, then click **Apply** to write it
- **Save** (Save Settings to File) writes the list to a JSON file for backup
- **Load** (Load Settings from File) reads a saved list for viewing; it does not write the values to the controller

### Laser Mode

The Device Config page's device type selector chooses CNC, Laser or Other. Choosing **Laser** writes `$32=1` (GRBL laser mode) and **CNC** writes `$32=0`; **Other** leaves `$32` alone. The selector also follows the `$32` value the controller reports.

## See Also

- [Tool Library & Materials](60-tool-library.md) — Tool and material management
- [Keyboard Shortcuts](90-shortcuts.md) — Shortcut reference
- [Device Setup](04-device-setup.md) — Connection configuration
