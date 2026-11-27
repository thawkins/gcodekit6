//! Tests for the binary's command-line parsing (`parse_args`, `version_line`).

use std::ffi::OsString;

use gcodekit6_design::{CliAction, HELP_TEXT, parse_args, version_line};

fn args(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

#[test]
fn test_cli_args_version_flag() {
    assert_eq!(parse_args(args(&["--version"])), CliAction::PrintVersion);
    assert_eq!(
        version_line(),
        format!("gcodekit6 {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn test_cli_args_help_flags() {
    assert_eq!(parse_args(args(&["--help"])), CliAction::PrintHelp);
    assert_eq!(parse_args(args(&["-h"])), CliAction::PrintHelp);
    assert_eq!(parse_args(args(&["part.nc", "-h"])), CliAction::PrintHelp);
    assert!(HELP_TEXT.contains("gcodekit6 [FILE...]"));
    assert!(HELP_TEXT.contains("--help") && HELP_TEXT.contains("--version"));
}

#[test]
fn test_cli_args_other_arguments_are_files() {
    assert_eq!(parse_args(args(&[])), CliAction::Run(Vec::new()));
    assert_eq!(
        parse_args(args(&["a.nc", "--verbose", "b.gckd"])),
        CliAction::Run(args(&["a.nc", "--verbose", "b.gckd"]))
    );
}
