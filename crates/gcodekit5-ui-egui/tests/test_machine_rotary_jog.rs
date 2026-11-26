//! Rotary jog in Machine Control (GTK's `-`/`+` buttons in the A, B and C readout rows):
//! shown for the rotary axes the machine reports, stepping by the rotary jog increment in
//! degrees, or holding in Continuous mode, under the pad's guards and cancel rules.

use std::sync::Arc;
use std::time::Duration;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use gcodekit5_app::test_support::LoopbackShared;
use gcodekit5_ui_egui::GcodeKitApp;

use crate::machine_support::{loopback_config, wait_until};
use crate::test_machine_keyboard_jog::{cancels, jogs, settled_jogs};
use crate::test_machine_pane::{pane_app_with, pointer_button, report_state};

const A_AND_B: &str = "<Idle|MPos:0.000,0.000,0.000,10.000,20.000|FS:0,0>";

/// Connected with a rotary step of 5 degrees, Idle with `report`.
fn rotary_app(report: &str, state: &str) -> (Harness<'static, GcodeKitApp>, Arc<LoopbackShared>) {
    rotary_app_with_step(5.0, report, state)
}

/// [`rotary_app`] with a rotary step of `step` degrees.
fn rotary_app_with_step(
    step: f64,
    report: &str,
    state: &str,
) -> (Harness<'static, GcodeKitApp>, Arc<LoopbackShared>) {
    let mut config = loopback_config();
    config.machine.jog_increment_rotary = step;
    let (mut harness, shared) = pane_app_with(config, Duration::from_millis(1), true);
    report_state(&mut harness, &shared, report, state);
    (harness, shared)
}

fn enabled(harness: &Harness<'_, GcodeKitApp>, label: &str) -> bool {
    !harness.get_by_label(label).accesskit_node().is_disabled()
}

#[test]
fn test_rotary_jog_steps_by_the_rotary_increment() {
    let (mut harness, shared) = rotary_app(A_AND_B, "Idle");
    assert!(harness.query_by_label("Jog C+").is_none(), "C not reported");
    assert_eq!(harness.query_all_by_label("Step 5.000\u{b0}").count(), 2);
    harness.get_by_label("Jog A+").click();
    harness.step();
    assert!(
        wait_until(|| jogs(&shared).len() == 1),
        "{:?}",
        shared.log()
    );
    harness.get_by_label("Jog B-").click();
    harness.step();
    assert!(
        wait_until(|| jogs(&shared).len() == 2),
        "{:?}",
        shared.log()
    );
    assert_eq!(
        jogs(&shared),
        ["$J=G91 G21 A5.000 F1000", "$J=G91 G21 B-5.000 F1000"]
    );
}

#[test]
fn test_rotary_jog_disabled_outside_idle() {
    let (mut harness, shared) = rotary_app(A_AND_B, "Idle");
    report_state(
        &mut harness,
        &shared,
        "<Alarm|MPos:0.000,0.000,0.000,10.000,20.000|FS:0,0>",
        "Alarm",
    );
    assert!(!enabled(&harness, "Jog A+"), "enabled in Alarm");
    harness.get_by_label("Jog A+").click();
    harness.step();
    assert!(
        settled_jogs(&harness, &shared).is_empty(),
        "{:?}",
        shared.log()
    );
}

/// Continuous: a held rotary button moves at most the axis limit (360 degrees) and ends with
/// one jog cancel on release.
#[test]
fn test_rotary_hold_cancels_on_release() {
    let (mut harness, shared) = rotary_app(A_AND_B, "Idle");
    harness.get_by_label("Continuous").click();
    harness.step();
    let at = harness.get_by_label("Jog B+").rect().center();
    pointer_button(&harness, at, true);
    harness.step();
    assert!(
        wait_until(|| jogs(&shared).len() == 1),
        "{:?}",
        shared.log()
    );
    assert_eq!(jogs(&shared), ["$J=G91 G21 B360.000 F1000"]);
    for _ in 0..5 {
        harness.step();
    }
    assert_eq!(cancels(&shared), 0, "hold ended by itself");
    pointer_button(&harness, at, false);
    harness.step();
    harness.step();
    assert!(wait_until(|| cancels(&shared) == 1), "{:?}", shared.log());
}

/// A held rotary button whose row goes away (the machine stops reporting the axis) ends the
/// hold.
#[test]
fn test_rotary_hold_cancelled_when_the_axis_row_goes() {
    let (mut harness, shared) = rotary_app(A_AND_B, "Idle");
    harness.get_by_label("Continuous").click();
    harness.step();
    let at = harness.get_by_label("Jog A-").rect().center();
    pointer_button(&harness, at, true);
    harness.step();
    assert!(
        wait_until(|| jogs(&shared).len() == 1),
        "{:?}",
        shared.log()
    );
    report_state(
        &mut harness,
        &shared,
        "<Jog|MPos:0.000,0.000,0.000|FS:0,0>",
        "Jog",
    );
    harness.step();
    assert!(wait_until(|| cancels(&shared) == 1), "{:?}", shared.log());
}

/// A rotary step too small for the jog line's 3 decimals is raised to 0.001 degree, so the
/// button never sends a zero-length jog.
#[test]
fn test_tiny_rotary_step_is_raised_not_sent_as_zero() {
    let (mut harness, shared) = rotary_app_with_step(0.0001, A_AND_B, "Idle");
    // The step used is shown in each rotary row, not the configured 0.0001.
    assert_eq!(harness.query_all_by_label("Step 0.001\u{b0}").count(), 2);
    harness.get_by_label("Jog A+").click();
    harness.step();
    assert!(
        wait_until(|| jogs(&shared).len() == 1),
        "{:?}",
        shared.log()
    );
    assert_eq!(jogs(&shared), ["$J=G91 G21 A0.001 F1000"]);
}
