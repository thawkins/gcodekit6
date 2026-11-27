//! Regression tests for the Repetier protocol translation and streaming
//! (audit car 1, issue #72).
//!
//! Each test pins one defect found by the audit: the translated line must
//! keep the meaning of the GRBL-style input, and the processor must frame,
//! acknowledge and rewind correctly. Streaming itself is the app's
//! `MachineController` (`gcodekit5-app` `test_machine_repetier`).

use gcodekit5_communication::firmware::repetier::processor::{RepetierEvent, RepetierProcessor};

/// Translates one outgoing line and returns the G-code body of each emitted
/// line, without the `N<n>` prefix and `*<checksum>` suffix.
fn translate(p: &mut RepetierProcessor, line: &str) -> Vec<String> {
    let out = p
        .process_outgoing(line.as_bytes())
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();
    out.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let body = l.split('*').next().unwrap_or("").trim();
            match body.split_once(' ') {
                Some((n, rest)) if n.starts_with('N') => rest.trim().to_string(),
                _ => body.to_string(),
            }
        })
        .collect()
}

#[test]
fn test_m400_is_not_translated_to_m3() {
    let mut p = RepetierProcessor::new();
    assert_eq!(translate(&mut p, "M400"), vec!["M400"]);
    assert_eq!(translate(&mut p, "M42 P4"), vec!["M42 P4"]);
}

#[test]
fn test_m4_is_still_translated_to_m3() {
    let mut p = RepetierProcessor::new();
    assert_eq!(translate(&mut p, "M4 S500"), vec!["M3 S500"]);
}

#[test]
fn test_preformatted_line_is_renumbered_and_m400_kept() {
    let mut p = RepetierProcessor::new();
    let out = p.process_outgoing(b"N7 M400*99").expect("line is sent");
    let text = String::from_utf8_lossy(&out);
    assert!(text.starts_with("N1 M400 "), "got {text:?}");
    assert_eq!(p.line_counter, 1);
    assert_eq!(p.resend_lines(1), Some(vec![text.into_owned()]));
}

#[test]
fn test_compact_words_are_tokenized() {
    let mut p = RepetierProcessor::new();
    assert_eq!(translate(&mut p, "M4S500"), vec!["M3 S500"]);
    assert_eq!(translate(&mut p, "M400"), vec!["M400"]);
    assert_eq!(translate(&mut p, "N10G1X10Y5"), vec!["G1 X10 Y5"]);
    assert!(status(&mut p).contains("MPos:10.000,5.000,0.000"));
}

#[test]
fn test_g28_keeps_axis_words() {
    let mut p = RepetierProcessor::new();
    assert_eq!(translate(&mut p, "G28 Z0"), vec!["G28 Z0.00"]);
}

#[test]
fn test_g28_1_is_not_turned_into_home_all() {
    let mut p = RepetierProcessor::new();
    assert_eq!(translate(&mut p, "G28.1"), vec!["G28.1"]);
}

#[test]
fn test_dollar_h_homes_all_axes() {
    let mut p = RepetierProcessor::new();
    assert_eq!(translate(&mut p, "$H"), vec!["G28"]);
}

#[test]
fn test_g10_l20_keeps_values_and_all_axes() {
    let mut p = RepetierProcessor::new();
    assert_eq!(
        translate(&mut p, "G10 L20 P1 X0.5 Y20"),
        vec!["G92 X0.5 Y20"]
    );
}

#[test]
fn test_g10_l20_zero_all_axes() {
    let mut p = RepetierProcessor::new();
    assert_eq!(
        translate(&mut p, "G10 L20 P1 X0 Y0 Z0"),
        vec!["G92 X0 Y0 Z0"]
    );
}

#[test]
fn test_jog_keeps_axis_and_feed_and_restores_absolute() {
    let mut p = RepetierProcessor::new();
    assert_eq!(
        translate(&mut p, "$J=G21 G91 X10 F1000"),
        vec!["G21", "G91", "G1 X10 F1000", "G90"]
    );
}

#[test]
fn test_jog_honours_g90_and_restores_prior_g91() {
    let mut p = RepetierProcessor::new();
    translate(&mut p, "G91");
    assert_eq!(
        translate(&mut p, "$J=G90 X10 F100"),
        vec!["G90", "G1 X10 F100", "G91"]
    );
}

#[test]
fn test_inch_jog_restores_mm() {
    let mut p = RepetierProcessor::new();
    assert_eq!(
        translate(&mut p, "$J=G20 G91 X1 F10"),
        vec!["G20", "G91", "G1 X1 F10", "G90", "G21"]
    );
}

#[test]
fn test_jog_with_multiple_axes() {
    let mut p = RepetierProcessor::new();
    assert_eq!(
        translate(&mut p, "$J=G91 X-1.5 Y2 F300"),
        vec!["G91", "G1 X-1.5 Y2 F300", "G90"]
    );
}

#[test]
fn test_user_line_number_is_replaced_not_dropped() {
    let mut p = RepetierProcessor::new();
    assert_eq!(translate(&mut p, "N10 G1 X5"), vec!["G1 X5"]);
}

