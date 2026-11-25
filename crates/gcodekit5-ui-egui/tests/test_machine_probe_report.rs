//! `[PRB:...]` probe reports in the machine status: read in mm whatever `$13` says, the
//! same way status reports are converted for the DRO (the M2 carry: GTK converted them,
//! the egui status did not).

use std::time::Duration;

use gcodekit5_app::machine::MachineEvent;
use gcodekit5_ui_egui::machine::{MachineStatus, probe_report};

use crate::machine_support::{connect, loopback_state, wait_machine};

#[test]
fn test_probe_report_converted_for_report_inches() {
    let mut status = MachineStatus::default();
    status.apply(&[MachineEvent::Received(
        "[PRB:1.000,-2.000,-12.700:1]".into(),
    )]);
    let mm = status.probe.expect("a probe report");
    assert_eq!(
        (mm.position.x, mm.position.z, mm.success),
        (1.0, -12.7, true)
    );
    status.apply(&[
        MachineEvent::Received("$13=1".into()),
        MachineEvent::Received("[PRB:0.5000,-1.0000,-0.5000:0]".into()),
    ]);
    let inch = status.probe.expect("a probe report");
    assert_eq!(inch.position.x, 12.7);
    assert_eq!(inch.position.y, -25.4);
    assert_eq!(inch.position.z, -12.7);
    assert!(!inch.success);
    // A disconnect forgets the report.
    status.apply(&[MachineEvent::Disconnected]);
    assert_eq!(status.probe, None);
    assert_eq!(probe_report("ok", false), None);
}

#[test]
fn test_probe_report_from_the_controller_in_inch_mode() {
    let (mut state, shared) = loopback_state(Duration::from_millis(1));
    connect(&mut state);
    shared.inject("$13=1\r\n[PRB:0.0394,0.0787,-0.5000:1]\r\n");
    assert!(
        wait_machine(&mut state.machine, |s| s.probe.is_some()),
        "no probe report"
    );
    let probe = state.machine.status().probe.expect("a probe report");
    assert_eq!(probe.position.z, -12.7);
    assert!((probe.position.x - 1.000_76).abs() < 1e-4);
}
