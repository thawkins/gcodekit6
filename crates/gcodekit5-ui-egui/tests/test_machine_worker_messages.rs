//! The machine worker's own messages (refusals, drops, transport failures) are worded in
//! English by the toolkit-free controller and translated where the UI shows them: the
//! console, the status bar and the Machine Control pane. The controller's replies
//! (`error:N`, `ALARM:N`) are protocol and stay as they are.

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::{MachineController, MachineEvent};
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_ui_egui::machine::MachineStatus;
use gcodekit5_ui_egui::panes::console::ConsolePane;
use gcodekit5_ui_egui::status_bar;

fn error(message: &str) -> MachineEvent {
    MachineEvent::Error(message.to_string())
}

/// The console's lines after `events`, in German.
fn german_console(events: &[MachineEvent]) -> Vec<String> {
    let controller = std::sync::Arc::new(std::sync::Mutex::new(MachineController::new()));
    let mut pane = ConsolePane::new(controller);
    pane.apply(events, &Translator::new("de"));
    pane.lines()
}

#[test]
fn test_worker_messages_are_translated_in_the_console() {
    let lines = german_console(&[
        error("cycle start ignored: probe stream halting"),
        error("dropped: controller reset after probe error: G0 X7"),
        MachineEvent::StreamHalted {
            reason: "error:9 on program line 41".to_string(),
        },
        error("write failed: broken pipe"),
        error("ALARM:1"),
    ]);
    assert_eq!(lines.len(), 5, "{lines:?}");
    assert!(!lines[0].contains("cycle start ignored"), "{lines:?}");
    assert!(!lines[1].contains("dropped"), "{lines:?}");
    assert!(lines[1].ends_with(": G0 X7"), "{lines:?}");
    assert!(!lines[2].contains("on program line"), "{lines:?}");
    assert!(lines[2].contains("error:9 "), "{lines:?}");
    assert!(lines[2].ends_with(" 41"), "{lines:?}");
    assert!(!lines[3].contains("write failed"), "{lines:?}");
    assert!(lines[3].ends_with(": broken pipe"), "{lines:?}");
    assert!(lines[4].ends_with("ALARM:1"), "{lines:?}");
}

#[test]
fn test_worker_messages_are_translated_in_the_status_bar() {
    let mut status = MachineStatus::default();
    status.apply(&[error("cycle start ignored: probe stream halting")]);
    let summary = status_bar::summary(&status, MeasurementSystem::Metric, &Translator::new("de"));
    let shown = summary.error.unwrap_or_default();
    assert!(!shown.is_empty());
    assert!(!shown.contains("cycle start ignored"), "{shown}");
}

#[test]
fn test_engraving_worker_refusals_are_translated() {
    use gcodekit5_ui_egui::panes::cam_tools::bitmap_engraving::refusal_text;
    let tr = Translator::new("de");
    let shown = refusal_text("Failed to load image file: no such file", &tr);
    assert!(!shown.contains("Failed to load"), "{shown}");
    assert!(shown.ends_with(": no such file"), "{shown}");
    let shown = refusal_text("The engraving worker stopped", &tr);
    assert_ne!(shown, "The engraving worker stopped");
}

#[test]
fn test_probe_failure_text_translates_the_controller_message() {
    use gcodekit5_ui_egui::panes::cam_tools::probe_routines::run::Outcome;
    let (mut run, _state, _shared) = crate::test_cam_probe_backstop::started_run();
    let tr = Translator::new("de");
    let outcomes = run.handle(
        &[
            MachineEvent::StreamHalted {
                reason: "error:9 on program line 41".to_string(),
            },
            MachineEvent::StreamStopped {
                acked: 41,
                total: 60,
            },
        ],
        &tr,
    );
    let text = outcomes
        .iter()
        .find_map(|o| match o {
            Outcome::Failed(text) => Some(text.clone()),
            _ => None,
        })
        .expect("the run failed");
    assert!(text.contains("error:9 "), "{text}");
    assert!(!text.contains("on program line"), "{text}");
}

#[test]
fn test_fixed_machine_errors_are_translated() {
    use gcodekit5_app::machine::MachineError;
    let tr = Translator::new("de");
    for error in [
        MachineError::NotConnected,
        MachineError::AlreadyConnected,
        MachineError::StreamInProgress,
    ] {
        let english = error.to_string();
        let shown = gcodekit5_ui_egui::machine::worker_message(&english, &tr);
        assert_ne!(shown, english, "{error:?} not translated");
    }
    // As the console shows a refusal reported as an event (Pause without a connection).
    let lines = german_console(&[error(&MachineError::NotConnected.to_string())]);
    assert!(!lines[0].contains("not connected"), "{lines:?}");
}

/// A line the worker dropped because the controller reset itself is shown in the console,
/// translated, with the line.
#[test]
fn test_line_not_sent_after_reset_is_shown_translated() {
    let lines = german_console(&[error(
        "not sent: the controller was reset: $J=G91 G21 Y1.00 F1000",
    )]);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("nicht gesendet"), "{lines:?}");
    assert!(lines[0].ends_with(": $J=G91 G21 Y1.00 F1000"), "{lines:?}");
}
