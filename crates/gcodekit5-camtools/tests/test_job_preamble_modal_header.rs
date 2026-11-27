//! Locks the exact bytes of the shared modal header that the generators emit at job start.

use gcodekit5_camtools::job_preamble::push_modal_header;

#[test]
fn test_job_preamble_modal_header_with_plane() {
    let mut gcode = String::from("; start\n");
    push_modal_header(&mut gcode, true);
    assert_eq!(
        gcode,
        "; start\nG21 ; Set units to millimeters\nG90 ; Absolute positioning\n\
         G17 ; XY plane selection\n"
    );
}

#[test]
fn test_job_preamble_modal_header_without_plane() {
    let mut gcode = String::new();
    push_modal_header(&mut gcode, false);
    assert_eq!(
        gcode,
        "G21 ; Set units to millimeters\nG90 ; Absolute positioning\n"
    );
}
