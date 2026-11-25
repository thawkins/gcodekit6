; Bore Center: 20.00 mm diameter
; Start (informational): X12.500 Y-30.250 Z-4.000
; All moves are relative to the probe position when this program starts

G21 ; Millimetres
G91 ; Incremental moves from the current position
; Start above the approximate bore centre
; Probing 4 points: left, right, front, back
G38.3 Z-3.000 F100.0 ; Lower into the bore (stops on contact)
G38.2 X-12.500 F100.0 ; Probe left wall
G0 X10.000 ; Back to nominal centre
G38.2 X12.500 F100.0 ; Probe right wall
G0 X-10.000 ; Back to nominal centre
G38.2 Y-12.500 F100.0 ; Probe front wall
G0 Y10.000 ; Back to nominal centre
G38.2 Y12.500 F100.0 ; Probe back wall
G0 Y-10.000 ; Back to nominal centre
G0 Z3.000 ; Raise back to start height
G90 ; Restore absolute moves
