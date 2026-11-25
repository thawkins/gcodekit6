; Corner Find: X-max/Y-max
; Start (informational): X12.500 Y-30.250 Z-4.000
; All moves are relative to the probe position when this program starts

G21 ; Millimetres
G91 ; Incremental moves from the current position
; Probe distance: 10.00 mm per edge, probe depth: 3.00 mm
G38.3 Z-3.000 F100.0 ; Lower inside the corner (stops on contact)
; Probe X edge
G38.2 X10.000 F100.0 ; Fast probe
G0 X-2.000 ; Back off by 2.00 mm
G38.2 X2.500 F25.0 ; Slow re-probe
G10 L20 P1 X-0.500 ; Work X0 at the X edge
G0 X-2.000 ; Move clear of the X edge
; Probe Y edge
G38.2 Y10.000 F100.0 ; Fast probe
G0 Y-2.000 ; Back off by 2.00 mm
G38.2 Y2.500 F25.0 ; Slow re-probe
G10 L20 P1 Y-0.500 ; Work Y0 at the Y edge
G0 Y-2.000 ; Move clear of the Y edge
G0 Z3.000 ; Raise back to start height
G90 ; Restore absolute moves