#[test]
fn test_parameter_only_lines_are_sent() {
    let mut p = RepetierProcessor::new();
    assert_eq!(translate(&mut p, "F1000"), vec!["F1000"]);
    assert_eq!(translate(&mut p, "S1000"), vec!["S1000"]);
    assert_eq!(translate(&mut p, "E2.5"), vec!["E2.5"]);
}

#[test]
fn test_line_numbers_are_consecutive_with_valid_checksum() {
    let mut p = RepetierProcessor::new();
    let a = String::from_utf8(p.process_outgoing(b"G1 X1").expect("sent")).expect("utf8");
    let b = String::from_utf8(p.process_outgoing(b"G1 X2").expect("sent")).expect("utf8");
    assert!(a.starts_with("N1 "), "got {a:?}");
    assert!(b.starts_with("N2 "), "got {b:?}");
    let (body, ck) = b.trim_end().split_once('*').expect("checksum present");
    let expected = body.bytes().fold(0u8, |acc, x| acc ^ x);
    assert_eq!(ck.parse::<u8>().expect("numeric checksum"), expected);
}

#[test]
fn test_ack_split_across_reads_is_detected() {
    let mut p = RepetierProcessor::new();
    let first = p.process_incoming(b"o".to_vec());
    assert!(first.events.is_empty());
    let second = p.process_incoming(b"k\n".to_vec());
    assert_eq!(second.events, vec![RepetierEvent::Ack]);
}

#[test]
fn test_resend_split_across_reads_is_detected() {
    let mut p = RepetierProcessor::new();
    assert!(p.process_incoming(b"Resend:1".to_vec()).events.is_empty());
    let r = p.process_incoming(b"2\nok\n".to_vec());
    assert_eq!(
        r.events,
        vec![RepetierEvent::Resend(12), RepetierEvent::Ack]
    );
}

fn status(p: &mut RepetierProcessor) -> String {
    let r = p.process_incoming(Vec::new());
    String::from_utf8_lossy(&r.display).into_owned()
}

#[test]
fn test_status_tracks_absolute_and_relative_moves() {
    let mut p = RepetierProcessor::new();
    translate(&mut p, "G90");
    translate(&mut p, "G1 X10 Y5");
    assert!(status(&mut p).contains("MPos:10.000,5.000,0.000"));
    translate(&mut p, "$J=G91 X1 F500");
    assert!(status(&mut p).contains("MPos:11.000,5.000,0.000"));
    translate(&mut p, "G1 Z2");
    assert!(status(&mut p).contains("MPos:11.000,5.000,2.000"));
}

#[test]
fn test_status_uses_reported_position() {
    let mut p = RepetierProcessor::new();
    let r =
        p.process_incoming(b"X:3.00 Y:4.00 Z:5.00 E:0.00 Count X:3.00 Y:4.00 Z:5.00\n".to_vec());
    let text = String::from_utf8_lossy(&r.display);
    assert!(text.contains("MPos:3.000,4.000,5.000"), "got {text:?}");
}

#[test]
fn test_status_state_follows_flow_control() {
    let mut p = RepetierProcessor::new();
    assert!(status(&mut p).contains("<Idle|"));
    p.mark_buffer_full();
    assert!(status(&mut p).contains("<Run|"));
    p.mark_ack();
    assert!(status(&mut p).contains("<Idle|"));
}

#[test]
fn test_resend_lines_replays_exact_suffix() {
    let mut p = RepetierProcessor::new();
    let sent: Vec<String> = translate_raw(&mut p, "$J=G91 X1 F100");
    assert_eq!(sent.len(), 3);
    // N2 and N3 were not accepted: replay exactly those, not N1.
    assert_eq!(p.resend_lines(2), Some(sent[1..].to_vec()));
    // The next number not yet sent: nothing to replay.
    assert_eq!(p.resend_lines(4), Some(Vec::new()));
    // A number never sent cannot be honoured.
    assert_eq!(p.resend_lines(5), None);
    assert_eq!(p.resend_lines(0), None);
}

#[test]
fn test_resend_of_evicted_line_is_refused() {
    let mut p = RepetierProcessor::new();
    for i in 0..600 {
        translate(&mut p, &format!("G1 X{i}"));
    }
    assert_eq!(p.resend_lines(1), None);
    assert_eq!(p.resend_lines(600).map(|l| l.len()), Some(1));
}

#[test]
fn test_activate_resets_modal_state_and_position() {
    let mut p = RepetierProcessor::new();
    translate(&mut p, "G20");
    translate(&mut p, "G91");
    translate(&mut p, "G1 X1");
    p.activate();
    translate(&mut p, "G1 X10");
    assert!(status(&mut p).contains("MPos:10.000,0.000,0.000"));
    assert_eq!(p.resend_lines(1).map(|l| l.len()), Some(1));
}

