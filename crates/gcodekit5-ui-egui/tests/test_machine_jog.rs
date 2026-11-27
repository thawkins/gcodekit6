//! Jogging from the Machine Control pane: hold-to-jog lasts as long as the press, is clamped
//! to the travel left, and every way a hold can end without a release sends jog cancel
//! (0x85); axis inversion and inch display apply to every jog.

use std::sync::Arc;
use std::time::Duration;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_app::machine::RealtimeCommand;
use gcodekit5_app::test_support::{LoopbackShared, Wrote};
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::commands::Command;
use gcodekit5_ui_egui::workspace::PaneId;

use crate::machine_support::{loopback_config, step_until, wait_until};
use crate::test_machine_pane::{
    pane_app_with, pointer_button, report_idle, report_state, wrote_realtime,
};

/// The `$J` lines written so far.
fn jog_lines(shared: &LoopbackShared) -> Vec<String> {
    shared
        .log()
        .into_iter()
        .filter_map(|w| match w {
            Wrote::Line(line) if line.starts_with("$J=") => Some(line),
            Wrote::Line(_) | Wrote::Realtime(_) => None,
        })
        .collect()
}

/// How many jog cancels were written.
fn cancels(shared: &LoopbackShared) -> usize {
    shared
        .log()
        .iter()
        .filter(|w| **w == Wrote::Realtime(RealtimeCommand::JogCancel.byte()))
        .count()
}

/// A connected app with `config`, a status report at machine position `mpos` (unless
/// `None`) and continuous jogging switched on.
fn hold_app(
    config: Config,
    mpos: Option<[f32; 3]>,
) -> (Harness<'static, GcodeKitApp>, Arc<LoopbackShared>) {
    let (mut harness, shared) = pane_app_with(config, Duration::from_millis(1), true);
    let report = match mpos {
        Some([x, y, z]) => format!("<Idle|MPos:{x:.4},{y:.4},{z:.4}|FS:0,0>"),
        // Idle, but only a work position and no offset: the machine position is unknown.
        None => "<Idle|WPos:0.000,0.000,0.000|FS:0,0>".to_string(),
    };
    report_state(&mut harness, &shared, &report, "Idle");
    harness.get_by_label("Continuous").click();
    harness.step();
    (harness, shared)
}

/// Presses the jog button `label` and keeps the button down; returns where.
fn press(harness: &mut Harness<'_, GcodeKitApp>, label: &str) -> egui::Pos2 {
    let center = harness.get_by_label(label).rect().center();
    pointer_button(harness, center, true);
    harness.step();
    center
}

/// Releases the pointer at `pos`.
fn release(harness: &mut Harness<'_, GcodeKitApp>, pos: egui::Pos2) {
    pointer_button(harness, pos, false);
    harness.step();
    harness.step();
}

/// Waits for exactly `n` jog lines and returns them.
fn wait_jogs(shared: &LoopbackShared, n: usize) -> Vec<String> {
    assert!(
        wait_until(|| jog_lines(shared).len() == n),
        "expected {n} jog lines: {:?}",
        shared.log()
    );
    jog_lines(shared)
}

/// Lets the worker write whatever it was given, then returns the jog cancel count.
fn settled_cancels(shared: &LoopbackShared) -> usize {
    std::thread::sleep(Duration::from_millis(30));
    cancels(shared)
}

/// I1: a hold lasts as long as the press (2.5 s here, past egui's click window) and ends with
/// exactly one jog cancel on release.
#[test]
fn test_hold_survives_long_press_and_cancels_once_on_release() {
    let (mut harness, shared) = hold_app(loopback_config(), Some([-100.0, -100.0, -50.0]));
    let at = press(&mut harness, "Jog Y-");
    assert_eq!(wait_jogs(&shared, 1), ["$J=G91 G21 Y-100.00 F1000"]);
    for _ in 0..10 {
        harness.step();
    }
    assert_eq!(settled_cancels(&shared), 0, "hold ended by itself");
    release(&mut harness, at);
    assert!(wait_until(|| cancels(&shared) == 1), "{:?}", shared.log());
    for _ in 0..4 {
        harness.step();
    }
    assert_eq!(settled_cancels(&shared), 1);
    assert_eq!(jog_lines(&shared).len(), 1);
}

