; Edge Find: X-axis negative
; Start (informational): X12.500 Y-30.250 Z-4.000
; All moves are relative to the probe position when this program starts

G21 ; Millimetres
G91 ; Incremental moves from the current position
; Probe distance: 10.00 mm, backoff: 2.00 mm, probe depth: 3.00 mm
G38.3 Z-3.000 F100.0 ; Lower beside the edge (stops on contact)
G38.2 X-10.000 F100.0 ; Fast probe
G0 X2.000 ; Back off by 2.00 mm
G38.2 X-2.500 F25.0 ; Slow re-probe
G0 X2.000 ; Move clear of the edge
G0 Z3.000 ; Raise back to start height
G21 ; Restore millimetres
G90 ; Restore absolute moves
