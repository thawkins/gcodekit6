//! Tests for Preferences that change what is sent to the machine
//! (owner decision A-02, issue #76): comments and line length. Jog direction is tested
//! with the jog panel (`gcodekit5-ui-egui`'s `test_machine_jog`).

use gcodekit5_app::machine::grbl_stream::prepare_program;

#[test]
fn test_comments_are_removed_unless_preserved() {
    let program = "G0 X1 (rapid)\n; header\nG1 X2 ; cut\n(only a comment)\n";
    assert_eq!(
        prepare_program(program, false, 256).expect("ok"),
        "G0 X1\nG1 X2\n"
    );
    assert_eq!(
        prepare_program(program, true, 256).expect("ok"),
        "G0 X1 (rapid)\n; header\nG1 X2 ; cut\n(only a comment)\n"
    );
}

#[test]
fn test_line_longer_than_maximum_is_refused() {
    let long = format!("G1 X1 ({})", "a".repeat(100));
    let err = prepare_program(&format!("G0 X0\n{long}\n"), true, 80).expect_err("too long");
    assert!(err.contains("line 2"), "{err}");
    // Comments are removed before the length is checked.
    assert!(prepare_program(&long, false, 80).is_ok());
    // 0 disables the check.
    assert!(prepare_program(&long, true, 0).is_ok());
}

#[test]
fn test_semicolon_inside_parentheses_is_part_of_the_comment() {
    let out = prepare_program("G1 X1 (rough; finish) Y2 F100 ; tail\n", false, 256).expect("ok");
    assert!(out.starts_with("G1 X1"), "{out}");
    assert!(out.contains("Y2 F100"), "{out}");
    assert!(!out.contains("tail") && !out.contains("rough"), "{out}");
}
