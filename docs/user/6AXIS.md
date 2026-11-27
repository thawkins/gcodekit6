# 6-Axis CNC Support Guide

## Overview

GCodeKit6 can drive machines with up to six axes: X, Y and Z plus the rotary axes A, B and C. It shows and jogs the rotary axes and streams programs that use them.

## Supported Axes

| Axis | Name | Rotation Plane | Typical Use |
|------|------|----------------|-------------|
| X | Linear | - | Horizontal movement (left/right) |
| Y | Linear | - | Horizontal movement (front/back) |
| Z | Linear | - | Vertical movement (up/down) |
| A | Rotary | Around X-axis | Rotary table tilt |
| B | Rotary | Around Y-axis | Trunnion/tilt head |
| C | Rotary | Around Z-axis | Rotary table spin |

## Getting Started

### Prerequisites

- A GRBL-family controller (GRBL, grblHAL or FluidNC) with rotary axis support
- Rotary axes properly configured on your controller

### Device Configuration

1. **Select Your Device**: Open the **Device Manager** page and select or add your device
2. **Configure Axis Count**: Set **No of Axes:** to the number of axes your machine has
3. **Set Axis Limits**: Set the X, Y and Z travel limits (mm) in the device's Limits

### Rotary Axes in Machine Control

The DRO shows A, B and C positions (in degrees) when the machine reports them in its status reports. Each rotary row has a **Zero** button and `-` / `+` jog buttons.

## Jogging Rotary Axes

### Step Size

A rotary jog button moves by the rotary step, 1 degree by default. In continuous mode, holding the button jogs until you release it, up to the axis limit (360 degrees by default).

### G-Code Generation

Rotary jogs are sent as GRBL jog commands (`$J=`) with relative moves; a rotary distance is written in degrees with three decimals, for example:

```gcode
$J=G91 G21 A10.000 F1000
```

## Work Coordinate Systems

6-axis machines support work coordinate system offsets for all axes:

- **G54-G59**: Standard work coordinate systems
- **G92**: Temporary offset (all 6 axes)
- **G10 L2 Pn**: Persistent offset storage

Example setting WCS with rotary offsets:

```gcode
G10 L2 P1 X0 Y0 Z0 A0 B90 C0   ; Set G54 with B at 90°
```

## G-Code Compatibility

### Supported Commands

All standard G-code commands work with 6-axis coordinates:

```gcode
G0 X10 Y20 Z5 A45 B0 C0        ; Rapid move
G1 X10 Y20 Z5 A45 B22.5 C0 F1000  ; Linear move
G2 X20 Y10 I5 J0 A90 B45 C0   ; Clockwise arc
G3 X10 Y20 I-5 J10 A0 B0 C180 ; Counter-clockwise arc
```

### Helical Interpolation

6-axis machines support helical moves with simultaneous rotation:

```gcode
G2 X50 Y0 Z-5 I25 J0 A360      ; 360° helix while cutting
```

## Troubleshooting

### Common Issues

**Issue**: Rotary axis doesn't move
- **Check**: Verify the axis is enabled and configured on your controller
- **Check**: Confirm the controller reports the axis in its status reports (the DRO shows it)
- **Check**: Ensure axis limits aren't exceeded

**Issue**: Incorrect rotary positioning
- **Check**: The controller's rotary axis scaling and direction settings
- **Check**: Work coordinate system offsets

## Firmware-Specific Notes

### GRBL
- Maximum 6 axes supported
- Status reports include A/B/C positions
- Use `$10=3` to enable verbose status

### grblHAL
- Native 6-axis support
- Plugin system for custom kinematics
- Network connectivity available

### FluidNC
- YAML configuration
- Web interface for settings
- WiFi and Ethernet support

## Best Practices

1. **Start Small**: Begin with 4-axis before attempting full 6-axis
2. **Test Limits**: Verify axis limits match your machine capabilities
3. **Save Configurations**: Save your controller settings to a file from Firmware Settings for backup
4. **Monitor Temperature**: Rotary axes can heat up during continuous operation
5. **Lubrication**: Keep rotary table bearings properly lubricated

## References

- [GRBL Documentation](https://github.com/gnea/grbl/wiki)
- [grblHAL Features](https://github.com/grblHAL/core/wiki)
- [FluidNC Documentation](https://github.com/bdring/FluidNC/wiki)

## Support

For issues or questions about 6-axis support:

1. Check this documentation first
2. File an issue on GitHub with your machine configuration
