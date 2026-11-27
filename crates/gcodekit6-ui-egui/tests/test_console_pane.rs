//! The Console pane: what it shows of the machine's traffic, its bounded buffer, its
//! command entry and the once-per-frame delivery of events to it.

use std::time::Duration;

use egui_kittest::kittest::Queryable;
use gcodekit6_app::i18n::Translator;
use gcodekit6_app::machine::{MachineController, MachineEvent};
use gcodekit6_app::test_support::Wrote;
use gcodekit6_communication::firmware::grbl::status_parser::StatusParser;
use gcodekit6_ui_egui::machine::Machine;
use gcodekit6_ui_egui::panes::console::{ConsolePane, MAX_CONSOLE_LINES, response_meaning};

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
    let shared = std::sync::Arc::new(gcodekit6_app::test_support::LoopbackShared::default());
    let mut machine = Machine::new(gcodekit6_app::test_support::loopback_controller(
        &shared,
        Duration::from_millis(1),
    ));
    gcodekit6_ui_egui::machine::lock(machine.controller())
        .connect(&gcodekit6_app::test_support::tcp_settings())
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
        .dispatch(gcodekit6_ui_egui::commands::Command::SoftReset);
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

/// A fresh scratch folder under `target/temp` for this test.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/console_history")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

const ENTRY: &str = "Type a command and press Enter...";

/// An app connected to a loopback machine whose console keeps its history in `file`.
fn history_app(
    file: &std::path::Path,
) -> (
    egui_kittest::Harness<'static, gcodekit6_ui_egui::GcodeKitApp>,
    std::sync::Arc<gcodekit6_app::test_support::LoopbackShared>,
) {
    let (state, shared) = crate::machine_support::loopback_state(Duration::from_millis(1));
    let mut state = state.with_history_file(file.to_path_buf());
    crate::machine_support::connect(&mut state);
    let mut harness =
        crate::machine_support::app_harness(gcodekit6_ui_egui::GcodeKitApp::from_state(state));
    harness.step();
    harness.step();
    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, ENTRY)
        .click();
    harness.step();
    (harness, shared)
}

/// A command typed in one session is in the next session's history (GTK's
/// `command_history.json`), and Up brings it back.
#[test]
fn test_console_history_kept_across_sessions() {
    let file = scratch("across_sessions").join(gcodekit6_app::command_history::HISTORY_FILE);
    {
        let (mut harness, shared) = history_app(&file);
        harness
            .get_by_role_and_label(egui::accesskit::Role::TextInput, ENTRY)
            .type_text("G0 X7");
        harness.step();
        harness.key_press(egui::Key::Enter);
        harness.step();
        assert!(
            wait_until(|| shared
                .log()
                .iter()
                .any(|w| matches!(w, Wrote::Line(l) if l.trim() == "G0 X7"))),
            "line not written"
        );
    }
    let saved = gcodekit6_app::command_history::CommandHistory::load_from(&file);
    assert_eq!(saved.get(0), Some("G0 X7"));

    let (mut harness, _shared) = history_app(&file);
    harness.key_press(egui::Key::ArrowUp);
    harness.step();
    let entry = harness.get_by_role_and_label(egui::accesskit::Role::TextInput, ENTRY);
    assert_eq!(entry.value().as_deref(), Some("G0 X7"));
}

/// GTK's job-end lines: "Job Completed" when every line is acknowledged, then "Work
/// completed in: m:ss" when the controller reports the program's end; "Job stopped:" with
/// the lines done when a job ends early.
#[test]
fn test_console_logs_the_end_of_a_job() {
    let (mut pane, tr) = console();
    let progress = MachineEvent::StreamProgress {
        sent: 1,
        acked: 0,
        total: 2,
    };
    pane.apply(
        &[
            progress.clone(),
            MachineEvent::StreamFinished { lines: 2 },
            received("[MSG:Pgm End]"),
        ],
        &tr,
    );
    assert_eq!(
        pane.lines(),
        vec![
            "Job Completed".to_string(),
            "[MSG:Pgm End]".to_string(),
            "\u{2705} Work completed in: 0:00".to_string(),
        ]
    );
    pane.apply(
        &[
            progress,
            MachineEvent::StreamStopped { acked: 1, total: 2 },
            received("[MSG:Program End]"),
        ],
        &tr,
    );
    let lines = pane.lines();
    assert_eq!(lines[3], "Job stopped: 1 / 2");
    assert_eq!(
        lines.len(),
        5,
        "no work time after a stopped job: {lines:?}"
    );
    assert_eq!(
        gcodekit6_ui_egui::panes::console::work_time(std::time::Duration::from_secs(125)),
        "2:05"
    );
}

