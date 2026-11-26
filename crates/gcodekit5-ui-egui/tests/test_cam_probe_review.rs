//! Probe Tools review fixes: the routine streams in the controller's stop-on-error mode (a
//! refused line is followed by a feed hold then a reset, and nothing more of the routine),
//! a result stops being applicable after an alarm, a reset or homing, values follow the
//! mm/inch setting while the lines sent stay the same, and a problem with a field the
//! routine does not show is named on the page.

use std::time::Duration;

use egui_kittest::kittest::Queryable;
use gcodekit5_app::test_support::Wrote;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_ui_egui::machine::lock;
use gcodekit5_ui_egui::panes::cam_tools::CamTool;
use gcodekit5_ui_egui::panes::cam_tools::probe_routines::{ProbeParams, RoutineKind};

use crate::test_cam_drill_press::{click, open_tool_tall, pane};
use crate::test_cam_probe_routines::start;
use crate::test_cam_probe_run::{
    IDLE, lines, probe_app, probe_app_with, reply, result, settle, until, z_touch_with,
};
use crate::test_designer_files_app::frames;

/// Makes the last probe result stale.
type Invalidate = fn(
    &mut egui_kittest::Harness<'static, gcodekit5_ui_egui::GcodeKitApp>,
    &gcodekit5_app::test_support::LoopbackShared,
);

const IMPERIAL: MeasurementSystem = MeasurementSystem::Imperial;

#[test]
fn test_cam_probe_error_holds_then_resets_in_the_controller() {
    // Acks are not held: the worker is feeding the routine when the error arrives.
    let (mut h, shared) = probe_app_with(true, IDLE, Duration::from_millis(40), false);
    {
        let mut cam = pane(&h);
        let p = cam.probe_mut();
        p.routine = RoutineKind::CornerFind;
        p.auto_update = true;
    }
    frames(&mut h, 2);
    // The Machine Control pane asks `$G` after the connect. Replies come in order, so while
    // its `ok` (40 ms) is owed the injected `error:9` would answer `$G`, not a routine line.
    until(&mut h, "the `$G` reply", |app| {
        lines(&shared).iter().any(|l| l == "$G")
            && lock(app.state().machine.controller()).lines_answered()
    });
    let before = shared.log().len();
    click(&mut h, "Probe");
    until(&mut h, "the first lines", |_| lines(&shared).len() >= 3);
    reply(&shared, "error:9");
    // The feed hold completes (GRBL reports `Hold:0` after `!`, never `Idle`).
    reply(&shared, "<Hold:0|MPos:1.000,2.000,3.000|FS:0,0>");
    until(&mut h, "the reset", |_| {
        shared.log().contains(&Wrote::Realtime(0x18))
    });
    let log = shared.log()[before..].to_vec();
    let hold = log.iter().position(|w| *w == Wrote::Realtime(b'!'));
    let reset = log.iter().position(|w| *w == Wrote::Realtime(0x18));
    assert!(hold.is_some() && hold < reset, "{log:?}");
    let hold = hold.unwrap_or_default();
    let after: Vec<&Wrote> = log[hold..]
        .iter()
        .filter(|w| matches!(w, Wrote::Line(l) if l != "G90"))
        .collect();
    assert!(after.is_empty(), "routine lines after the hold: {after:?}");
    // The Y result's WCS write comes after the error: it is never sent (the X one was
    // already in the controller's buffer, which the reset empties).
    assert!(
        !log.iter()
            .any(|w| matches!(w, Wrote::Line(l) if l.starts_with("G10 L20 P1 Y"))),
        "the G10 after the error was sent"
    );
    until(&mut h, "the failure text", |app| {
        result(app).contains("error:9")
    });
    // The machine reports Idle: G90 is restored and the run ends.
    reply(&shared, IDLE);
    until(&mut h, "the run's end", |app| {
        !app.cam_tools().borrow().probe_running()
    });
}

