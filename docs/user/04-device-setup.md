# Device Setup

## Device Profiles

A device profile describes one machine: its controller, connection, dimensions and
capabilities. Open the **Device Manager** page to create and edit profiles. Each profile has
four tabs: **General**, **Connection**, **Dimensions** and **Capabilities**. Click **Save** to
store your changes.

On the **General** tab, the **Controller** field sets the firmware the app talks to. See
[Supported Controllers](05-supported-controllers.md).

To use a profile, select it and click **Set Active**.

## Connection Types

The connection is set on the **Connection** tab of the device profile:

- **Connection Type**: Serial, TCP/IP or WebSocket
- **Port** and **Baud Rate** (Serial)
- **TCP Host** and **TCP Port** (TCP/IP and WebSocket)
- **Timeout (ms)**
- **Auto Reconnect**

The **Connection** section of the Machine Control page uses the active profile as follows:

- **Auto Reconnect** always comes from the active profile.
- For a TCP/IP or WebSocket profile with a host, the connection type, host, port and timeout
  come from the profile, and Machine Control shows **Host** and **Port** fields.
- Otherwise Machine Control shows the serial port list and a **Refresh ports** button.

### Serial / USB

The most common connection method for GRBL and similar controllers.

1. Connect your CNC controller via USB
2. In the **Connection** section of Machine Control, click the **Refresh ports** button to
   rescan
3. Select the port from the list (e.g., `/dev/ttyUSB0` on Linux, `COM3` on Windows)
4. Click **Connect**

> **Linux users**: You may need to add your user to the `dialout` group for serial port access. See [Troubleshooting](06-connection-troubleshooting.md).

### TCP/IP

For network-connected controllers (e.g., FluidNC with Ethernet, or controllers behind a
serial-to-TCP bridge).

1. In the device profile, set **Connection Type** to **TCP/IP** and enter the **TCP Host**
   (e.g., `192.168.1.100`) and **TCP Port**
2. In Machine Control, check the **Host** and **Port** fields
3. Click **Connect**

### WebSocket

For WiFi-enabled controllers such as FluidNC.

1. In the device profile, set **Connection Type** to **WebSocket** and enter the controller's
   **TCP Host** and **TCP Port** (FluidNC's WebSocket port is usually `81`)
2. In Machine Control, check the **Host** and **Port** fields
3. Click **Connect**

The app builds the WebSocket address from the host and port. Only plain WebSocket is
supported, not secure (TLS) WebSocket.

## Auto Reconnect

Enable **Auto Reconnect** in the device profile to restore a dropped connection
automatically. The app makes up to 10 attempts, waiting 1 s before the first, 2 s before the
second and 5 s before each later one. A program that was streaming is stopped and is not
resumed after the reconnect.

## Number of Axes

Set the number of axes your machine has in the **No of Axes** field on the **Capabilities**
tab of the device profile. The value must be a whole number from 1 to 6; any other value
shows an "Invalid No of Axes" error when you save. The default is 3. If no profile is active,
the CAM tools assume 3 axes.

For a machine without a Z axis, such as a pen plotter, set **No of Axes** to `2` and click
**Set Active**. With fewer than 3 axes, the Designer, **Tabbed Box Maker** and Gerber converter
leave Z moves (safe-height retracts, plunges and step-downs) out of the G-code.
**Drill Press** and **Spoilboard Surfacing** need a Z axis: with fewer than 3
axes they refuse to generate and show "This tool needs at least 3 axes (X, Y, Z); the active
device has fewer."

## Verifying Connection

After connecting, you should see:

- **Machine state** in the status bar (Idle, Alarm, Run, Hold, etc.)
- **Position coordinates** in the DRO (Digital Readout)
- **Firmware identification** (e.g., "Grbl 1.1h", "FluidNC v3.x")
- **Startup messages** in the Console panel

## Troubleshooting

See [Troubleshooting Connections](06-connection-troubleshooting.md) for common issues.
