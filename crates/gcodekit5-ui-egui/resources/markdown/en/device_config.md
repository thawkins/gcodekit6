# Device Config

Device Config is where you **retrieve, inspect, and manage firmware settings** (e.g. GRBL `$$`) and the **capabilities** derived from those settings. The **Firmware Settings** command opens this page.

## What you’ll see
- **Settings**: a searchable, filterable list of `$` settings, grouped by category.
- **Description**: the meaning of the selected setting.

Device details (name/firmware/version) are in a separate window: Machine > **Device Info**.

## Retrieve settings (recommended first step)
1. Connect to the machine in **Machine Control**.
2. Click **Retrieve** to read `$$` from the controller.
3. Use the **Filter:** box and **Category:** filter to locate specific settings.

Retrieved settings are stored so other parts of the app can use them (e.g. max travel, max feed rate, spindle/laser parameters).

## Working with settings
### Search + filter
- Use **Filter:** for setting number, name, or description.
- Use **Category:** to quickly jump to areas like motion limits, acceleration, spindle, etc.

### Edit
- Some settings may be **read-only** depending on firmware/controller.
- Click a writable setting's row to open its **Edit Setting** dialog, enter the value, then click **Apply** to write it to the controller.

## Saving and loading
- **Save** exports the current settings to a file for backup/sharing.
- **Load** imports a settings file so you can compare against a known-good configuration.

## Capabilities (derived)
Some UI behaviors are derived from settings, for example:
- Homing enabled/disabled (e.g. GRBL `$22`)
- Laser mode (e.g. GRBL `$32`)

## Safety notes
Changing firmware settings can immediately affect motion, limits, and safety features.
- If you are unsure about a setting, **save a backup** first.
- After changes, validate limits/homing behavior at low speed.

## Related
- [Device Manager](help:device_manager)
- [Device Console](help:device_console)
- [Machine Control](help:machine_control)
- [Index](help:index)

