; Spoilboard Grid Toolpath
; Dimensions: 200.0 x 150.0 mm
; Grid Spacing: 25.0 mm
; Laser Power: 500.0
G21 ; Set units to millimeters
G90 ; Absolute positioning
G17 ; XY plane selection

; Home, then run from the G54 work zero you set (offsets unchanged)
$H ; Home all axes
G54 ; Select work coordinate system 1
; Travel moves use machine Z-1.00 (G53) so they never rise above the homed position
; NOTE: requires GRBL laser mode ($32=1) to be enabled by the user beforehand

; Vertical Lines
M5
G0 X0.000 Y0.000
M3 S500
G1 Y150.000 F1500.0
M5
G0 X25.000 Y150.000
M3 S500
G1 Y0.000 F1500.0
M5
G0 X50.000 Y0.000
M3 S500
G1 Y150.000 F1500.0
M5
G0 X75.000 Y150.000
M3 S500
G1 Y0.000 F1500.0
M5
G0 X100.000 Y0.000
M3 S500
G1 Y150.000 F1500.0
M5
G0 X125.000 Y150.000
M3 S500
G1 Y0.000 F1500.0
M5
G0 X150.000 Y0.000
M3 S500
G1 Y150.000 F1500.0
M5
G0 X175.000 Y150.000
M3 S500
G1 Y0.000 F1500.0
M5
G0 X200.000 Y0.000
M3 S500
G1 Y150.000 F1500.0

; Horizontal Lines
M5
G0 X0.000 Y0.000
M3 S500
G1 X200.000 F1500.0
M5
G0 X200.000 Y25.000
M3 S500
G1 X0.000 F1500.0
M5
G0 X0.000 Y50.000
M3 S500
G1 X200.000 F1500.0
M5
G0 X200.000 Y75.000
M3 S500
G1 X0.000 F1500.0
M5
G0 X0.000 Y100.000
M3 S500
G1 X200.000 F1500.0
M5
G0 X200.000 Y125.000
M3 S500
G1 X0.000 F1500.0
M5
G0 X0.000 Y150.000
M3 S500
G1 X200.000 F1500.0

M5 ; Laser Off
G0 X0 Y0 ; Return to origin
M30 ; End program