#[test]
fn test_cam_probe_result_is_dropped_after_alarm_reset_or_homing() {
    let cases: [(&str, Invalidate); 3] = [
        ("alarm", |_, shared| reply(shared, "ALARM:1")),
        ("reset", |_, shared| {
            reply(shared, "Grbl 1.1h ['$' for help]")
        }),
        ("homing", |h, _| {
            let sent = lock(h.state().state().machine.controller()).send_line("$H");
            assert_eq!(sent, Ok(()));
        }),
    ];
    for (name, invalidate) in cases {
        let (mut h, shared) = probe_app(true, IDLE);
        z_touch_with(&mut h, &shared, "[PRB:1.000,2.000,-12.700:1]");
        until(&mut h, "Idle", |app| !app.state().machine.job_running());
        assert!(pane(&h).probe_last_result().is_some());
        invalidate(&mut h, &shared);
        until(&mut h, name, |app| {
            app.cam_tools().borrow().probe_last_result().is_none()
        });
        let before = lines(&shared).len();
        click(&mut h, "Apply to WCS");
        settle(&mut h, 100);
        assert!(
            !lines(&shared)[before..]
                .iter()
                .any(|l| l.starts_with("G10")),
            "{name}: applied a stale result"
        );
    }
}

#[test]
fn test_cam_probe_values_follow_the_measurement_system() {
    let mut p = ProbeParams::new(IMPERIAL);
    assert_eq!(p.safe_height, "0.197");
    assert_eq!(p.max_depth, "0.787");
    // Feeds stay in mm/min, as on every CAM page.
    assert_eq!(p.fast_feed, "100.00");
    let mut metric = ProbeParams::new(MeasurementSystem::Metric);
    metric.set_units(IMPERIAL);
    assert_eq!(metric.safe_height, "0.197");
    p.set_units(MeasurementSystem::Metric);
    // The same physical values typed in inches make the same lines.
    let mut metric = ProbeParams::new(MeasurementSystem::Metric);
    metric.safe_height = "6.35".into();
    metric.max_depth = "25.4".into();
    metric.backoff = "2.54".into();
    metric.plate_thickness = "1.27".into();
    let mut inch = ProbeParams::new(IMPERIAL);
    inch.safe_height = "0.25".into();
    inch.max_depth = "1".into();
    inch.backoff = "0.1".into();
    inch.plate_thickness = "0.05".into();
    assert_eq!(inch.run_lines(start()), metric.run_lines(start()));
    let lines = inch.run_lines(start()).expect("lines");
    assert!(
        lines.iter().any(|l| l == "G38.2 Z-25.400 F100.0"),
        "{lines:?}"
    );
    assert!(lines.iter().any(|l| l == "G0 Z6.350"), "{lines:?}");
    // Generate's program too.
    assert_eq!(
        inch.generate(start()).map(|o| o.gcode),
        metric.generate(start()).map(|o| o.gcode)
    );
}

#[test]
fn test_cam_probe_inch_page_shows_inches_and_sends_mm_lines() {
    let (mut h, shared) = probe_app(true, IDLE);
    {
        let mut cam = pane(&h);
        let p = cam.probe_mut();
        p.set_units(IMPERIAL);
        p.safe_height = "0.25".into();
        p.max_depth = "1".into();
        p.backoff = "0.1".into();
    }
    frames(&mut h, 2);
    assert!(h.query_all_by_label("in").count() > 0, "no inch unit shown");
    let before = lines(&shared).len();
    click(&mut h, "Probe");
    // GTK's Z-touch lines for 25.4 mm depth, 2.54 mm backoff and 6.35 mm safe height.
    let expected: Vec<String> = [
        "G21",
        "G91",
        "G38.2 Z-25.400 F100.0",
        "G0 Z2.540",
        "G38.2 Z-3.040 F25.0",
        "G0 Z6.350",
        "G90",
    ]
    .map(String::from)
    .to_vec();
    until(&mut h, "the program", |_| {
        lines(&shared).len() >= before + expected.len()
    });
    assert_eq!(
        lines(&shared)[before..before + expected.len()],
        expected[..]
    );
}

#[test]
fn test_cam_probe_names_a_problem_in_a_hidden_field() {
    let mut h = open_tool_tall(CamTool::ProbeRoutines);
    // Z-touch does not show the tip radius.
    pane(&h).probe_mut().tip_radius = "-1".into();
    frames(&mut h, 2);
    assert!(
        h.query_by_label("Probe Tip Radius: Must not be negative")
            .is_some(),
        "the hidden field's problem is not named"
    );
}