/// Clear console empties the log: its "Error: ALARM:1" line goes (Machine Control may still
/// show the alarm itself).
#[test]
fn test_console_clear_empties_the_log() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    shared.inject("ALARM:1\n");
    let deadline = std::time::Instant::now() + crate::machine_support::EVENTUALLY;
    while harness.query_by_label("Error: ALARM:1").is_none() && std::time::Instant::now() < deadline
    {
        harness.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        harness.query_by_label("Error: ALARM:1").is_some(),
        "never logged"
    );
    harness.get_by_label("Clear console").click();
    harness.step();
    harness.step();
    assert!(
        harness.query_by_label("Error: ALARM:1").is_none(),
        "log cleared"
    );
}

/// GRBL 1.1 and grblHAL report M2/M30 as `[MSG:Pgm End]`; GTK's `Program End` is matched
/// too. A line typed after the job forgets its start; a realtime byte does not.
#[test]
fn test_console_program_end_lines_and_a_later_manual_m2() {
    use gcodekit6_ui_egui::panes::console::is_program_end;
    assert!(is_program_end("[MSG:Pgm End]"));
    assert!(is_program_end("[MSG:Program End]"));
    assert!(!is_program_end("ok"));
    let (mut pane, tr) = console();
    let progress = MachineEvent::StreamProgress {
        sent: 1,
        acked: 0,
        total: 1,
    };
    pane.apply(
        &[
            progress.clone(),
            MachineEvent::StreamFinished { lines: 1 },
            MachineEvent::Sent("! (Feed hold)".to_string()),
            MachineEvent::Sent("~ (Cycle start)".to_string()),
            received("[MSG:Pgm End]"),
        ],
        &tr,
    );
    assert!(
        pane.lines()
            .last()
            .is_some_and(|l| l.contains("Work completed in:")),
        "a realtime byte keeps the job: {:?}",
        pane.lines()
    );
    pane.apply(
        &[
            progress,
            MachineEvent::StreamFinished { lines: 1 },
            MachineEvent::Sent("M2".to_string()),
            received("[MSG:Pgm End]"),
        ],
        &tr,
    );
    assert_eq!(
        pane.lines().last().map(String::as_str),
        Some("[MSG:Pgm End]"),
        "a typed M2 after the job reports no stale time"
    );
}

/// `error:N` / `ALARM:N` replies decode to the GRBL 1.1 (or grblHAL) text; other lines and
/// codes without a known text have no meaning.
#[test]
fn test_console_response_meaning_decodes_errors_and_alarms() {
    assert_eq!(
        response_meaning("error:22").as_deref(),
        Some("Feed rate has not yet been set or is undefined.")
    );
    assert!(
        response_meaning("ALARM:1")
            .expect("alarm 1 has a meaning")
            .starts_with("Hard limit")
    );
    assert_eq!(
        response_meaning("ALARM:10").as_deref(),
        Some("EStop asserted. Clear and reset")
    );
    // The worker logs a rejected line as an error, possibly with its program line.
    assert_eq!(
        response_meaning("Error: error:22 on program line 3").as_deref(),
        Some("Feed rate has not yet been set or is undefined.")
    );
    assert_eq!(response_meaning("error:200"), None);
    assert_eq!(response_meaning("ok"), None);
    assert_eq!(response_meaning("[MSG:error:22]"), None);
}

/// An `error:N` reply is shown with its meaning next to its row, and is logged unchanged.
#[test]
fn test_console_shows_error_meaning_in_app() {
    let (mut harness, shared) = pane_app(Duration::from_millis(1), true);
    shared.inject("error:22\n");
    let meaning = "Feed rate has not yet been set or is undefined.";
    let deadline = std::time::Instant::now() + crate::machine_support::EVENTUALLY;
    while harness.query_by_label(meaning).is_none() && std::time::Instant::now() < deadline {
        harness.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        harness.query_by_label(meaning).is_some(),
        "the meaning of error:22 is not shown"
    );
    // The logged row itself is unchanged.
    assert!(harness.query_by_label("Error: error:22").is_some());
}
