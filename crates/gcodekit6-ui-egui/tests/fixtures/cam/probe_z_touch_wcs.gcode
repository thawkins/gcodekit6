; Z-Touch Probe Routine
; Start (informational): X12.500 Y-30.250 Z-4.000
; All moves are relative to the probe position when this program starts

G21 ; Millimetres
G91 ; Incremental moves from the current position
; Max depth: 20.00 mm, plate thickness: 1.50 mm
G38.2 Z-20.000 F100.0 ; Fast probe down
G0 Z1.250 ; Back off by 1.25 mm
G38.2 Z-1.750 F25.0 ; Slow re-probe
G10 L20 P2 Z1.500 ; Work Z0 at the surface under the plate
G0 Z5.000 ; Retract to safe height above the surface
G21 ; Restore millimetres
G90 ; Restore absolute moves
