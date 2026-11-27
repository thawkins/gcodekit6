# Device Console

The Device Console shows raw lines received from the controller and messages sent by GCodeKit6.

## What you will see
- Connection events (connected/disconnected)
- Startup strings (e.g. `Grbl 1.2h ...`)
- Status lines, responses (`ok`), and errors (`error:n`)

## Errors
When a GRBL line includes `error:n`, the console will also show the decoded meaning after the raw text.

## Tips
- **Copy last error** copies the newest error or alarm line.
- **Clear console** empties the log.

## Related
- [Machine Control](help:machine_control)
- [Device Config](help:device_config)
- [Index](help:index)