/// I2c: the pointer leaving the button while still pressed ends the hold.
#[test]
fn test_hold_cancelled_when_pointer_leaves_button() {
    let (mut harness, shared) = hold_app(loopback_config(), Some([-100.0, -100.0, -50.0]));
    let at = press(&mut harness, "Jog X+");
    wait_jogs(&shared, 1);
    harness.event(egui::Event::PointerMoved(at + egui::vec2(300.0, 300.0)));
    harness.step();
    assert!(wait_until(|| cancels(&shared) == 1), "{:?}", shared.log());
    release(&mut harness, at + egui::vec2(300.0, 300.0));
    assert_eq!(settled_cancels(&shared), 1);
}

/// I2a: the window losing focus ends the hold (its release would never arrive).
#[test]
fn test_hold_cancelled_when_window_loses_focus() {
    let (mut harness, shared) = hold_app(loopback_config(), Some([-100.0, -100.0, -50.0]));
    press(&mut harness, "Jog X+");
    wait_jogs(&shared, 1);
    harness.input_mut().focused = false;
    harness.step();
    assert!(wait_until(|| cancels(&shared) == 1), "{:?}", shared.log());
    harness.input_mut().focused = true;
}

/// I2b: the Machine Control pane hidden by another page ends the hold.
#[test]
fn test_hold_cancelled_when_pane_hidden() {
    let (mut harness, shared) = hold_app(loopback_config(), Some([-100.0, -100.0, -50.0]));
    press(&mut harness, "Jog X+");
    wait_jogs(&shared, 1);
    harness
        .state_mut()
        .workspace_mut()
        .show_pane(PaneId::Visualizer);
    harness.step();
    assert!(wait_until(|| cancels(&shared) == 1), "{:?}", shared.log());
}

/// I2d: a disconnect ends the hold (the controller writes the cancel itself) and the hold
/// is forgotten: the release after reconnecting sends nothing.
#[test]
fn test_hold_forgotten_on_disconnect() {
    let (mut harness, shared) = hold_app(loopback_config(), Some([-100.0, -100.0, -50.0]));
    let at = press(&mut harness, "Jog X+");
    wait_jogs(&shared, 1);
    harness.state_mut().dispatch(Command::Disconnect);
    assert!(wait_until(|| cancels(&shared) == 1), "{:?}", shared.log());
    harness.state_mut().dispatch(Command::Connect);
    assert!(
        step_until(&mut harness, |app| app.state().machine.status().connected),
        "no reconnect"
    );
    release(&mut harness, at);
    assert_eq!(settled_cancels(&shared), 1, "{:?}", shared.log());
}

/// I2d: starting a stream ends the hold with a cancel ahead of the program.
#[test]
fn test_hold_cancelled_on_stream_start() {
    let (mut harness, shared) = hold_app(loopback_config(), Some([-100.0, -100.0, -50.0]));
    let at = press(&mut harness, "Jog X+");
    wait_jogs(&shared, 1);
    harness
        .state_mut()
        .state_mut()
        .machine
        .set_program("G1 X1 F100\nG1 X2 F100\n")
        .expect("program");
    harness.state_mut().dispatch(Command::StreamStart);
    assert!(
        wait_until(|| shared
            .log()
            .contains(&Wrote::Line("G1 X1 F100".to_string()))),
        "{:?}",
        shared.log()
    );
    let log = shared.log();
    let cancel = log
        .iter()
        .position(|w| *w == Wrote::Realtime(RealtimeCommand::JogCancel.byte()))
        .expect("jog cancel");
    let program = log
        .iter()
        .position(|w| *w == Wrote::Line("G1 X1 F100".to_string()))
        .expect("program line");
    assert!(cancel < program, "{log:?}");
    release(&mut harness, at);
    assert_eq!(settled_cancels(&shared), 1);
}

