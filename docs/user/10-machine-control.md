# Machine Control

## Overview

The Machine Control area provides manual control of your CNC machine including position readout, jogging, homing, coordinate systems, and overrides.

## Digital Readout (DRO)

The DRO displays the current position of your machine in real-time.

### Position Modes
- **Machine Position (MPos)** — Absolute coordinates from the home/reference point
- **Work Position (WPos)** — Coordinates relative to the active Work Coordinate System offset

### Displayed Axes
- **X, Y, Z** — Standard 3-axis display
- **A** — Optional rotary axis (displayed when a 4+ axis machine is detected)

### Units
- **Millimeters (mm)** — Values shown to 2 decimal places
- **Inches (in)** — Toggle from the unit selector; values auto-convert

### Zero Buttons
- **Zero X / Y / Z** — Set the current position as zero for that axis in the active WCS
- **Zero All** — Set all axes to zero in the active WCS

## Jogging

### Keyboard Jog Controls

The jog keys are on the numeric keypad. They work only while Machine Control has the keyboard
(click in the pane first), never while a text field or dropdown has it.

| Key | Action |
|-----|--------|
| **6** / **4** | Jog X+ / X- |
| **8** / **2** | Jog Y+ / Y- |
| **9** / **3** | Jog Z+ / Z- |

Jog buttons are also available in the UI panel for mouse control.

### Step Sizes

Select the distance per jog step:

| Step | Use Case |
|------|----------|
| 0.001 mm | Ultra-fine positioning |
| 0.01 mm | Fine positioning |
| 0.1 mm | Normal precision moves |
| 1.0 mm | General movement |
| 10.0 mm | Fast positioning |
| 100.0 mm | Rapid traverse |

In imperial mode the presets are 0.0001, 0.001, 0.01, 0.1 and 1 in. A step that is not one of
the presets is kept and shown as its own entry in the list.

### Jog Feed Rate
Set the feed rate for jog moves. The rate is in mm/min (or in/min when in imperial mode).

### Safety
- Jog commands are blocked when the machine is in **Alarm** state
- Jog commands are blocked while a **program is running**
- Soft limits (if enabled) prevent jogging beyond the machine's travel

## Homing

Click the **Home** button to run the homing cycle. This moves the machine to its reference position using limit switches.

- **Home** — Home all axes in the firmware-defined order (**Ctrl+Home**)

After homing, the DRO resets to the machine's reference coordinates.

## Work Coordinate Systems (WCS)

GCodeKit6 supports six work coordinate systems: **G54** through **G59**.

- Select the active WCS from the dropdown in the DRO panel
- Each WCS stores independent X, Y, Z offsets
- **Zero** buttons set offsets so the current position becomes zero in the selected WCS

Use multiple WCS when:
- Machining multiple parts with different origins
- Using fixtures at known locations
- Switching between roughing and finishing setups

## Emergency Stop

Click **E-STOP** or press **Ctrl+Shift+K** (Cmd on macOS) to send a soft reset, which halts all motion. The key works on every page, also while a text field has the keyboard. This puts the machine into alarm state. Use **Unlock** (**Ctrl+U**) to recover.

## Alarm Handling

When the machine enters an alarm state:

1. Read the alarm message in the console to identify the cause
2. Fix the underlying issue (e.g., clear obstructions, check limit switches)
3. Click **Unlock** to clear the alarm
4. Re-home if required by your firmware

## See Also

- [Overrides](12-overrides.md) — Feed rate, spindle, and rapid overrides
- [Console](15-console.md) — Device console
- [Keyboard Shortcuts](90-shortcuts.md) — Complete shortcut reference
