# Supported Controllers

GCodeKit5 speaks two controller protocols: **GRBL** and **Repetier**. You choose the
controller in the **Controller** field of the device profile (Device Manager, **General**
tab). The app does not pick the protocol for you.

## Controller Choices

| Controller | Status |
|------------|--------|
| GRBL | Supported |
| Repetier | Supported |
| grblHAL | Supported via GRBL |
| FluidNC | Supported via GRBL |
| TinyG | Not supported |
| g2core | Not supported |
| Smoothieware | Not supported |
| Marlin | Not supported |

The list shows every controller type a profile can name, but a profile with a controller
marked "not supported" cannot be saved.

## GRBL

GRBL connections use GRBL's character-counting streaming protocol, real-time commands
(feed hold, cycle start, overrides, soft reset) and `$J=` jog commands. After connecting, the
app sends `$I` and `$$` to read the firmware's identity and settings.

### grblHAL and FluidNC

grblHAL and FluidNC use the GRBL protocol. Some settings behave differently on them, so the
app works out the GRBL variant from two sources: the profile's **Controller** field and the
identity the controller reports (its banner and `$I` reply). A variant named by either source
wins over plain GRBL.

## Repetier

Repetier connections number and checksum every line, keep one command in flight at a time
and replay lines when the firmware asks for a resend. The emergency stop sends `M112`.
After connecting, the app sends `M110` and `M115`.

Probing is not available on Repetier firmware.

## Firmware Mismatch

If the controller identifies itself as a different protocol from the active profile (for
example a Repetier board on a GRBL profile), the app closes the connection and asks you to
change the active machine in the Device Manager.
