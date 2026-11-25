//! The status bar: what it summarises of the machine status, in the user's units.

use std::time::Duration;

use craft_ui::docking::{Action, Placement};

use egui_kittest::kittest::{NodeT, Queryable};
use gcodekit5_app::i18n::Translator;
use gcodekit5_communication::firmware::grbl::status_parser::FullStatus;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_ui_egui::machine::{MachineStatus, Progress};
use gcodekit5_ui_egui::status_bar::{position_text, summary};

use gcodekit5_app::test_support::Wrote;
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::workspace::PaneId;

use crate::machine_support::{
    app_harness, loopback_state, start_held_stream, step_until, wait_until,
};
use crate::test_machine_pane::pane_app;

fn report(line: &str) -> FullStatus {
    gcodekit5_communication::firmware::grbl::status_parser::StatusParser::parse_full(line)
}

#[test]
fn test_status_bar_disconnected_shows_only_connection() {
    let tr = Translator::new("en");
    let s = summary(&MachineStatus::default(), MeasurementSystem::Metric, &tr);
    assert_eq!(s.connection, "Disconnected");
    assert!(!s.connected);
    assert_eq!(s.state, None);
    assert_eq!(s.position, None);
    assert_eq!(s.progress, None);
    assert_eq!(s.error, None);
}

#[test]
fn test_status_bar_position_follows_units() {
    let xyz = [25.4, 1.23456, -3.1];
    assert_eq!(
        position_text(xyz, MeasurementSystem::Metric),
        "X: 25.40  Y: 1.23  Z: -3.10 (mm)"
    );
    assert_eq!(
        position_text(xyz, MeasurementSystem::Imperial),
        "X: 1.000  Y: 0.049  Z: -0.122 (in)"
    );
}

#[test]
fn test_status_bar_connected_summary() {
    let tr = Translator::new("en");
    let status = MachineStatus {
        connected: true,
        streaming: true,
        progress: Some(Progress {
            sent: 30,
            acked: 25,
            total: 100,
        }),
        report: Some(report("<Run|MPos:1.00000,2.00000,3.00000|FS:500,0>")),
        last_error: Some("boom".to_string()),
        ..MachineStatus::default()
    };
    let s = summary(&status, MeasurementSystem::Metric, &tr);
    assert_eq!(s.connection, "Connected");
    assert_eq!(s.state.as_deref(), Some("Run"));
    assert_eq!(
        s.position.as_deref(),
        Some("X: 1.00  Y: 2.00  Z: 3.00 (mm)")
    );
    assert_eq!(s.progress, Some((0.25, "25.0%".to_string())));
    assert_eq!(s.error.as_deref(), Some("boom"));
}

#[test]
fn test_status_bar_shows_position_in_app() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    shared.inject("<Idle|MPos:1.23456,2.00000,-3.10000|FS:0,0>\n");
    assert!(step_until(&mut harness, |app| app
        .state()
        .machine
        .status()
        .report
        .is_some()));
    harness.step();
    assert!(
        harness
            .query_by_label("X: 1.23  Y: 2.00  Z: -3.10 (mm)")
            .is_some()
    );
}

/// Hides the Machine Control pane behind the visualizer's tab.
fn hide_machine_control(harness: &mut egui_kittest::Harness<'_, GcodeKitApp>) {
    let workspace = harness.state_mut().workspace_mut();
    workspace
        .apply(Action::Move {
            panel: PaneId::MachineControl,
            anchor: PaneId::Visualizer,
            placement: Placement::Tab { before: None },
        })
        .expect("move the pane into the visualizer's tabs");
    workspace
        .apply(Action::Activate {
            panel: PaneId::Visualizer,
        })
        .expect("show the visualizer");
    harness.step();
    harness.step();
}

#[test]
fn test_status_bar_estop_disabled_while_disconnected() {
    let (mut harness, _shared) = pane_app(Duration::from_millis(1), false);
    hide_machine_control(&mut harness);
    assert!(
        harness
            .get_by_label("E-STOP")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn test_status_bar_estop_stops_stream_without_machine_control() {
    let (mut state, shared) = loopback_state(Duration::from_millis(20));
    start_held_stream(&mut state, &shared, 50);
    let mut harness = app_harness(GcodeKitApp::from_state(state));
    harness.step();
    hide_machine_control(&mut harness);
    let estop = harness.get_by_label("E-STOP");
    assert!(!estop.accesskit_node().is_disabled());
    estop.click();
    harness.step();
    harness.step();
    assert!(
        wait_until(|| shared.log().contains(&Wrote::Realtime(0x18))),
        "no 0x18: {:?}",
        shared.log()
    );
    assert!(
        step_until(&mut harness, |app| !app.state().machine.status().streaming),
        "stream still running"
    );
}
