; Drill Press Toolpath
; Hole Diameter: 12.000 mm
; Tool Diameter: 6.000 mm
; Depth: 0.000 to -5.000 mm
; Center: X50.000 Y25.000
G21 ; Set units to millimeters
G90 ; Absolute positioning
; Home, then run from the G54 work zero you set (offsets unchanged)
$H ; Home all axes
G54 ; Select work coordinate system 1
; Travel moves use machine Z-1.00 (G53) so they never rise above the homed position
G53 G0 Z-1.00 ; Move to safe height (machine coordinates)
M3 S12000 ; Start spindle
G0 X50.000 Y25.000 ; Move to hole center
; Helical interpolation cycle
G0 X53.000 Y25.000
G1 Z0.000 F200.0
G2 X53.000 Y25.000 I-3.000 J0.0 Z-1.000 F800.0
G2 X53.000 Y25.000 I-3.000 J0.0 Z-2.000 F800.0
G2 X53.000 Y25.000 I-3.000 J0.0 Z-3.000 F800.0
G2 X53.000 Y25.000 I-3.000 J0.0 Z-4.000 F800.0
G2 X53.000 Y25.000 I-3.000 J0.0 Z-5.000 F800.0
G2 X53.000 Y25.000 I-3.000 J0.0 F800.0
G1 X50.000 Y25.000 F800.0
G53 G0 Z-1.00 ; Retract to safe height (machine coordinates)
M5 ; Stop spindle
M30 ; End program