/// I2d and the soft reset chord: a soft reset during a hold ends it (0x18 stops the jog), so
/// the release sends no cancel.
#[test]
fn test_soft_reset_chord_during_hold_clears_hold() {
    let (mut harness, shared) = hold_app(loopback_config(), Some([-100.0, -100.0, -50.0]));
    let at = press(&mut harness, "Jog X+");
    wait_jogs(&shared, 1);
    harness.key_press_modifiers(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::K,
    );
    harness.step();
    assert!(
        wait_until(|| wrote_realtime(&shared, RealtimeCommand::SoftReset)),
        "{:?}",
        shared.log()
    );
    release(&mut harness, at);
    assert_eq!(settled_cancels(&shared), 0, "{:?}", shared.log());
}

/// I3: a hold moves at most the travel left in its direction, and the controller's
/// `$130` travel replaces the configured one once reported.
#[test]
fn test_hold_clamped_to_remaining_travel() {
    let (mut harness, shared) = hold_app(loopback_config(), Some([-150.0, -100.0, -50.0]));
    let at = press(&mut harness, "Jog X+");
    assert_eq!(wait_jogs(&shared, 1)[0], "$J=G91 G21 X150.00 F1000");
    release(&mut harness, at);
    let at = press(&mut harness, "Jog X-");
    assert_eq!(wait_jogs(&shared, 2)[1], "$J=G91 G21 X-50.00 F1000");
    release(&mut harness, at);

    shared.inject("$130=300.000\n");
    assert!(
        step_until(&mut harness, |app| app.state().machine.status().grbl_travel
            [0]
            == Some(300.0)),
        "no $130"
    );
    let at = press(&mut harness, "Jog X-");
    assert_eq!(wait_jogs(&shared, 3)[2], "$J=G91 G21 X-150.00 F1000");
    release(&mut harness, at);
}

/// I3: at the end of travel the hold does not move and the pane says why.
#[test]
fn test_hold_refused_at_limit() {
    let (mut harness, shared) = hold_app(loopback_config(), Some([0.0, -100.0, -50.0]));
    let at = press(&mut harness, "Jog X+");
    harness.step();
    assert!(jog_lines(&shared).is_empty(), "{:?}", shared.log());
    let _ = harness.get_by_label("Jog refused: no travel left in that direction.");
    release(&mut harness, at);
    assert_eq!(settled_cancels(&shared), 0);
}

/// A hold clamped to less travel than a jog line can write (5 um left: it would be sent
/// as `X0.00`) is refused like one at the limit, with no zero-length jog; with 0.01 mm left
/// it moves exactly that.
#[test]
fn test_hold_refused_when_clamped_below_a_written_step() {
    for (mpos, expected) in [
        (-0.005_f32, None),
        (-0.0049, None),
        (-0.0051, Some("$J=G91 G21 X0.01 F1000")),
    ] {
        let (mut harness, shared) = hold_app(loopback_config(), Some([mpos, -100.0, -50.0]));
        let at = press(&mut harness, "Jog X+");
        harness.step();
        match expected {
            None => {
                let _ = harness.get_by_label("Jog refused: no travel left in that direction.");
                release(&mut harness, at);
                assert!(
                    crate::test_machine_keyboard_jog::settled_jogs(&harness, &shared).is_empty(),
                    "{mpos}: {:?}",
                    shared.log()
                );
            }
            Some(line) => {
                assert_eq!(wait_jogs(&shared, 1), [line], "{mpos}");
                release(&mut harness, at);
            }
        }
    }
}

/// I3: without a machine position the hold does not move and the pane says why.
#[test]
fn test_hold_refused_without_position() {
    let (mut harness, shared) = hold_app(loopback_config(), None);
    let at = press(&mut harness, "Jog X+");
    harness.step();
    assert!(jog_lines(&shared).is_empty(), "{:?}", shared.log());
    let _ = harness.get_by_label("Jog refused: the machine position is unknown.");
    release(&mut harness, at);
    assert_eq!(settled_cancels(&shared), 0);
}

