; Boss Center: 20.00 mm diameter
; Start (informational): X12.500 Y-30.250 Z-4.000
; All moves are relative to the probe position when this program starts

G21 ; Millimetres
G91 ; Incremental moves from the current position
; Start above the approximate boss centre, clear of its top
; Probing 4 points from outside toward center: left, right, front, back
G0 X-12.000 ; Move to left probe position
G38.3 Z-3.000 F100.0 ; Lower beside the boss (stops on contact)
G38.2 X12.000 F100.0 ; Probe left side toward centre
G0 X-1.000 ; Move clear of the boss
G0 Z3.000 ; Raise above the boss
G0 X23.000 ; Move to right probe position
G38.3 Z-3.000 F100.0 ; Lower beside the boss (stops on contact)
G38.2 X-12.000 F100.0 ; Probe right side toward centre
G0 X1.000 ; Move clear of the boss
G0 Z3.000 ; Raise above the boss
G0 X-11.000 Y-12.000 ; Move to front probe position
G38.3 Z-3.000 F100.0 ; Lower beside the boss (stops on contact)
G38.2 Y12.000 F100.0 ; Probe front side toward centre
G0 Y-1.000 ; Move clear of the boss
G0 Z3.000 ; Raise above the boss
G0 Y23.000 ; Move to back probe position
G38.3 Z-3.000 F100.0 ; Lower beside the boss (stops on contact)
G38.2 Y-12.000 F100.0 ; Probe back side toward centre
G0 Y1.000 ; Move clear of the boss
G0 Z3.000 ; Raise above the boss
G0 Y-11.000 ; Return to nominal centre
G90 ; Restore absolute moves
