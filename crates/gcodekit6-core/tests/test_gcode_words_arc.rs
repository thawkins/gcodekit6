//! Regression tests for the shared G-code word tokenizer and arc maths.
//!
//! Moved unchanged (apart from the import path) from the visualizer's
//! `gcode_parsing_regressions.rs` and `audit_followup_regressions.rs` when its
//! dead G-code pipeline was deleted (audit T50); `gcodekit6_core::gcode` is the
//! canonical owner of `words` and `arc`.

use gcodekit6_core::gcode::arc::arc_sweep;
use gcodekit6_core::gcode::words::{parse_line, strip_comments};

#[test]
fn test_words_strip_paren_comment_keeps_following_words() {
    assert_eq!(strip_comments("G1 (move) X10").trim(), "G1   X10".trim());
    let words = parse_line("g1(c)x-1.5 ; Y99");
    assert_eq!(words.len(), 2);
    assert_eq!(words[1].letter, 'X');
    assert_eq!(words[1].value, -1.5);
}

#[test]
fn test_arc_sweep_quarter_and_full_circle() {
    let ccw = arc_sweep((0.0, 0.0), (1.0, 0.0), (0.0, 1.0), false).expect("arc");
    assert!((ccw.sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    let cw = arc_sweep((0.0, 0.0), (1.0, 0.0), (0.0, 1.0), true).expect("arc");
    assert!((cw.sweep + 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-9);

    let full = arc_sweep((0.0, 0.0), (1.0, 0.0), (1.0, 0.0), true).expect("arc");
    assert!((full.sweep + std::f64::consts::TAU).abs() < 1e-9);
    let (x, y) = full.point_at((0.0, 0.0), 0.5);
    assert!((x + 1.0).abs() < 1e-9 && y.abs() < 1e-9);

    assert_eq!(full.segment_count(std::f64::consts::PI / 36.0, 4, 128), 72);
    assert!(arc_sweep((0.0, 0.0), (0.0, 0.0), (1.0, 0.0), false).is_none());
}