/// C2: Preferences > Machine > Invert reverses an axis for step and hold jogs alike.
#[test]
fn test_inverted_z_jogs_negative() {
    let mut config = loopback_config();
    config
        .machine
        .axis_direction_invert
        .insert("Z".to_string(), true);
    let (mut harness, shared) = pane_app_with(config.clone(), Duration::from_millis(1), true);
    report_idle(&mut harness, &shared);
    harness.get_by_label("Jog Z+").click();
    harness.step();
    assert_eq!(wait_jogs(&shared, 1), ["$J=G91 G21 Z-1.00 F1000"]);
    // The same from a key or menu (the Jog Z+ command).
    harness.state_mut().dispatch(Command::JogZPos);
    assert_eq!(wait_jogs(&shared, 2)[1], "$J=G91 G21 Z-1.00 F1000");
    drop(harness);

    let (mut harness, shared) = hold_app(config, Some([-100.0, -100.0, -40.0]));
    let at = press(&mut harness, "Jog Z+");
    assert_eq!(wait_jogs(&shared, 1), ["$J=G91 G21 Z-60.00 F1000"]);
    release(&mut harness, at);
}

/// C1 (app): exiting during a hold writes jog cancel before the transport closes.
#[test]
fn test_exit_during_hold_cancels_jog() {
    let (mut harness, shared) = hold_app(loopback_config(), Some([-100.0, -100.0, -50.0]));
    press(&mut harness, "Jog X+");
    wait_jogs(&shared, 1);
    harness.state_mut().on_exit_with(|| Ok(()));
    let log = shared.log();
    let cancel = log
        .iter()
        .position(|w| *w == Wrote::Realtime(RealtimeCommand::JogCancel.byte()))
        .expect("no jog cancel on exit");
    let closed = shared
        .closed_at
        .lock()
        .expect("closed_at")
        .expect("transport not closed");
    assert!(cancel < closed, "{log:?}");
}

/// Settings that show inches.
fn inch_config() -> Config {
    let mut config = loopback_config();
    config.ui.measurement_system = MeasurementSystem::Imperial;
    config
}

/// I6: in inch mode the DRO shows inches with three decimals.
#[test]
fn test_dro_in_inches() {
    let (mut harness, shared) = pane_app_with(inch_config(), Duration::from_millis(1), true);
    shared.inject("<Idle|MPos:25.400,-50.800,0.000|FS:0,0>\n");
    assert!(
        step_until(&mut harness, |app| app
            .state()
            .machine
            .status()
            .report
            .is_some()),
        "no status report"
    );
    harness.step();
    assert!(
        !harness
            .query_all_by_label("1.000")
            .collect::<Vec<_>>()
            .is_empty()
    );
    assert!(
        !harness
            .query_all_by_label("-2.000")
            .collect::<Vec<_>>()
            .is_empty()
    );
    assert!(harness.query_by_label("25.40").is_none());
    assert!(harness.query_by_label("1.00").is_none());
}

/// I6: inch step presets jog the matching distance in mm (the `$J` line stays G21).
#[test]
fn test_jog_step_in_inches() {
    let (mut harness, shared) = pane_app_with(inch_config(), Duration::from_millis(1), true);
    report_idle(&mut harness, &shared);
    harness.get_by_label("Step").click();
    // The popup's items appear in the accessibility tree a frame after it opens.
    harness.step();
    harness.step();
    harness.get_by_label("0.1 in").click();
    harness.step();
    harness.get_by_label("Jog X+").click();
    harness.step();
    assert_eq!(wait_jogs(&shared, 1), ["$J=G91 G21 X2.54 F1000"]);
}