#[test]
fn test_status_reports_g92_offset_as_wco() {
    let mut p = RepetierProcessor::new();
    translate(&mut p, "G1 X10 Y4");
    translate(&mut p, "G10 L20 P1 X0 Y0");
    let s = status(&mut p);
    assert!(s.contains("MPos:10.000,4.000,0.000"), "got {s:?}");
    assert!(s.contains("WCO:10.000,4.000,0.000"), "got {s:?}");
    // Absolute moves are now in the shifted work coordinates.
    translate(&mut p, "G1 X1");
    assert!(status(&mut p).contains("MPos:11.000,4.000,0.000"));
    // Homing clears the offset.
    translate(&mut p, "$H");
    assert!(status(&mut p).contains("WCO:0.000,0.000,0.000"));
}

#[test]
fn test_configuration_axis_words_do_not_move_position() {
    let mut p = RepetierProcessor::new();
    translate(&mut p, "M201 X1000 Y1000 Z100");
    translate(&mut p, "M92 X80");
    assert!(status(&mut p).contains("MPos:0.000,0.000,0.000"));
}

#[test]
fn test_ack_in_same_read_reports_idle() {
    let mut p = RepetierProcessor::new();
    p.mark_buffer_full();
    let r = p.process_incoming(b"ok\n".to_vec());
    let text = String::from_utf8_lossy(&r.display);
    assert!(text.contains("<Idle|"), "got {text:?}");
}

/// Translates one outgoing line and returns each emitted numbered line as
/// written to the port.
fn translate_raw(p: &mut RepetierProcessor, line: &str) -> Vec<String> {
    let out = p.process_outgoing(line.as_bytes()).expect("line is sent");
    String::from_utf8_lossy(&out)
        .split_inclusive('\n')
        .map(str::to_string)
        .collect()
}

/// A bare axis line continues the modal motion: after `G0` it stays a rapid and is
/// not forced to `G1` (P2FW-6).
#[test]
fn test_bare_axis_line_continues_modal_motion() {
    let mut p = RepetierProcessor::new();
    assert_eq!(translate(&mut p, "G0 X1"), vec!["G0 X1"]);
    assert_eq!(translate(&mut p, "X10"), vec!["G0 X10"]);
    assert_eq!(translate(&mut p, "G1 X2 F100"), vec!["G1 X2 F100"]);
    assert_eq!(translate(&mut p, "Y3"), vec!["G1 Y3"]);
    // A jog does not change the program's modal motion.
    translate(&mut p, "G0 X0");
    translate(&mut p, "$J=G91 X1 F500");
    assert_eq!(translate(&mut p, "X5"), vec!["G0 X5"]);
    assert!(status(&mut p).contains("MPos:5.000,3.000,0.000"));
}

/// Only `ok` and a bare `wait` acknowledge a line; a message that mentions waiting
/// does not (P2FW-6).
#[test]
fn test_waiting_message_is_not_an_ack() {
    let mut p = RepetierProcessor::new();
    let r = p.process_incoming(b"echo:waiting for heater\n".to_vec());
    assert!(!r.events.contains(&RepetierEvent::Ack), "{:?}", r.events);
    let r = p.process_incoming(b"wait\n".to_vec());
    assert!(r.events.contains(&RepetierEvent::Ack), "{:?}", r.events);
}

/// A bare `G92` resets the `G92` offset reported as the stand-in `WCO` (P2FW-6).
#[test]
fn test_bare_g92_resets_wco() {
    let mut p = RepetierProcessor::new();
    translate(&mut p, "G1 X10 Y4");
    translate(&mut p, "G92 X3");
    assert!(status(&mut p).contains("WCO:7.000,0.000,0.000"));
    // A bare `G92` zeroes every axis here, like `G92 X0 Y0 Z0`.
    translate(&mut p, "G92");
    let s = status(&mut p);
    assert!(s.contains("WCO:10.000,4.000,0.000"), "got {s:?}");
    assert!(s.contains("MPos:10.000,4.000,0.000"), "got {s:?}");
}

/// Characterization of the outgoing comment stripping (T58, CAMVIS-10).
///
/// Repetier/Marlin firmware treats `;` as a line comment and has no
/// parenthesis comments, so the processor strips both before sending. These
/// assertions pin that behaviour.
#[test]
fn test_comment_stripping_characterization() {
    let mut p = RepetierProcessor::new();
    // A removed comment between words leaves them separated.
    assert_eq!(translate(&mut p, "G1(c)X10"), vec!["G1 X10"]);
    // A `;` inside parentheses is part of the comment; the `G1` is kept.
    assert_eq!(translate(&mut p, "(a;b) G1"), vec!["G1"]);
    assert_eq!(translate(&mut p, "G1 X10 ; c"), vec!["G1 X10"]);
    assert_eq!(translate(&mut p, "G1 X10 (c) Y5"), vec!["G1 X10 Y5"]);
    // A comment-only line sends nothing.
    assert!(translate(&mut p, "(only a comment)").is_empty());
    assert!(translate(&mut p, "; only a comment").is_empty());
    // Nested parentheses are tracked by depth, so the whole comment goes.
    assert_eq!(translate(&mut p, "(a(b)c) G1 X1"), vec!["G1 X1"]);
    assert_eq!(translate(&mut p, "(a(b)X5) G1"), vec!["G1"]);
}
