//! Comment splitting in `comment_processor`.
//!
//! The characterization tests pin the behaviour on well-formed lines, which
//! the nested-comment fix must not change. The nesting tests lock the fix:
//! a `( ... )` comment that contains another `( ... )` ends at the matching
//! outer `)`, as in `gcodekit5_core::gcode::words::strip_comments`.

use gcodekit5_camtools::comment_processor::{
    SystemCommand, split_comments, strip_comments, system_command, words,
};

/// Owned `(code, comments)` pair for compact assertions.
fn split(line: &str) -> (String, Vec<String>) {
    split_comments(line)
}

/// Owned comment list from string slices.
fn texts(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// Letters of the words found on a line.
fn letters(line: &str) -> String {
    words(line).iter().map(|w| w.letter).collect()
}

#[test]
fn test_split_comments_no_comment() {
    assert_eq!(split("G1 X10"), ("G1 X10".to_string(), Vec::new()));
}

#[test]
fn test_split_comments_single_paren_comment() {
    assert_eq!(
        split("G1 (move) X10"),
        ("G1 X10".to_string(), texts(&["move"]))
    );
}

#[test]
fn test_split_comments_paren_comment_mid_line_separates_words() {
    assert_eq!(split("G1(c)X10"), ("G1 X10".to_string(), texts(&["c"])));
    assert_eq!(letters("G1(c)X10"), "GX");
}

#[test]
fn test_split_comments_semicolon_comment() {
    assert_eq!(
        split("G1 X10 ; feed Y99"),
        ("G1 X10".to_string(), texts(&["feed Y99"]))
    );
    assert_eq!(letters("G1 X10 ; feed Y99"), "GX");
}

#[test]
fn test_split_comments_comment_only_line() {
    assert_eq!(
        split("(only a comment)"),
        (String::new(), texts(&["only a comment"]))
    );
    assert!(words("(only a comment X5)").is_empty());
}

#[test]
fn test_split_comments_unclosed_paren_comments_rest() {
    assert_eq!(
        split("G1 (unclosed X5"),
        ("G1".to_string(), texts(&["unclosed X5"]))
    );
}

#[test]
fn test_split_comments_unbalanced_close_is_text() {
    assert_eq!(split("G1 X1) Y2"), ("G1 X1) Y2".to_string(), Vec::new()));
}

#[test]
fn test_split_comments_semicolon_inside_paren_does_not_end_line() {
    assert_eq!(split("(a;b) G1"), ("G1".to_string(), texts(&["a;b"])));
}

#[test]
fn test_system_command_ignores_comment() {
    assert_eq!(
        system_command("$H (home)"),
        Some(SystemCommand::Known("$H".to_string()))
    );
}

#[test]
fn test_split_comments_nested_paren_no_phantom_word() {
    assert_eq!(split("(a(b)X5) G1"), ("G1".to_string(), texts(&["a(b)X5"])));
    assert_eq!(strip_comments("(a(b)X5) G1"), "G1");
    assert_eq!(letters("(a(b)X5) G1"), "G");
}

#[test]
fn test_split_comments_nested_paren_keeps_following_code() {
    assert_eq!(
        split("(a(b)c) G1 X1"),
        ("G1 X1".to_string(), texts(&["a(b)c"]))
    );
    assert_eq!(letters("(a(b)c) G1 X1"), "GX");
}

#[test]
fn test_split_comments_nested_unclosed_comments_rest() {
    assert_eq!(
        split("G1 (a(b) X5"),
        ("G1".to_string(), texts(&["a(b) X5"]))
    );
}
