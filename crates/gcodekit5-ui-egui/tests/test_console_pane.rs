//! The Console pane: what it shows of the machine's traffic, its bounded buffer, its
//! command entry and the once-per-frame delivery of events to it.

use std::time::Duration;

use egui_kittest::kittest::Queryable;
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::{MachineController, MachineEvent};
use gcodekit5_app::test_support::Wrote;
use gcodekit5_communication::firmware::grbl::status_parser::StatusParser;
use gcodekit5_ui_egui::machine::Machine;
use gcodekit5_ui_egui::panes::console::{ConsolePane, MAX_CONSOLE_LINES};

use crate::machine_support::{step_until, wait_until};
use crate::test_machine_pane::pane_app;

/// A console over an unconnected controller, and an English translator.
fn console() -> (ConsolePane, Translator) {
    let controller = std::sync::Arc::new(std::sync::Mutex::new(MachineController::new()));
    (ConsolePane::new(controller), Translator::new("en"))
}

fn received(line: &str) -> MachineEvent {
    MachineEvent::Received(line.to_string())
}

#[test]
fn test_console_filters_status_reports() {
    let (mut pane, tr) = console();
    pane.apply(
        &[
            MachineEvent::StatusReport(Box::new(StatusParser::parse_full("<Idle|MPos:0,0,0>"))),
            received("<Idle|MPos:0.000,0.000,0.000|FS:0,0>"),
            received("[MSG:hello]"),
            MachineEvent::StreamProgress {
                sent: 1,
                acked: 1,
                total: 2,
            },
        ],
        &tr,
    );
    assert_eq!(pane.lines(), vec!["[MSG:hello]".to_string()]);
}

#[test]
fn test_console_buffer_is_bounded() {
    let (mut pane, tr) = console();
    let events: Vec<MachineEvent> = (0..MAX_CONSOLE_LINES + 100)
        .map(|i| received(&format!("line {i}")))
        .collect();
    pane.apply(&events, &tr);
    let lines = pane.lines();
    assert_eq!(lines.len(), MAX_CONSOLE_LINES);
    assert_eq!(lines.first().map(String::as_str), Some("line 100"));
    assert_eq!(
        lines.last().map(String::as_str),
        Some(format!("line {}", MAX_CONSOLE_LINES + 99).as_str())
    );
}

#[test]
fn test_console_logs_connection_and_errors() {
    let (mut pane, tr) = console();
    pane.apply(
        &[
            MachineEvent::Connected,
            MachineEvent::Error("port gone".to_string()),
            MachineEvent::Disconnected,
        ],
        &tr,
    );
    assert_eq!(
        pane.lines(),
        vec![
            "Device connected".to_string(),
            "Error: port gone".to_string(),
            "Device disconnected".to_string()
        ]
    );
}

#[test]
fn test_machine_pending_events_are_taken_once() {
    let shared = std::sync::Arc::new(gcodekit5_app::test_support::LoopbackShared::default());
    let mut machine = Machine::new(gcodekit5_app::test_support::loopback_controller(
        &shared,
        Duration::from_millis(1),
    ));
    gcodekit5_ui_egui::machine::lock(machine.controller())
        .connect(&gcodekit5_app::test_support::tcp_settings())
        .expect("connect");
    assert!(crate::machine_support::wait_machine(&mut machine, |s| {
        s.connected
    }));
    assert_eq!(machine.take_pending(), vec![MachineEvent::Connected]);
    assert!(machine.take_pending().is_empty());
    machine.absorb(vec![received("late")]);
    assert_eq!(machine.take_pending(), vec![received("late")]);
}

#[test]
fn test_console_shows_received_lines_in_app() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    shared.inject("[MSG:Hello console]\n");
    let hello = received("[MSG:Hello console]");
    assert!(
        step_until(&mut harness, |app| app
            .state()
            .machine
            .events()
            .contains(&hello)),
        "the line never arrived"
    );
    harness.step();
    assert!(harness.query_by_label("[MSG:Hello console]").is_some());
}

#[test]
fn test_console_entry_sends_line_and_echoes_it() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    let entry = harness.get_by_role_and_label(
        egui::accesskit::Role::TextInput,
        "Type a command and press Enter...",
    );
    entry.click();
    harness.step();
    harness
        .get_by_role_and_label(
            egui::accesskit::Role::TextInput,
            "Type a command and press Enter...",
        )
        .type_text("G0 X5");
    harness.step();
    harness.key_press(egui::Key::Enter);
    harness.step();
    assert!(
        wait_until(|| shared.log().iter().any(|w| matches!(
            w,
            Wrote::Line(l) if l.trim() == "G0 X5"
        ))),
        "line not written: {:?}",
        shared.log()
    );
    harness.step();
    assert!(harness.query_by_label("> G0 X5").is_some());
}

#[test]
fn test_console_echoes_home_from_machine_control() {
    let (mut harness, _shared) = pane_app(Duration::from_millis(1), true);
    harness.get_by_label("Home").click();
    let sent = MachineEvent::Sent("$H".to_string());
    assert!(
        step_until(&mut harness, |app| app
            .state()
            .machine
            .events()
            .contains(&sent)),
        "no Sent event"
    );
    harness.step();
    assert!(harness.query_by_label("> $H").is_some());
}

#[test]
fn test_console_echoes_soft_reset_label() {
    let (mut harness, _shared) = pane_app(Duration::from_millis(1), true);
    harness
        .state_mut()
        .dispatch(gcodekit5_ui_egui::commands::Command::SoftReset);
    let sent = MachineEvent::Sent("0x18 (Reset)".to_string());
    assert!(
        step_until(&mut harness, |app| app
            .state()
            .machine
            .events()
            .contains(&sent)),
        "no Sent event"
    );
    harness.step();
    assert!(harness.query_by_label("> 0x18 (Reset)").is_some());
}

/// Copy last error (GTK's console header button) takes the newest error or alarm line,
/// skipping acknowledgements and status reports.
#[test]
fn test_console_last_error_is_newest_error_or_alarm() {
    let (mut pane, tr) = console();
    assert_eq!(pane.last_error(), None);
    pane.apply(
        &[
            received("error:9"),
            received("ALARM:1"),
            received("ok"),
            received("[MSG:hello]"),
        ],
        &tr,
    );
    assert_eq!(pane.last_error().as_deref(), Some("ALARM:1"));
}

/// The console header's icon buttons keep their names, and Copy last error puts the line
/// on the clipboard.
#[test]
fn test_console_header_buttons_in_app() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    shared.inject("ALARM:1\n");
    let deadline = std::time::Instant::now() + crate::machine_support::EVENTUALLY;
    while harness.query_all_by_label("ALARM:1").next().is_none()
        && std::time::Instant::now() < deadline
    {
        harness.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        harness.query_all_by_label("ALARM:1").next().is_some(),
        "alarm never logged"
    );
    assert!(harness.query_by_label("Clear console").is_some());
    harness.get_by_label("Copy last error").click();
    harness.step();
    let copied = harness
        .output()
        .platform_output
        .commands
        .iter()
        .any(|c| matches!(c, egui::OutputCommand::CopyText(text) if text.ends_with("ALARM:1")));
    assert!(copied, "{:?}", harness.output().platform_output.commands);
}
