//! The Machine Limits section and the Buffer readout of the Machine Control pane: limits from
//! the controller's `$130`-`$132` (else Preferences), rotary limits once the machine reports
//! the axis, and the buffer from the status report's `Bf:` field, else "-".

use std::time::Duration;

use egui_kittest::kittest::Queryable;

use crate::machine_support::step_until;
use crate::test_machine_pane::{pane_app, report_state};

#[test]
fn test_buffer_shown_from_status_report() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    report_state(
        &mut harness,
        &shared,
        "<Idle|MPos:0.000,0.000,0.000|FS:0,0>",
        "Idle",
    );
    assert!(
        harness.query_by_label("Buffer: -").is_some(),
        "no Buffer: -"
    );
    report_state(
        &mut harness,
        &shared,
        "<Run|MPos:0.000,0.000,0.000|Bf:15,128|FS:0,0>",
        "Run",
    );
    assert!(
        harness.query_by_label("Buffer: 15/128").is_some(),
        "buffer not shown"
    );
}

#[test]
fn test_machine_limits_follow_grbl_travel() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    // Preferences' defaults until the controller reports its travel.
    assert_eq!(harness.query_all_by_label("0 - 200.00 mm").count(), 2);
    assert!(harness.query_by_label("0 - 100.00 mm").is_some());
    shared.inject("$130=300.000\n");
    assert!(
        step_until(&mut harness, |app| app.state().machine.status().grbl_travel
            [0]
            == Some(300.0)),
        "$130 not read"
    );
    harness.step();
    assert!(harness.query_by_label("0 - 300.00 mm").is_some());
}

#[test]
fn test_rotary_limit_shown_once_axis_reported() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    report_state(
        &mut harness,
        &shared,
        "<Idle|MPos:0.000,0.000,0.000|FS:0,0>",
        "Idle",
    );
    assert!(harness.query_by_label("A:").is_none(), "A shown unreported");
    report_state(
        &mut harness,
        &shared,
        "<Hold:0|MPos:0.000,0.000,0.000,90.000|FS:0,0>",
        "Hold:0",
    );
    assert!(harness.query_by_label("A:").is_some(), "A limit not shown");
    assert!(harness.query_by_label("0 - 360.0\u{b0}").is_some());
}
