# Touch Probe User Guide

## Overview

GCodeKit5 includes touch probe routines for work coordinate setup and tool length measurement. This guide covers the hardware requirements, safety procedures, and step-by-step instructions for each probing routine.

## Table of Contents

1. [Hardware Requirements](#hardware-requirements)
2. [Safety Warnings](#safety-warnings)
3. [Quick Start](#quick-start)
4. [Probe Routines](#probe-routines)
   - [Z-Touch (Surface Probe)](#z-touch-surface-probe)
   - [Edge Find](#edge-find)
   - [Corner Find](#corner-find)
   - [Bore Center](#bore-center)
   - [Boss Center](#boss-center)
   - [Tool Length](#tool-length)
5. [Common Parameters](#common-parameters)
6. [Troubleshooting](#troubleshooting)

---

## Hardware Requirements

### Touch Probe Types

| Type | Description | Connection |
|------|-------------|------------|
| **Touch Probe** | Spring-loaded stylus with electrical contact | Z-probe input pin on controller |
| **Tool Length Setter** | Fixed plate on machine bed | Same Z-probe input or dedicated second pin |
| **Touch Plate** | Conductive plate laid on the work | Z-probe input pin on controller |

### Controller Compatibility

| Firmware | Probe Support | Notes |
|----------|---------------|-------|
| **GRBL 1.1** | Yes | Standard `G38.2` / `G38.3` commands |
| **grblHAL** | Yes | Supported through the GRBL protocol |
| **FluidNC** | Yes | Supported through the GRBL protocol |
| **Repetier** | No | "Probing is not available on Repetier firmware." |

### Wiring

Connect your probe to the controller's Z-probe input:

- **Normally Open (NO)**: Circuit closes when probe touches
- **Normally Closed (NC)**: Circuit opens when probe touches (safer - fail-safe)

> **Important**: Check your controller manual for the correct probe pin. Incorrect wiring can damage the controller or probe.

---

## Safety Warnings

**READ BEFORE USING PROBE FUNCTIONS**

1. **Emergency Stop**: Always have your hand near the E-Stop button when running probe cycles
2. **Safe Height**: Always verify safe height is set correctly before probing
3. **Probe Direction**: Ensure probe moves TOWARD the workpiece, not away
4. **Max Depth**: Set reasonable max probe depth to prevent crashes
5. **Retract**: Probe routines include automatic retract - do not interrupt
6. **Spindle**: Ensure spindle is OFF and tool is stopped before probing
7. **Clearance**: Ensure probe stylus has clearance for moves

**Before First Use:**
- Test probe trigger by hand: the page's **Probe:** line shows Triggered or Not triggered
- Start with slow feed rates

---

## Quick Start

### Basic Z-Touch Probe

1. **Connect** to the machine, with your touch probe wired to the controller
2. **Jog** to approximately 10 mm above the workpiece surface
3. Open **CAM Tools > Probe Tools**
4. Select the **Z-Touch** routine
5. Verify **Safe Height:** (default: 5 mm)
6. Click **Probe**
7. The probe will:
   - Probe down until it triggers
   - Back off and re-probe slowly
   - Retract to the safe height
   - Show the result under **Results**
8. Click **Apply to WCS** to write the result into the **Target WCS**

**Stop Probe** stops a run in progress. **Generate**, **Send to Editor** and **Preview** produce the routine's program without running it.

A run starts only when the machine is connected and Idle, no job is running, and the probe input is not already triggered.

### Quick Probe from Machine Control

For fast probing at the current position:
1. Go to the **Machine Control** page
2. Click **Probe Z** for a Z touch down to 20 mm, or one of the **Edge:** buttons (X-, X+, Y-, Y+) for an edge find 10 mm along that axis
3. Both run at 100 mm/min and write no work offset

---

## Probe Routines

All routines move in incremental mode (`G91`) from wherever the probe is when the program starts. Position the probe before running.

### Z-Touch (Surface Probe)

**Purpose**: Find the Z=0 surface of your workpiece

**Parameters**:
| Parameter | Default | Description |
|-----------|---------|-------------|
| Safe Height: | 5 mm | Retract height above the surface |
| Max Probe Depth | 20 mm | Maximum distance to probe down |
| Fast Feed Rate: | 100 mm/min | Initial probe speed |
| Slow Feed Rate: | 25 mm/min | Re-probe speed for accuracy |
| Backoff Distance: | 2 mm | Distance to back off before re-probe (0 skips the re-probe) |
| Plate Thickness: | 0 mm | Thickness of a touch plate laid on the work |

**Procedure**:
1. Position probe above the surface (or touch plate), within the max depth
2. Run Z-Touch routine
3. Probe touches surface, backs off, re-probes slowly
4. Result shows trigger position
5. Apply to WCS sets Z=0 at the surface (under the plate)

**G-code Generated** (defaults, metric, comment header left out):
```gcode
G21 ; Millimetres
G91 ; Incremental moves from the current position
G38.2 Z-20.000 F100.0 ; Fast probe down
G0 Z2.000 ; Back off by 2.00 mm
G38.2 Z-2.500 F25.0 ; Slow re-probe
G0 Z5.000 ; Retract to safe height above the surface
G21 ; Restore millimetres
G90 ; Restore absolute moves
```

The last two lines restore your units (`G20` when you work in inches) and absolute mode. With **Auto-Update WCS** on, a `G10 L20` line writes the result into the Target WCS.

---

### Edge Find

**Purpose**: Find an edge on one axis with high precision

**Parameters**:
| Parameter | Default | Description |
|-----------|---------|-------------|
| Probe Axis | X Axis | Axis to probe (X, Y, or Z) |
| Probe Direction | Negative | Direction to probe |
| Probe Distance | 10 mm | Distance to probe |
| Probe Depth: | 3 mm | How far the probe is lowered before probing sideways |
| Probe Tip Radius: | 0 mm | Stylus ball radius, compensated in the result |
| Plate Thickness: | 0 mm | Touch plate thickness |
| Fast Feed Rate: / Slow Feed Rate: | 100 / 25 mm/min | Probe speeds |
| Backoff Distance: | 2 mm | Distance to back off |

**Procedure**:
1. Position probe beside the edge, at a safe height above the work
2. Select axis and direction
3. Run Edge Find
4. Probe lowers, finds the edge, backs off, re-probes slowly, and returns to the start height
5. Result shows edge position

**Tip**: For best accuracy:
- Use slow feed rate for final probe
- Ensure probe is perpendicular to edge
- Clean surface for electrical contact

---

### Corner Find

**Purpose**: Find the intersection of two edges (an inside corner)

**Parameters**:
| Parameter | Default | Description |
|-----------|---------|-------------|
| Corner | X-min / Y-min | Which corner to locate: X-min / Y-min, X-max / Y-min, X-min / Y-max, X-max / Y-max |
| Probe Distance | 10 mm | Distance along each edge |
| Probe Depth: | 3 mm | How far the probe is lowered before probing sideways |
| Probe Tip Radius: | 0 mm | Stylus ball radius, compensated in the result |
| Fast Feed Rate: / Slow Feed Rate: | 100 / 25 mm/min | Probe speeds |

**Procedure**:
1. Position probe inside the corner, at a safe height above the work
2. Select the corner
3. Run Corner Find
4. Probe finds the X edge, then the Y edge
5. Computed corner shown in results

---

### Bore Center

**Purpose**: Find the center of an internal circular feature (hole)

**Parameters**:
| Parameter | Default | Description |
|-----------|---------|-------------|
| Diameter: | 20 mm | Approximate bore diameter |
| Probe Depth: | 3 mm | How far the probe is lowered into the bore |
| Probe Tip Radius: | 0 mm | Stylus ball radius, compensated in the measured diameter |
| Fast Feed Rate: / Slow Feed Rate: | 100 / 25 mm/min | Probe speeds |

**Procedure**:
1. Position probe above the approximate bore center
2. Enter approximate bore diameter
3. Run Bore Center
4. Probe touches 4 points: left, right, front, back
5. Center and measured diameter shown in results

Auto-Update WCS does not set bore centres; use **Apply to WCS**.

---

### Boss Center

**Purpose**: Find the center of an external circular feature (pin/boss)

**Parameters**: Same as Bore Center

**Procedure**:
1. Position probe above the approximate boss center, clear of the top of the boss
2. Enter approximate boss diameter
3. Run Boss Center
4. Probe touches 4 points from outside
5. Center and measured diameter shown in results

**Difference from Bore Center**: Probe approaches from outside rather than inside.

---

### Tool Length

**Purpose**: Measure tool length using a fixed tool setter

**Parameters**:
| Parameter | Default | Description |
|-----------|---------|-------------|
| Setter Plate X | 100 mm | X position of the setter (written in the program header as a reminder) |
| Setter Plate Y | 100 mm | Y position of the setter (written in the program header as a reminder) |
| Setter Plate Z | -50 mm | Machine Z at which the reference tool triggers the setter |
| Safe Height: | 5 mm | Retract height |
| Max Probe Depth | 20 mm | Maximum probe depth |
| Fast Feed Rate: | 100 mm/min | Probe speed |

**Procedure**:
1. Install tool to measure
2. Jog the tool tip above the tool setter, within the max depth (the routine does not move to the setter)
3. Run Tool Length routine
4. Probes down until trigger
5. The result is trigger Z - Setter Plate Z

**After Measurement**:
- Apply it with `G43.1 Z<offset>` (positive for a tool longer than the reference tool)
- Re-measure the reference tool to update Setter Plate Z whenever the setter is moved

---

## Common Parameters

The Probe Tools page shows these for every routine, with the routine's own settings under **Routine Settings**:

| Setting | Default | Description |
|---------|---------|-------------|
| Fast Feed Rate: | 100 mm/min | Fast probe rate |
| Slow Feed Rate: | 25 mm/min | Slow probe rate |
| Safe Height: | 5 mm | Retract height (used by Z-Touch and Tool Length) |
| Backoff Distance: | 2 mm | Backoff distance |
| Max Probe Depth | 20 mm | Maximum probe depth |
| Target WCS | G54 | The WCS (G54-G59) that results are written into |
| Auto-Update WCS | Off | Write the result into the Target WCS (`G10 L20`) from the generated G-code; Z-Touch, Edge Find and Corner Find only |

**Load** and **Save** keep the page's values as a preset file.

---

## Troubleshooting

### Probe Does Not Trigger

| Symptom | Cause | Solution |
|---------|-------|----------|
| Probe moves to max depth | No electrical contact | Check wiring, clean probe tip |
| **Probe:** never shows Triggered | Probe not detected | Check controller configuration |
| Intermittent trigger | Poor contact | Clean surface, check probe spring |

### Incorrect Positions

| Symptom | Cause | Solution |
|---------|-------|----------|
| Position offset | Probe tip diameter | Set **Probe Tip Radius:** |
| Inconsistent results | Machine backlash | Check the machine's mechanics |
| Wrong axis | Wrong routine selected | Verify axis selection |

### Error Messages

| Error | Meaning | Solution |
|-------|---------|----------|
| "Probe failed" | The run did not complete; the reason follows | Clear any alarm (`$X`); absolute mode (`G90`) is restored once the machine is Idle |
| "The probe input is already triggered." | The probe reads as touching before the run | Free the probe, check wiring |
| "Probe aborted: the machine was disconnected." | The connection closed during the run | Reconnect and run again |
| "Probing is not available on Repetier firmware." | The device uses Repetier | Use a GRBL-family controller |

---

## Tips for Best Results

1. **Calibration**: Measure your stylus ball and enter it as **Probe Tip Radius:**
2. **Cleanliness**: Keep probe tip and workpiece clean
3. **Speed**: Use slower feeds for better accuracy
4. **Repetition**: Run routine 2-3 times and average results
5. **Temperature**: Allow machine to warm up before precision work
6. **Documentation**: Save probe settings as a preset for repeat setups

---

## Language Support

The probing interface is translated with the rest of the app:
- English
- German (Deutsch)
- Spanish (Español)
- French (Français)
- Italian (Italiano)
- Portuguese (Português)

---

## Safety Checklist

Before each probing session:

- [ ] Spindle is OFF
- [ ] Emergency stop is accessible
- [ ] Safe height is appropriate
- [ ] Max depth won't cause crash
- [ ] Probe is clean and functioning
- [ ] Workpiece is secure
- [ ] Clearance for moves verified
