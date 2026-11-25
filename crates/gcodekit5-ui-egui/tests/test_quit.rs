//! Quit flow: every quit source (window close, the Quit command, the OS) goes through
//! `GcodeKitApp::request_quit`. A running job asks "Stop job and quit?" first, then unsaved
//! work asks Save / Discard / Cancel; a clean app quits at once. Nothing here closes the
//! process: the tests read the viewport commands each frame emits and the restart flag.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use craft_dialogs::{Answer, MessageResult, Request};
use egui::{ViewportCommand, ViewportEvent, ViewportId};
use egui_kittest::Harness;
use gcodekit5_app::UnsavedChangesChoice;
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::app::{DirtyState, QuitSource};
use gcodekit5_ui_egui::dialogs::{ConfirmChoice, ConfirmRequest};
use gcodekit5_ui_egui::{AppState, GcodeKitApp};

/// What the app has open when the user quits.
#[derive(Clone, Copy, Debug)]
enum Work {
    EditorDirty,
    DesignerDirty,
    Streaming,
    Clean,
}

const SOURCES: [QuitSource; 3] = [
    QuitSource::WindowClose,
    QuitSource::MenuQuit,
    QuitSource::Os,
];
const WORK: [Work; 4] = [
    Work::EditorDirty,
    Work::DesignerDirty,
    Work::Streaming,
    Work::Clean,
];

/// Titles of the message boxes shown so far, in order.
type Shown = Arc<Mutex<Vec<String>>>;

/// The app in a harness that runs a frame the way eframe does (`logic`, then `ui`, then the
/// dialogs at the end of the frame), with `work` open and every message box answered with
/// `reply`.
fn harness(work: Work, reply: MessageResult) -> (Harness<'static, GcodeKitApp>, Shown) {
    let mut app = GcodeKitApp::from_state(AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    ));
    set_work(app.state_mut(), work);
    let shown: Shown = Arc::default();
    answer_with(&mut app, &shown, reply);
    let harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, app: &mut GcodeKitApp| {
                app.logic(ui.ctx());
                app.ui(ui);
            },
            app,
        );
    (harness, shown)
}

fn set_work(state: &mut AppState, work: Work) {
    state.dirty = DirtyState {
        editor: matches!(work, Work::EditorDirty),
        designer: matches!(work, Work::DesignerDirty),
    };
    state.job_running = matches!(work, Work::Streaming);
}

/// Answers every message box with `reply`, recording its title in `shown`.
fn answer_with(app: &mut GcodeKitApp, shown: &Shown, reply: MessageResult) {
    let shown = Arc::clone(shown);
    app.dialogs_mut()
        .set_override(Some(Box::new(move |request: &Request| match request {
            Request::Message(message) => {
                shown
                    .lock()
                    .expect("shown lock")
                    .push(message.title.clone());
                Answer::Message(reply.clone())
            }
            Request::File(file) => panic!("unexpected file dialog {file:?}"),
        })));
}

/// The custom Save / Discard buttons of the unsaved-changes prompt, as craft-dialogs reports
/// them.
fn save() -> MessageResult {
    MessageResult::Custom("Save".into())
}
fn discard() -> MessageResult {
    MessageResult::Custom("Discard".into())
}

/// One frame; `close` presses the window's close button first. Returns the root viewport's
/// commands of that frame. The end of the frame shows the queued dialog, whose answer the
/// next frame takes.
fn frame(harness: &mut Harness<'_, GcodeKitApp>, close: bool) -> Vec<ViewportCommand> {
    if close {
        harness
            .input_mut()
            .viewports
            .entry(ViewportId::ROOT)
            .or_default()
            .events
            .push(ViewportEvent::Close);
    }
    harness.step();
    let commands = harness
        .output()
        .viewport_output
        .get(&ViewportId::ROOT)
        .map(|output| output.commands.clone())
        .unwrap_or_default();
    harness
        .state_mut()
        .dialogs_mut()
        .end_frame(None::<&eframe::Frame>);
    commands
}

/// Starts a quit from `source`: the close button for [`QuitSource::WindowClose`], else a call
/// to `request_quit` before the frame. Returns the frame's commands.
fn quit_from(harness: &mut Harness<'_, GcodeKitApp>, source: QuitSource) -> Vec<ViewportCommand> {
    match source {
        QuitSource::WindowClose => frame(harness, true),
        QuitSource::MenuQuit | QuitSource::Os => {
            harness.state_mut().request_quit(source);
            frame(harness, false)
        }
    }
}

fn expected_prompt(work: Work) -> &'static str {
    match work {
        Work::EditorDirty | Work::DesignerDirty => "Unsaved changes",
        Work::Streaming => "Stop job and quit?",
        Work::Clean => panic!("a clean app shows no prompt"),
    }
}

