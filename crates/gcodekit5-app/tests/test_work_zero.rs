//! The work zero command builders and the `[GC:...]` parser-state reading.

use gcodekit5_app::machine::Axis;
use gcodekit5_app::work_zero::{
    PARSER_STATE_QUERY, Wcs, WcsWord, go_to_work_zero_command, parser_state_wcs,
    reports_grbl_parser_state, wcs_in_line, zero_axes_command, zero_rotary_command,
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

/// Review Focus 2: a rotary zero names the active system's P number, as X/Y/Z do, and
/// writes the rotary calibration value.
#[test]
fn test_zero_rotary_command_uses_p_number_and_calibration() {
    let line = |wcs, axis, value| zero_rotary_command(wcs, axis, value);
    assert_eq!(
        line(Wcs::G54, Axis::A, 0.0).as_deref(),
        Some("G10 L20 P1 A0")
    );
    assert_eq!(
        line(Wcs::G55, Axis::B, 12.5).as_deref(),
        Some("G10 L20 P2 B12.5")
    );
    assert_eq!(
        line(Wcs::G59, Axis::C, -3.25).as_deref(),
        Some("G10 L20 P6 C-3.25")
    );
}

/// A calibration is sent with at most 3 decimals, and a non-finite one not at all.
#[test]
fn test_zero_rotary_command_rounds_and_refuses_non_finite() {
    let line = |value| zero_rotary_command(Wcs::G54, Axis::A, value);
    assert_eq!(line(0.1 + 0.2).as_deref(), Some("G10 L20 P1 A0.3"));
    assert_eq!(line(90.00049).as_deref(), Some("G10 L20 P1 A90"));
    assert_eq!(line(-0.0001).as_deref(), Some("G10 L20 P1 A0"));
    assert_eq!(line(100.0).as_deref(), Some("G10 L20 P1 A100"));
    assert_eq!(line(f64::NAN), None);
    assert_eq!(line(f64::INFINITY), None);
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
        Some(WcsWord::Known(Wcs::G54))
    );
    assert_eq!(
        parser_state_wcs("[GC:G1 G57 G17 G20 G91 G94 M3 M8 T1 F500 S1000]\r"),
        Some(WcsWord::Known(Wcs::G57))
    );
    assert_eq!(parser_state_wcs("[MSG:G55]"), None);
    assert_eq!(parser_state_wcs("ok"), None);
}

#[test]
fn test_parser_state_outside_g54_g59_is_other() {
    // grblHAL with nine systems.
    assert_eq!(
        parser_state_wcs("[GC:G0 G59.1 G17 G21 G90 G94 M5 M9 T0 F0 S0]"),
        Some(WcsWord::Other)
    );
    assert_eq!(parser_state_wcs("[GC:G0 G17 G21]"), Some(WcsWord::Other));
}

#[test]
fn test_wcs_in_line_finds_g5x_words() {
    let known = |w| Some(WcsWord::Known(w));
    assert_eq!(wcs_in_line("G55"), known(Wcs::G55));
    assert_eq!(wcs_in_line("g56"), known(Wcs::G56));
    assert_eq!(wcs_in_line("G0G59X0"), known(Wcs::G59));
    assert_eq!(wcs_in_line("G90 G58 G0 X1"), known(Wcs::G58));
    assert_eq!(wcs_in_line("G055"), known(Wcs::G55));
    assert_eq!(wcs_in_line("G55.0"), known(Wcs::G55));
    // Not a WCS selection.
    assert_eq!(wcs_in_line("G10 L20 P2 X0"), None);
    assert_eq!(wcs_in_line("G0 X54"), None);
    assert_eq!(wcs_in_line("G1 X0 (G55)"), None);
    assert_eq!(wcs_in_line("G1 X0 ; G55"), None);
    assert_eq!(wcs_in_line("$G"), None);
}

#[test]
fn test_wcs_in_line_outside_g54_g59_is_other() {
    assert_eq!(wcs_in_line("G59.2"), Some(WcsWord::Other));
    assert_eq!(wcs_in_line("G54.1 P7"), Some(WcsWord::Other));
    // The last selection wins.
    assert_eq!(wcs_in_line("G59.1 G55"), Some(WcsWord::Known(Wcs::G55)));
    assert_eq!(wcs_in_line("G55 G59.3"), Some(WcsWord::Other));
}

#[test]
fn test_only_grbl_family_is_asked_for_parser_state() {
    use gcodekit5_devicedb::ControllerType;
    assert!(reports_grbl_parser_state(None));
    for grbl in [
        ControllerType::Grbl,
        ControllerType::GrblHal,
        ControllerType::FluidNC,
    ] {
        assert!(reports_grbl_parser_state(Some(&grbl)), "{grbl:?}");
    }
    for other in [
        ControllerType::TinyG,
        ControllerType::G2Core,
        ControllerType::Smoothieware,
        ControllerType::Marlin,
        ControllerType::Repetier,
    ] {
        assert!(!reports_grbl_parser_state(Some(&other)), "{other:?}");
    }
}
