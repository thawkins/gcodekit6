//! Comment stripping in the shared G-code word tokenizer.
//!
//! The characterization tests pin the behaviour on well-formed lines, which
//! the nested-comment fix must not change. The nesting tests lock the fix:
//! a `( ... )` comment that contains another `( ... )` ends at the matching
//! outer `)`, not at the first `)`.

use std::borrow::Cow;

use gcodekit5_core::gcode::words::{parse_line, strip_comments};

/// Letters of the words found on a line, for compact assertions.
fn letters(line: &str) -> String {
    parse_line(line).iter().map(|w| w.letter).collect()
}

#[test]
fn test_strip_comments_no_comment_is_borrowed() {
    assert!(matches!(strip_comments("G1 X10"), Cow::Borrowed("G1 X10")));
}

#[test]
fn test_strip_comments_single_paren_comment() {
    assert_eq!(strip_comments("G1 (move) X10"), "G1   X10");
}

#[test]
fn test_strip_comments_paren_comment_mid_line_separates_words() {
    assert_eq!(strip_comments("G1(c)X10"), "G1 X10");
    assert_eq!(letters("G1(c)X10"), "GX");
}

#[test]
fn test_strip_comments_semicolon_comment() {
    assert_eq!(strip_comments("G1 X10 ; feed Y99"), "G1 X10 ");
    assert_eq!(strip_comments("; only a comment"), "");
}

#[test]
fn test_strip_comments_comment_only_line() {
    assert_eq!(strip_comments("(only a comment)"), " ");
    assert!(parse_line("(only a comment X5)").is_empty());
}

#[test]
fn test_strip_comments_unclosed_paren_comments_rest() {
    assert_eq!(strip_comments("G1 (unclosed X5"), "G1 ");
}

#[test]
fn test_strip_comments_unbalanced_close_is_text() {
    assert_eq!(strip_comments("G1 X1) Y2"), "G1 X1) Y2");
    assert_eq!(letters("G1 X1) Y2"), "GXY");
}

#[test]
fn test_strip_comments_semicolon_inside_paren_does_not_end_line() {
    assert_eq!(strip_comments("(a;b) G1"), "  G1");
    assert_eq!(letters("(a;b) G1 X1"), "GX");
}

#[test]
fn test_strip_comments_nested_paren_no_phantom_word() {
    assert_eq!(strip_comments("(a(b)X5) G1"), "  G1");
    assert_eq!(letters("(a(b)X5) G1"), "G");
}

#[test]
fn test_strip_comments_nested_paren_keeps_following_code() {
    assert_eq!(strip_comments("(a(b)c) G1 X1"), "  G1 X1");
    assert_eq!(letters("(a(b)c) G1 X1"), "GX");
}

#[test]
fn test_strip_comments_nested_unclosed_comments_rest() {
    assert_eq!(strip_comments("G1 (a(b) X5"), "G1 ");
}
