# Frequently Asked Questions

## General

### What CNC controllers does GCodeKit5 support?

GCodeKit5 supports **GRBL** and **Repetier**. **grblHAL** and **FluidNC** are supported via GRBL. You choose the controller in the device profile. See [Supported Controllers](05-supported-controllers.md).

### Is GCodeKit5 free?

Yes. GCodeKit5 is open-source and free to use.

### What operating systems are supported?

Linux, macOS, and Windows are all supported. Linux is the primary development platform.

### What connection methods are available?

GCodeKit5 supports **Serial/USB**, **TCP/IP**, and **WebSocket** connections. See [Device Setup](04-device-setup.md) for details.

## Connection Issues

### Why can't I see my serial port?

- Check that your USB cable is connected and supports data (not charge-only)
- On Linux, add your user to the `dialout` group: `sudo usermod -aG dialout $USER`
- Click **Refresh Ports** to re-scan
- See [Troubleshooting Connections](06-connection-troubleshooting.md)

### Why is my machine in ALARM state?

This is normal after power-on for many GRBL-based controllers. Click **Unlock** or run a **Home** cycle. If the alarm persists, check for triggered limit switches.

### Can I connect over WiFi?

Yes, for controllers that support it (e.g., FluidNC). Use the **WebSocket** or **TCP/IP** connection mode. See [Device Setup](04-device-setup.md).

## Features

### Can I use GCodeKit5 with a laser?

Yes. GCodeKit5 supports laser mode with variable power control (M3/M4 with S values). The CAM tools include a **Laser Image Engraver** for raster images. The device type follows the controller's `$32` (laser mode) report.

### Does it support 4-axis or more?

Yes. GCodeKit5 supports up to 6 axes (X, Y, Z, A, B, C). The DRO automatically displays additional axes when detected.

### Can I import SVG or DXF files?

Yes. The Designer can import **SVG**, **DXF**, and **STL** files. Import via **File → Import** and select the file type.

### Can I make PCBs with GCodeKit5?

Yes. The **Gerber Converter** CAM tool converts Gerber and Excellon files to G-code for isolation routing and drilling.

### How do I calculate speeds and feeds?

Use the built-in **Speeds & Feeds Calculator** in the CAM Tools tab. Select your material and tool, and it calculates the recommended RPM and feed rate.

## Jogging

### What keys do I use to jog?

The default jog keys are on the numeric keypad: **6**/**4** for X, **8**/**2** for Y and **9**/**3** for Z. They work while the Machine Control page has the keyboard. See [Keyboard Shortcuts](90-shortcuts.md) for the full list. All shortcuts are customizable in Settings.

### Why won't my machine jog?

- The machine must be **connected** and not in **Alarm** state
- Jogging is disabled while a **program is running**
- Check that **soft limits** are not blocking the movement

## Troubleshooting

### G-code streaming is slow

- Try a shorter or higher-quality USB cable
- Close other applications that may be using the serial port

### The visualizer is blank or slow

- Ensure your GPU and driver support Vulkan, Metal, DirectX 12 or OpenGL ES 3 (wgpu)
- Update your graphics drivers
- For large G-code files (100k+ lines), rendering may take a few seconds

## Getting Help

- **Built-in Help**: Click the Help button on any panel for context-sensitive documentation
- **GitHub Issues**: [Report bugs](https://github.com/thawkins/gcodekit6/issues)
