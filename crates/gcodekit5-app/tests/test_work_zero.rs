//! The work zero command builders and the `[GC:...]` parser-state reading.

use gcodekit5_app::machine::Axis;
use gcodekit5_app::work_zero::{
    PARSER_STATE_QUERY, Wcs, go_to_work_zero_command, parser_state_wcs, wcs_in_line,
    zero_axes_command,
};

#[test]
fn test_wcs_codes_and_p_numbers() {
    let codes: Vec<&str> = Wcs::ALL.iter().map(|w| w.code()).collect();
    assert_eq!(codes, ["G54", "G55", "G56", "G57", "G58", "G59"]);
    let p: Vec<u8> = Wcs::ALL.iter().map(|w| w.p_number()).collect();
    assert_eq!(p, [1, 2, 3, 4, 5, 6]);
}

#[test]
fn test_zero_axes_command_uses_the_wcs_p_number() {
    assert_eq!(zero_axes_command(Wcs::G54, &[Axis::X]), "G10 L20 P1 X0");
    assert_eq!(zero_axes_command(Wcs::G55, &[Axis::X]), "G10 L20 P2 X0");
    assert_eq!(zero_axes_command(Wcs::G59, &[Axis::Z]), "G10 L20 P6 Z0");
    assert_eq!(
        zero_axes_command(Wcs::G56, &[Axis::X, Axis::Y, Axis::Z]),
        "G10 L20 P3 X0 Y0 Z0"
    );
}

#[test]
fn test_go_to_work_zero_command() {
    assert_eq!(go_to_work_zero_command(false), "G90 G0 X0 Y0");
    assert_eq!(go_to_work_zero_command(true), "G90 G0 X0 Y0 Z0");
}

#[test]
fn test_parser_state_query_is_dollar_g() {
    assert_eq!(PARSER_STATE_QUERY, "$G");
}

#[test]
fn test_parser_state_wcs_reads_grbl_gc_reply() {
    assert_eq!(
        parser_state_wcs("[GC:G0 G54 G17 G21 G90 G94 M5 M9 T0 F0 S0]"),
        Some(Wcs::G54)
    );
    assert_eq!(
        parser_state_wcs("[GC:G1 G57 G17 G20 G91 G94 M3 M8 T1 F500 S1000]\r"),
        Some(Wcs::G57)
    );
    assert_eq!(parser_state_wcs("[GC:G0 G17 G21]"), None);
    assert_eq!(parser_state_wcs("[MSG:G55]"), None);
    assert_eq!(parser_state_wcs("ok"), None);
}

#[test]
fn test_wcs_in_line_finds_g5x_words() {
    assert_eq!(wcs_in_line("G55"), Some(Wcs::G55));
    assert_eq!(wcs_in_line("g56"), Some(Wcs::G56));
    assert_eq!(wcs_in_line("G0G59X0"), Some(Wcs::G59));
    assert_eq!(wcs_in_line("G90 G58 G0 X1"), Some(Wcs::G58));
    assert_eq!(wcs_in_line("G055"), Some(Wcs::G55));
    // Not a WCS selection.
    assert_eq!(wcs_in_line("G10 L20 P2 X0"), None);
    assert_eq!(wcs_in_line("G0 X54"), None);
    assert_eq!(wcs_in_line("G54.1"), None);
    assert_eq!(wcs_in_line("G1 X0 (G55)"), None);
    assert_eq!(wcs_in_line("G1 X0 ; G55"), None);
    assert_eq!(wcs_in_line("$G"), None);
}
