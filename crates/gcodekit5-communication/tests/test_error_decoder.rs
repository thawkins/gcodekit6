//! Tests for `gcodekit5_communication::firmware::grbl::error_decoder` (moved from the inline test module).

use gcodekit5_communication::firmware::grbl::error_decoder::*;

#[test]
fn test_decode_error() {
    assert_eq!(
        decode_error(1),
        "G-code words consist of a letter and a value. Letter was not found."
    );
    assert_eq!(
        decode_error(9),
        "G-code locked out during alarm or jog state."
    );
    assert_eq!(
        decode_error(53),
        "Setting is not available, possibly due to limited driver support."
    );
    assert!(decode_error(69).contains("Unknown error code"));
    assert!(decode_error(255).contains("Unknown error code"));
}

#[test]
fn test_decode_alarm() {
    assert!(decode_alarm(1).contains("Hard limit"));
    assert!(decode_alarm(2).contains("Soft limit"));
    assert!(decode_alarm(12).contains("Limit switch engaged"));
    assert!(decode_alarm(15).contains("auto squared axis"));
    assert!(decode_alarm(255).contains("Unknown alarm code"));
}

#[test]
fn test_format_error() {
    let msg = format_error(1);
    assert!(msg.starts_with("error:1"));
    assert!(msg.contains("Letter was not found"));
}

#[test]
fn test_format_alarm() {
    let msg = format_alarm(1);
    assert!(msg.starts_with("ALARM:1"));
    assert!(msg.contains("Hard limit"));
}

/// grblHAL extended codes return the upstream texts (grblHAL/core
/// a53a16a4d756f12a27006152ba3a99b5ec46b2c7, errors.c / alarms.c).
#[test]
fn test_decode_grblhal_extended_codes_match_upstream() {
    assert_eq!(decode_error(39), "Value out of range.");
    assert_eq!(
        decode_error(45),
        "Only homing is allowed when a limit switch is engaged."
    );
    assert_eq!(
        decode_error(79),
        "Not allowed while critical event is active."
    );
    assert!(decode_error(60).contains("Unknown error code"));
    assert_eq!(decode_alarm(10), "EStop asserted. Clear and reset");
    assert_eq!(
        decode_alarm(11),
        "Homing required. Execute homing command ($H) to continue."
    );
    assert_eq!(decode_alarm(22), "Buffer overflow.");
    assert!(decode_alarm(23).contains("Unknown alarm code"));
}
