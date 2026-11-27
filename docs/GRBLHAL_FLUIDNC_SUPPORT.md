# grblHAL and FluidNC Support

GCodeKit5 drives grblHAL and FluidNC controllers through its GRBL protocol. Both
firmwares speak the GRBL protocol, so a profile set to either one connects,
streams and edits settings the same way as a GRBL machine.

## Device profiles

The device profile's controller type can be `GrblHal` or `FluidNC`
(`ControllerType` in `crates/gcodekit5-devicedb/src/model.rs`). The Device Manager form
lists them as "grblHAL (supported via GRBL)" and "FluidNC (supported via GRBL)".

In the protocol registry (`crates/gcodekit5-communication/src/firmware/protocol_registry.rs`)
grblHAL and FluidNC are **not** marked as supported protocols of their own.
`ControllerProtocol::from_name` maps `grblHAL`, `GrblHal` and `FluidNC` to the
GRBL protocol.

## Connection types

A profile's connection type is one of:

- `Serial` (the default): serial or USB port and baud rate.
- `TCP/IP`: host in `tcp_host`, port in `tcp_port` (default `23`, telnet).
- `WebSocket`: host in `tcp_host`, port in `tcp_port`.

A network profile with a host fills the Machine Control connection form with its
connection type, host, port and timeout.

Example profile fields for a grblHAL machine on the network:

```json
{
  "name": "My grblHAL CNC",
  "controller_type": "GrblHal",
  "connection_type": "TCP/IP",
  "tcp_host": "192.168.1.100",
  "tcp_port": 23
}
```

## Firmware settings

The Firmware Settings window reads settings with `$$` and writes them with
`$nn=value` for GRBL, grblHAL and FluidNC alike.

`GrblFamily::detect` decides which GRBL family the controller runs. grblHAL or
FluidNC named by either source wins:

- the controller's identity (for example grblHAL's `[FIRMWARE:grblHAL]` line from
  `$I`, or a FluidNC banner);
- the active device profile's controller type.

On grblHAL and FluidNC the masks and switches below `$100` are not limited to a
byte (0 to 255), so wider values such as `$10=511` are accepted. Classic GRBL and
an unknown firmware keep the byte limit.

## Maximum axes

`FirmwareCapabilities` records 5 axes for GRBL, 6 for grblHAL and 6 for FluidNC.

## Testing with the grblHAL simulator

The scripts in `scripts/` run `grblHAL_sim` behind a virtual serial port at
`/dev/ttyGRBL`. Creating the link in `/dev` needs sudo. See
[scripts/README.md](../scripts/README.md).

```bash
sudo ./scripts/start-grblhal-sim.sh   # start; creates /dev/ttyGRBL
./scripts/test-grblhal-sim.sh         # send ?, $$, $I and $G
sudo ./scripts/stop-grblhal-sim.sh    # stop socat and remove /dev/ttyGRBL
```

Connect GCodeKit5 to the serial port `/dev/ttyGRBL`. Any baud rate works.

## Troubleshooting

### Network connection failed

- Check that the controller is on the same network.
- Check the IP address and port number.
- Check that the firewall allows the port.
- Test with telnet: `telnet <ip> <port>`.

### grblHAL not detected

- Check that the controller answers `$I` with a `[FIRMWARE:grblHAL]` line.
- Or set the profile's controller type to grblHAL.

## References

- [grblHAL GitHub](https://github.com/grblHAL)
- [grblHAL core](https://github.com/grblHAL/core)
- [FluidNC GitHub](https://github.com/bdring/FluidNC)
- [FluidNC Wiki](http://wiki.fluidnc.com/)
