use gcodekit_core::gcode::parser::parse_lines;

#[test]
fn test_parse_lines_strips_comments_and_whitespace() {
    let input = "G0 X0 ; move to origin\n  G1 X10 (linear move) \n; full line comment\nG2 X20 Y20";
    let lines = parse_lines(input);
    assert_eq!(lines, vec!["G0 X0".to_string(), "G1 X10".to_string(), "G2 X20 Y20".to_string()]);
}
