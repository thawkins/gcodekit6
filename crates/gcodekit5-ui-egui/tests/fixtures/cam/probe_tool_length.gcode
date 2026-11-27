; Tool Length Measurement
; Start (informational): X12.500 Y-30.250 Z-4.000
; All moves are relative to the probe position when this program starts

G21 ; Millimetres
G91 ; Incremental moves from the current position
; Position the tool tip above the tool setter (at X100.000 Y100.000) before running
; Reference: machine Z-50.000 = trigger height of the reference tool; tool length offset = trigger Z - reference
G38.2 Z-20.000 F100.0 ; Probe down onto the setter
G0 Z5.000 ; Retract to safe height above the setter
G21 ; Restore millimetres
G90 ; Restore absolute moves
