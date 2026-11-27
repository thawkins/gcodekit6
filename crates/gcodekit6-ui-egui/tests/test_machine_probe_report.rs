//! `[PRB:...]` probe reports read in mm whatever `$13` says, the same way status reports
//! are converted for the DRO (the M2 carry: GTK converted them).

use gcodekit6_ui_egui::machine::{parse_report_inches, probe_report};

#[test]
fn test_probe_report_converted_for_report_inches() {
    let mm = probe_report("[PRB:1.000,-2.000,-12.700:1]", false).expect("a probe report");
    assert_eq!(
        (mm.position.x, mm.position.z, mm.success),
        (1.0, -12.7, true)
    );
    assert_eq!(parse_report_inches("$13=1"), Some(true));
    let inch = probe_report("[PRB:0.5000,-1.0000,-0.5000:0]", true).expect("a probe report");
    assert_eq!(inch.position.x, 12.7);
    assert_eq!(inch.position.y, -25.4);
    assert_eq!(inch.position.z, -12.7);
    assert!(!inch.success);
    assert_eq!(probe_report("ok", false), None);
    assert_eq!(parse_report_inches("$130=300"), None);
}
