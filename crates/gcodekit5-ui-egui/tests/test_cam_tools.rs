//! The CAM Tools pane: the picker, and the handoff of a page's program to the editor
//! (Send to Editor replaces the editor's program, Preview also brings the visualizer to the
//! front, unsaved editor edits are asked about first, and nothing is handed over while a
//! job runs).

use std::time::Duration;

use craft_dialogs::{Answer, MessageResult};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::panes::cam_tools::CamDelivery;
use gcodekit5_ui_egui::workspace::PaneId;

use crate::test_designer_files_app::{answer, app, default_state, discard, frames, shown};

/// The program handed over in these tests.
const PROGRAM: &str = "G0 X1\nG1 X2 F100\n";

/// The app with the CAM Tools pane in front.
pub fn cam_app(
    state: gcodekit5_ui_egui::AppState,
) -> (
    Harness<'static, GcodeKitApp>,
    crate::test_designer_files_app::Shared,
) {
    let (mut h, script) = app(state);
    let shown = h
        .state_mut()
        .workspace_mut()
        .apply(craft_ui::docking::Action::Activate {
            panel: PaneId::CamTools,
        });
    assert!(shown.is_ok(), "{shown:?}");
    frames(&mut h, 2);
    (h, script)
}

/// Asks the CAM pane to hand [`PROGRAM`] over, as a page's button does.
fn request(h: &mut Harness<'static, GcodeKitApp>, delivery: CamDelivery) {
    h.state()
        .cam_tools()
        .borrow_mut()
        .request_to_editor(PROGRAM.to_string(), delivery);
    frames(h, 3);
}

fn editor_text(h: &Harness<'static, GcodeKitApp>) -> String {
    h.state().editor().borrow().buffer().text().to_string()
}

#[test]
fn test_cam_pane_replaces_the_placeholder_and_shows_the_picker() {
    let (h, _script) = cam_app(default_state());
    assert!(h.state().workspace().is_drawn(PaneId::CamTools));
    assert!(h.query_by_label("Select a tool").is_some());
    assert!(
        h.query_by_label("Choose a tool from the list to see details.")
            .is_some()
    );
    assert_eq!(h.state().cam_tools().borrow().open_tool(), None);
}

#[test]
fn test_send_to_editor_replaces_the_program_and_forgets_the_file() {
    let (mut h, _script) = cam_app(default_state());
    request(&mut h, CamDelivery::Editor);
    assert_eq!(editor_text(&h), PROGRAM);
    assert!(h.state().editor().borrow().buffer().path().is_none());
    assert!(h.state().state().dirty.editor, "the program is unsaved");
    assert!(
        h.state().workspace().is_drawn(PaneId::CamTools),
        "the pane stays in front"
    );
}

#[test]
fn test_preview_brings_the_visualizer_to_the_front() {
    let (mut h, _script) = cam_app(default_state());
    request(&mut h, CamDelivery::Preview);
    assert_eq!(editor_text(&h), PROGRAM);
    assert!(h.state().workspace().is_drawn(PaneId::Visualizer));
    assert!(!h.state().workspace().is_drawn(PaneId::CamTools));
}

/// Unsaved editor edits: Cancel keeps them, Discard delivers.
#[test]
fn test_send_over_unsaved_edits_asks_first() {
    let (mut h, script) = cam_app(default_state());
    h.state()
        .editor()
        .borrow_mut()
        .buffer_mut()
        .set_text("G1 X9\n".to_string());
    answer(&script, [Answer::Message(MessageResult::Cancel)]);
    request(&mut h, CamDelivery::Editor);
    assert_eq!(shown(&script), ["Unsaved changes"]);
    assert_eq!(editor_text(&h), "G1 X9\n");

    answer(&script, [discard()]);
    request(&mut h, CamDelivery::Editor);
    assert_eq!(shown(&script).len(), 2);
    assert_eq!(editor_text(&h), PROGRAM);
}

/// While a job streams nothing is handed over, and the CAM pane says why.
#[test]
fn test_send_is_refused_while_a_job_runs() {
    let (mut state, shared) = crate::machine_support::loopback_state(Duration::from_millis(1));
    crate::machine_support::start_held_stream(&mut state, &shared, 100);
    let (mut h, _script) = cam_app(state);
    assert!(h.state().state().job_running());
    let before = editor_text(&h);
    request(&mut h, CamDelivery::Preview);
    assert_eq!(editor_text(&h), before);
    let mut cam = h.state().cam_tools().borrow_mut();
    assert_eq!(
        cam.notice(),
        Some("Stop the running job before sending G-code to the editor.")
    );
    assert_eq!(cam.take_editor_gcode(), None, "the program was dropped");
}

/// The picker filters by category and by the search text (title or description).
#[test]
fn test_cam_picker_filters_by_category_and_search() {
    use gcodekit5_app::i18n::Translator;
    use gcodekit5_ui_egui::panes::cam_tools::{CamTool, CamToolsPane, Category};
    let tr = Translator::new("en");
    let mut pane = CamToolsPane::new();
    pane.set_category(Category::Maintenance);
    let listed = pane.listed_tools(&tr);
    assert!(listed.contains(&CamTool::SpoilboardSurfacing));
    assert!(!listed.contains(&CamTool::DrillPress));
    pane.set_category(Category::All);
    pane.set_search("PECK");
    assert_eq!(pane.listed_tools(&tr), [CamTool::DrillPress]);
}

/// One handoff at a time: a request made while another waits for the unsaved-changes answer
/// is dropped with a notice, and its program can never be delivered later.
#[test]
fn test_second_handoff_while_one_waits_is_dropped_with_a_notice() {
    use gcodekit5_ui_egui::panes::designer::ToEditor;
    let (mut h, script) = cam_app(default_state());
    crate::test_designer_files_app::add_rect(&mut h);
    h.state()
        .editor()
        .borrow_mut()
        .buffer_mut()
        .set_text("G1 X9\n".to_string());
    // The designer asks first (it is delivered first in the frame); the CAM request waits
    // behind its prompt and is dropped.
    h.state()
        .designer()
        .borrow_mut()
        .request_to_editor(ToEditor::Generate);
    answer(&script, [discard()]);
    request(&mut h, CamDelivery::Editor);
    assert_eq!(shown(&script), ["Unsaved changes"]);
    let mut cam = h.state().cam_tools().borrow_mut();
    assert_eq!(cam.notice(), Some(DROPPED));
    assert_eq!(cam.take_editor_gcode(), None, "the dropped program is gone");
    drop(cam);
    let text = editor_text(&h);
    assert!(
        !text.contains("G1 X2 F100"),
        "the dropped program reached the editor"
    );
}

/// The notice shown when a handoff is dropped.
const DROPPED: &str = "Another program is waiting to go to the editor; this one was not sent.";

/// Machine events that reach the pane while it is borrowed wait for the next call.
#[test]
fn test_cam_pane_keeps_machine_events_while_busy() {
    use gcodekit5_app::machine::MachineEvent;
    use gcodekit5_ui_egui::panes::cam_tools::SharedCamTools;
    use gcodekit5_ui_egui::workspace::Pane;
    let (h, _script) = cam_app(default_state());
    let tr = h.state().state().translator;
    let rc = std::rc::Rc::clone(h.state().cam_tools());
    let mut shared = SharedCamTools::new(std::rc::Rc::clone(&rc));
    {
        let _busy = rc.borrow_mut();
        shared.machine_events(&[MachineEvent::Connected], &tr);
    }
    assert_eq!(shared.waiting_events(), 1);
    shared.machine_events(&[], &tr);
    assert_eq!(shared.waiting_events(), 0);
}
