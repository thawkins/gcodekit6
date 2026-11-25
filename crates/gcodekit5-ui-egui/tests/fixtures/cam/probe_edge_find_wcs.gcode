; Edge Find: Y-axis positive
; Start (informational): X12.500 Y-30.250 Z-4.000
; All moves are relative to the probe position when this program starts

G21 ; Millimetres
G91 ; Incremental moves from the current position
; Probe distance: 15.00 mm, backoff: 2.00 mm, probe depth: 4.00 mm
G38.3 Z-4.000 F100.0 ; Lower beside the edge (stops on contact)
G38.2 Y15.000 F100.0 ; Fast probe
G0 Y-2.000 ; Back off by 2.00 mm
G38.2 Y2.500 F25.0 ; Slow re-probe
G10 L20 P3 Y-1.000 ; Work zero at the edge (tip compensated)
G0 Y-2.000 ; Move clear of the edge
G0 Z4.000 ; Raise back to start height
G90 ; Restore absolute moves
