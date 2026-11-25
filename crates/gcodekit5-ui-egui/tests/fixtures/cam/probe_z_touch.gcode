; Z-Touch Probe Routine
; Start (informational): X12.500 Y-30.250 Z-4.000
; All moves are relative to the probe position when this program starts

G21 ; Millimetres
G91 ; Incremental moves from the current position
; Max depth: 20.00 mm, plate thickness: 0.00 mm
G38.2 Z-20.000 F100.0 ; Fast probe down
G0 Z2.000 ; Back off by 2.00 mm
G38.2 Z-2.500 F25.0 ; Slow re-probe
G0 Z5.000 ; Retract to safe height above the surface
G90 ; Restore absolute moves
