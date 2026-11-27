; Spoilboard Surfacing Toolpath
; Dimensions: 304.8 x 203.2 mm
; Tool Diameter: 25.4 mm
; Cut Depth: 0.5 mm
G21 ; Set units to millimeters
G90 ; Absolute positioning
G17 ; XY plane selection

; Home, then run from the G54 work zero you set (offsets unchanged)
$H ; Home all axes
G54 ; Select work coordinate system 1
; Travel moves use machine Z-1.00 (G53) so they never rise above the homed position
G53 G0 Z-1.00 ; Move to safe height (machine coordinates)
M3 S16000
G0 X0 Y0
G1 Z-0.508 F600.0
G1 X304.800 F1200.0
G1 Y12.700 F1200.0
G1 X0.000 F1200.0
G1 Y25.400 F1200.0
G1 X304.800 F1200.0
G1 Y38.100 F1200.0
G1 X0.000 F1200.0
G1 Y50.800 F1200.0
G1 X304.800 F1200.0
G1 Y63.500 F1200.0
G1 X0.000 F1200.0
G1 Y76.200 F1200.0
G1 X304.800 F1200.0
G1 Y88.900 F1200.0
G1 X0.000 F1200.0
G1 Y101.600 F1200.0
G1 X304.800 F1200.0
G1 Y114.300 F1200.0
G1 X0.000 F1200.0
G1 Y127.000 F1200.0
G1 X304.800 F1200.0
G1 Y139.700 F1200.0
G1 X0.000 F1200.0
G1 Y152.400 F1200.0
G1 X304.800 F1200.0
G1 Y165.100 F1200.0
G1 X0.000 F1200.0
G1 Y177.800 F1200.0
G1 X304.800 F1200.0
G1 Y190.500 F1200.0
G1 X0.000 F1200.0
G1 Y203.200 F1200.0
G1 X304.800 F1200.0
G53 G0 Z-1.00 ; Retract (machine coordinates)
M5
M30