/// Clean: every source quits at once with no prompt; the window is not held open.
#[test]
fn test_quit_clean_quits_immediately() {
    for source in SOURCES {
        let (mut harness, shown) = harness(Work::Clean, MessageResult::Cancel);
        let commands = quit_from(&mut harness, source);
        assert!(
            !commands.contains(&ViewportCommand::CancelClose),
            "{source:?}: {commands:?}"
        );
        assert!(
            commands.contains(&ViewportCommand::Close),
            "{source:?}: {commands:?}"
        );
        assert!(harness.state().quit_confirmed(), "{source:?}");
        frame(&mut harness, false);
        assert!(shown.lock().expect("shown").is_empty(), "{source:?}");
    }
}

/// Editor dirty, designer dirty, streaming: every source holds the window open
/// (`CancelClose`) and shows the prompt; Cancel keeps the app open.
#[test]
fn test_quit_prompts_and_cancel_keeps_app_open() {
    for source in SOURCES {
        for work in WORK.into_iter().filter(|work| !matches!(work, Work::Clean)) {
            let (mut harness, shown) = harness(work, MessageResult::Cancel);
            let commands = quit_from(&mut harness, source);
            assert!(
                commands.contains(&ViewportCommand::CancelClose),
                "{source:?} {work:?}: {commands:?}"
            );
            assert!(
                !commands.contains(&ViewportCommand::Close),
                "{source:?} {work:?}: {commands:?}"
            );
            // The prompt's answer (Cancel) is taken on the next frame.
            let commands = frame(&mut harness, false);
            assert_eq!(
                *shown.lock().expect("shown"),
                [expected_prompt(work)],
                "{source:?} {work:?}"
            );
            assert!(
                !commands.contains(&ViewportCommand::Close),
                "{source:?} {work:?}: {commands:?}"
            );
            assert!(!harness.state().quit_confirmed(), "{source:?} {work:?}");
            // Still open: a later close asks again.
            let commands = frame(&mut harness, true);
            assert!(
                commands.contains(&ViewportCommand::CancelClose),
                "{source:?} {work:?}: {commands:?}"
            );
            assert_eq!(shown.lock().expect("shown").len(), 2, "{source:?} {work:?}");
        }
    }
}

/// A queued quit prompt brings the window back (unminimized, visible, focused) so the user
/// can answer it, also when the window was minimized.
#[test]
fn test_quit_prompt_brings_window_back() {
    for source in SOURCES {
        let (mut harness, _shown) = harness(Work::EditorDirty, MessageResult::Cancel);
        let commands = quit_from(&mut harness, source);
        for expected in [
            ViewportCommand::Minimized(false),
            ViewportCommand::Visible(true),
            ViewportCommand::Focus,
        ] {
            assert!(
                commands.contains(&expected),
                "{source:?}: {expected:?} missing from {commands:?}"
            );
        }
    }
}

/// No dialog can be shown (craft-dialogs answers `Unavailable`): the quit is cancelled, the
/// app stays open and the next close asks again instead of being stuck.
#[test]
fn test_quit_prompt_unavailable_cancels_and_asks_again() {
    let (mut harness, shown) = harness(Work::EditorDirty, MessageResult::Cancel);
    {
        let shown = Arc::clone(&shown);
        harness
            .state_mut()
            .dialogs_mut()
            .set_override(Some(Box::new(move |request: &Request| {
                if let Request::Message(message) = request {
                    shown
                        .lock()
                        .expect("shown lock")
                        .push(message.title.clone());
                }
                Answer::Unavailable("no parent window".into())
            })));
    }
    harness
        .state_mut()
        .state_mut()
        .restart
        .store(true, Ordering::SeqCst);
    quit_from(&mut harness, QuitSource::WindowClose);
    let commands = frame(&mut harness, false);
    assert!(!commands.contains(&ViewportCommand::Close), "{commands:?}");
    assert!(!harness.state().quit_confirmed());
    assert!(!harness.state().state().restart.load(Ordering::SeqCst));
    let commands = frame(&mut harness, true);
    assert!(
        commands.contains(&ViewportCommand::CancelClose),
        "{commands:?}"
    );
    assert_eq!(
        shown.lock().expect("shown").len(),
        2,
        "a new prompt is queued"
    );
}

/// An unsaved-changes prompt that the quit flow did not ask (another feature's) never
/// decides a quit.
#[test]
fn test_unsaved_prompt_outside_quit_does_not_quit() {
    let (mut harness, _shown) = harness(Work::EditorDirty, discard());
    harness
        .state_mut()
        .dialogs_mut()
        .confirm(ConfirmRequest::UnsavedGcode);
    frame(&mut harness, false);
    let commands = frame(&mut harness, false);
    assert!(!commands.contains(&ViewportCommand::Close), "{commands:?}");
    assert!(!harness.state().quit_confirmed());
}