/// A started stream counts as a running job before its first progress report.
#[test]
fn test_job_running_right_after_start() {
    let (mut state, shared) = crate::machine_support::loopback_state(Duration::from_millis(1));
    shared
        .hold_acks
        .store(true, std::sync::atomic::Ordering::SeqCst);
    crate::machine_support::connect(&mut state);
    state.machine.set_program("G1 X1 F100\n").expect("program");
    let program = state.machine.program().cloned().expect("program");
    gcodekit5_ui_egui::machine::lock(state.machine.controller())
        .start_stream(program)
        .expect("start");
    assert!(!state.machine.status().streaming, "no progress polled yet");
    assert!(state.job_running());
}

/// `$13=1`: reports in inches are converted to mm before the DRO and the hold clamp use
/// them (X at -150 mm reports -5.9055 in).
#[test]
fn test_report_inches_converted_for_dro_and_clamp() {
    let (mut harness, shared) = pane_app_with(loopback_config(), Duration::from_millis(1), true);
    shared.inject("$13=1\n");
    shared.inject("$130=300.000\n");
    assert!(
        step_until(&mut harness, |app| {
            let status = app.state().machine.status();
            status.report_inches && status.grbl_travel[0] == Some(300.0)
        }),
        "no $13 / $130"
    );
    report_state(
        &mut harness,
        &shared,
        "<Idle|MPos:-5.9055,-3.9370,-1.9685|FS:0,0>",
        "Idle",
    );
    assert!(
        !harness
            .query_all_by_label("-150.00")
            .collect::<Vec<_>>()
            .is_empty()
    );
    assert!(
        !harness
            .query_all_by_label("-100.00")
            .collect::<Vec<_>>()
            .is_empty()
    );
    assert!(harness.query_by_label("-5.91").is_none());
    harness.get_by_label("Continuous").click();
    harness.step();
    let at = press(&mut harness, "Jog X-");
    assert_eq!(wait_jogs(&shared, 1), ["$J=G91 G21 X-150.00 F1000"]);
    release(&mut harness, at);
}

/// Jogging (step button, Jog command and hold) is refused while homing and in alarm.
#[test]
fn test_jog_refused_in_home_and_alarm() {
    for state in ["Home", "Alarm"] {
        let (mut harness, shared) =
            pane_app_with(loopback_config(), Duration::from_millis(1), true);
        let report = format!("<{state}|MPos:-100.000,-100.000,-50.000|FS:0,0>");
        report_state(&mut harness, &shared, &report, state);
        harness.get_by_label("Jog X+").click();
        harness.step();
        harness.state_mut().dispatch(Command::JogXPos);
        harness.step();
        let _ = harness.get_by_label("Jog refused: the machine is not idle.");
        harness.get_by_label("Continuous").click();
        harness.step();
        let at = press(&mut harness, "Jog X+");
        harness.step();
        release(&mut harness, at);
        std::thread::sleep(Duration::from_millis(30));
        assert!(jog_lines(&shared).is_empty(), "{state}: {:?}", shared.log());
    }
}

/// A hold ends with jog cancel as soon as a report leaves Idle/Jog (an alarm here).
#[test]
fn test_hold_ends_when_state_becomes_alarm() {
    let (mut harness, shared) = hold_app(loopback_config(), Some([-100.0, -100.0, -50.0]));
    let at = press(&mut harness, "Jog X+");
    wait_jogs(&shared, 1);
    report_state(
        &mut harness,
        &shared,
        "<Jog|MPos:-90.000,-100.000,-50.000|FS:1000,0>",
        "Jog",
    );
    assert_eq!(settled_cancels(&shared), 0, "Jog state ended the hold");
    report_state(
        &mut harness,
        &shared,
        "<Alarm|MPos:-80.000,-100.000,-50.000|FS:0,0>",
        "Alarm",
    );
    assert!(wait_until(|| cancels(&shared) == 1), "{:?}", shared.log());
    release(&mut harness, at);
    assert_eq!(settled_cancels(&shared), 1);
}

