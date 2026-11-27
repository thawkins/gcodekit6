; Drill Press Toolpath
; Hole Diameter: 10.000 mm
; Tool Diameter: 6.000 mm
; Depth: 0.000 to -10.000 mm
; Center: X100.000 Y100.000
G21 ; Set units to millimeters
G90 ; Absolute positioning
G17 ; XY plane selection
; Runs in the active work coordinate system: set the X/Y/Z work zero first
G0 Z5.00 ; Move to safe height
M3 S10000 ; Start spindle
G0 X100.000 Y100.000 ; Move to hole center
; Helical interpolation cycle
G0 X102.000 Y100.000
G1 Z0.000 F100.0
G2 X102.000 Y100.000 I-2.000 J0.0 Z-2.000 F500.0
G2 X102.000 Y100.000 I-2.000 J0.0 Z-4.000 F500.0
G2 X102.000 Y100.000 I-2.000 J0.0 Z-6.000 F500.0
G2 X102.000 Y100.000 I-2.000 J0.0 Z-8.000 F500.0
G2 X102.000 Y100.000 I-2.000 J0.0 Z-10.000 F500.0
G2 X102.000 Y100.000 I-2.000 J0.0 F500.0
G1 X100.000 Y100.000 F500.0
G0 Z5.00 ; Retract to safe height
M5 ; Stop spindle
M30 ; End program