/// Discard (unsaved changes) and Yes (stop job) quit; our own confirmed close does not ask
/// again.
#[test]
fn test_quit_confirmed_prompt_quits_without_asking_again() {
    for (work, reply) in [
        (Work::EditorDirty, discard()),
        (Work::DesignerDirty, discard()),
        (Work::Streaming, MessageResult::Yes),
    ] {
        let (mut harness, shown) = harness(work, reply);
        quit_from(&mut harness, QuitSource::WindowClose);
        let commands = frame(&mut harness, false);
        assert!(
            commands.contains(&ViewportCommand::Close),
            "{work:?}: {commands:?}"
        );
        assert!(harness.state().quit_confirmed(), "{work:?}");
        // The Close comes back as a close request; it goes through.
        let commands = frame(&mut harness, true);
        assert!(
            !commands.contains(&ViewportCommand::CancelClose),
            "{work:?}: {commands:?}"
        );
        assert_eq!(shown.lock().expect("shown").len(), 1, "{work:?}");
    }
}

/// A running job is asked about first, then the unsaved editor, then the unsaved design.
#[test]
fn test_quit_asks_job_first_then_unsaved_work() {
    let (mut harness, shown) = harness(Work::Streaming, MessageResult::Yes);
    harness.state_mut().state_mut().dirty = DirtyState {
        editor: true,
        designer: true,
    };
    // Stop job: Yes (answered at the end of this frame). Discard for the rest.
    quit_from(&mut harness, QuitSource::MenuQuit);
    answer_with(harness.state_mut(), &shown, discard());
    frame(&mut harness, false);
    frame(&mut harness, false);
    let commands = frame(&mut harness, false);
    assert_eq!(
        *shown.lock().expect("shown"),
        ["Stop job and quit?", "Unsaved changes", "Unsaved changes"]
    );
    assert!(commands.contains(&ViewportCommand::Close), "{commands:?}");
}

/// "Save" cannot save yet (no editor or designer in this front end): the app stays open.
#[test]
fn test_quit_save_without_save_path_keeps_app_open() {
    let (mut harness, _shown) = harness(Work::EditorDirty, save());
    quit_from(&mut harness, QuitSource::MenuQuit);
    let commands = frame(&mut harness, false);
    assert!(!commands.contains(&ViewportCommand::Close), "{commands:?}");
    assert!(!harness.state().quit_confirmed());
}

/// Stop job: No keeps the app open.
#[test]
fn test_quit_stop_job_no_keeps_app_open() {
    let (mut harness, _shown) = harness(Work::Streaming, MessageResult::No);
    quit_from(&mut harness, QuitSource::Os);
    let commands = frame(&mut harness, false);
    assert!(!commands.contains(&ViewportCommand::Close), "{commands:?}");
    assert!(!harness.state().quit_confirmed());
}

/// A restart cancelled at the unsaved-changes prompt is forgotten: a later clean quit does
/// not relaunch the app.
#[test]
fn test_cancelled_restart_does_not_relaunch_later_quit() {
    let (mut harness, shown) = harness(Work::EditorDirty, MessageResult::Yes);
    harness.state_mut().request_restart();
    // Restart now: Yes. Then the unsaved editor asks; Cancel.
    frame(&mut harness, false);
    answer_with(harness.state_mut(), &shown, MessageResult::Cancel);
    let commands = frame(&mut harness, false);
    assert!(!commands.contains(&ViewportCommand::Close), "{commands:?}");
    assert!(
        harness.state().state().restart.load(Ordering::SeqCst),
        "restart pending while the prompt is up"
    );
    let commands = frame(&mut harness, false);
    assert_eq!(
        *shown.lock().expect("shown"),
        ["Restart to apply settings?", "Unsaved changes"]
    );
    assert!(!commands.contains(&ViewportCommand::Close), "{commands:?}");
    assert!(
        !harness.state().state().restart.load(Ordering::SeqCst),
        "cancelled restart is cleared"
    );

    // Later the editor is clean and the user quits: no relaunch.
    set_work(harness.state_mut().state_mut(), Work::Clean);
    let commands = quit_from(&mut harness, QuitSource::MenuQuit);
    assert!(commands.contains(&ViewportCommand::Close), "{commands:?}");
    assert!(!harness.state().state().restart.load(Ordering::SeqCst));
}

/// A confirmed restart with nothing unsaved closes the window with the restart flag set.
#[test]
fn test_confirmed_restart_quits_and_relaunches() {
    let (mut harness, _shown) = harness(Work::Clean, MessageResult::Yes);
    harness.state_mut().request_restart();
    frame(&mut harness, false);
    let commands = frame(&mut harness, false);
    assert!(commands.contains(&ViewportCommand::Close), "{commands:?}");
    assert!(harness.state().state().restart.load(Ordering::SeqCst));
}

/// The unsaved-changes prompt's answers map onto the shared choice.
#[test]
fn test_confirm_choice_maps_to_unsaved_choice() {
    assert_eq!(ConfirmChoice::Yes.as_unsaved(), UnsavedChangesChoice::Save);
    assert_eq!(
        ConfirmChoice::No.as_unsaved(),
        UnsavedChangesChoice::Discard
    );
    assert_eq!(
        ConfirmChoice::Cancel.as_unsaved(),
        UnsavedChangesChoice::Cancel
    );
}