/// A `$13` change drops the cached work offset (and report), which were in the old units;
/// the next report is converted with the new setting.
#[test]
fn test_report_inches_change_drops_cached_offset() {
    use gcodekit5_app::machine::MachineEvent;
    use gcodekit5_communication::firmware::grbl::status_parser::StatusParser;
    use gcodekit5_ui_egui::machine::MachineStatus;
    let mut status = MachineStatus::default();
    let report = |line: &str| MachineEvent::StatusReport(Box::new(StatusParser::parse_full(line)));
    status.apply(&[report(
        "<Idle|MPos:-10.000,0.000,0.000|WCO:-5.000,0.000,0.000|FS:0,0>",
    )]);
    assert!(status.wco.is_some());
    status.apply(&[MachineEvent::Received("$13=1".to_string())]);
    assert!(status.report_inches);
    assert!(status.wco.is_none(), "offset in the old units kept");
    assert!(status.report.is_none(), "report in the old units kept");
    status.apply(&[report("<Idle|MPos:-1.0000,0.000,0.000|FS:0,0>")]);
    let mpos = status.machine_position().expect("position");
    assert!((mpos[0] + 25.4).abs() < 1e-3, "{mpos:?}");
    // The same setting again changes nothing.
    status.apply(&[MachineEvent::Received("$13=1".to_string())]);
    assert!(status.report.is_some());
}

/// What sends a command line in [`test_jog_blocked_after_command_line_until_fresh_report`].
#[derive(Clone, Copy, Debug)]
enum LineSource {
    HomeButton,
    HomeCommand,
    CommandEntry,
}

/// After `$H` (button or command) or an MDI line, the last Idle report may predate the line:
/// no jog (step button, Jog command, hold) runs until a report read after the line.
#[test]
fn test_jog_blocked_after_command_line_until_fresh_report() {
    let idle = "<Idle|MPos:-100.000,-100.000,-50.000|FS:0,0>";
    for source in [
        LineSource::HomeButton,
        LineSource::HomeCommand,
        LineSource::CommandEntry,
    ] {
        let (mut harness, shared) =
            pane_app_with(loopback_config(), Duration::from_millis(1), true);
        report_state(&mut harness, &shared, idle, "Idle");
        let line = match source {
            LineSource::HomeButton => {
                harness.get_by_label("Home").click();
                "$H"
            }
            LineSource::HomeCommand => {
                harness.state_mut().dispatch(Command::Home);
                "$H"
            }
            LineSource::CommandEntry => {
                crate::test_machine_pane::focus_entry(&mut harness);
                crate::test_machine_pane::entry(&harness).type_text("G0 X0");
                harness.step();
                harness.get_by_label("Send Command").click();
                "G0 X0"
            }
        };
        harness.step();
        assert!(
            wait_until(|| shared.log().contains(&Wrote::Line(line.to_string()))),
            "{source:?}: line not written: {:?}",
            shared.log()
        );
        for _ in 0..3 {
            harness.step();
        }
        assert!(!harness.state().state().machine.jog_allowed(), "{source:?}");

        // Step button, Jog command and hold, all refused.
        harness.get_by_label("Jog X+").click();
        harness.step();
        harness.state_mut().dispatch(Command::JogXPos);
        harness.step();
        let _ = harness.get_by_label("Jog refused: the machine is not idle.");
        harness.get_by_label("Continuous").click();
        harness.step();
        let at = press(&mut harness, "Jog X+");
        release(&mut harness, at);
        std::thread::sleep(Duration::from_millis(30));
        assert!(
            jog_lines(&shared).is_empty(),
            "{source:?}: {:?}",
            shared.log()
        );

        // A report read after the line: jogging works again.
        shared.inject(&format!("{idle}\n"));
        assert!(
            step_until(&mut harness, |app| app.state().machine.jog_allowed()),
            "{source:?}: no fresh report"
        );
        harness.step();
        harness.state_mut().dispatch(Command::JogXPos);
        assert_eq!(wait_jogs(&shared, 1).len(), 1, "{source:?}");
        let at = press(&mut harness, "Jog X+");
        assert_eq!(
            wait_jogs(&shared, 2)[1],
            "$J=G91 G21 X100.00 F1000",
            "{source:?}"
        );
        release(&mut harness, at);
    }
}
