//! GTK's "Set %" in the Machine Control pane: the feed or spindle override is moved to an
//! exact percentage one realtime step at a time, each sent only once the controller's `Ov:`
//! report confirmed the one before.

use std::sync::Arc;
use std::time::Duration;

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use gcodekit5_app::machine::RealtimeCommand;
use gcodekit5_app::test_support::{LoopbackShared, Wrote};
use gcodekit5_ui_egui::GcodeKitApp;

use crate::machine_support::{step_until, wait_until};
use crate::test_machine_pane::pane_app;

/// How many times the realtime byte of `command` was written.
fn count(shared: &LoopbackShared, command: RealtimeCommand) -> usize {
    shared
        .log()
        .iter()
        .filter(|w| **w == Wrote::Realtime(command.byte()))
        .count()
}

/// Has the controller report feed and spindle overrides and waits until the app has them.
fn report_ov(
    harness: &mut Harness<'_, GcodeKitApp>,
    shared: &LoopbackShared,
    feed: u16,
    spindle: u16,
) {
    shared.inject(&format!(
        "<Idle|MPos:0.000,0.000,0.000|Ov:{feed},100,{spindle}|FS:0,0>\n"
    ));
    assert!(
        step_until(harness, |app| app
            .state()
            .machine
            .status()
            .overrides
            .is_some_and(|o| o.feed == feed && o.spindle == spindle)),
        "Ov:{feed},100,{spindle} never reported"
    );
    harness.step();
}

/// The Set % field `index` (0 feed, 1 spindle): the spin buttons showing a percentage.
fn set_target(harness: &mut Harness<'_, GcodeKitApp>, index: usize, percent: f64) {
    harness
        .get_all_by_role(Role::SpinButton)
        .filter(|n| n.value().is_some_and(|v| v.ends_with('%')))
        .nth(index)
        .expect("target field")
        .set_value_accesskit(percent);
    harness.step();
    harness.step();
}

/// Clicks the Set % button `index` (0 feed, 1 spindle).
fn click_set(harness: &mut Harness<'_, GcodeKitApp>, index: usize) {
    harness
        .query_all_by_label("Set %")
        .nth(index)
        .expect("Set %")
        .click();
    harness.step();
    harness.step();
}

/// Whether the Set % button `index` is enabled.
fn set_enabled(harness: &Harness<'_, GcodeKitApp>, index: usize) -> bool {
    harness
        .query_all_by_label("Set %")
        .nth(index)
        .is_some_and(|n| !n.accesskit_node().is_disabled())
}

fn connected_with_ov() -> (Harness<'static, GcodeKitApp>, Arc<LoopbackShared>) {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    report_ov(&mut harness, &shared, 100, 100);
    (harness, shared)
}

#[test]
fn test_feed_set_percent_steps_with_confirmation() {
    let (mut harness, shared) = connected_with_ov();
    set_target(&mut harness, 0, 112.0);
    click_set(&mut harness, 0);
    assert!(
        wait_until(|| count(&shared, RealtimeCommand::FeedOverridePlus10) == 1),
        "{:?}",
        shared.log()
    );
    // Nothing more until the controller reports the step.
    for _ in 0..4 {
        harness.step();
    }
    assert_eq!(count(&shared, RealtimeCommand::FeedOverridePlus1), 0);
    assert!(!set_enabled(&harness, 0), "Set enabled mid-sequence");
    report_ov(&mut harness, &shared, 110, 100);
    assert!(step_until(&mut harness, |_| count(
        &shared,
        RealtimeCommand::FeedOverridePlus1
    ) == 1));
    report_ov(&mut harness, &shared, 111, 100);
    assert!(step_until(&mut harness, |_| count(
        &shared,
        RealtimeCommand::FeedOverridePlus1
    ) == 2));
    report_ov(&mut harness, &shared, 112, 100);
    harness.step();
    assert!(set_enabled(&harness, 0), "Set not re-enabled at the target");
    assert_eq!(count(&shared, RealtimeCommand::FeedOverridePlus10), 1);
    assert_eq!(count(&shared, RealtimeCommand::FeedOverridePlus1), 2);
    // The spindle override was never touched.
    assert!(!shared.log().iter().any(|w| matches!(
        w,
        Wrote::Realtime(b) if (0x99..=0x9D).contains(b)
    )));
}

#[test]
fn test_spindle_set_percent_sends_spindle_bytes() {
    let (mut harness, shared) = connected_with_ov();
    set_target(&mut harness, 1, 90.0);
    click_set(&mut harness, 1);
    assert!(
        wait_until(|| count(&shared, RealtimeCommand::SpindleOverrideMinus10) == 1),
        "{:?}",
        shared.log()
    );
    report_ov(&mut harness, &shared, 100, 90);
    harness.step();
    harness.step();
    assert!(set_enabled(&harness, 1));
    assert_eq!(count(&shared, RealtimeCommand::FeedOverrideMinus10), 0);
}

#[test]
fn test_set_percent_disabled_while_disconnected() {
    let (harness, _shared) = pane_app(Duration::from_millis(1), false);
    assert!(!set_enabled(&harness, 0));
    assert!(!set_enabled(&harness, 1));
}
