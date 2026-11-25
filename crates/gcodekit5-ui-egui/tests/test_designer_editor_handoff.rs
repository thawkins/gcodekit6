//! The designer's Generate G-Code and Frame hand their program to the editor: it replaces
//! the editor's text and forgets the editor's file (Save then asks where), unsaved editor
//! edits are asked about first (Save / Discard / Cancel), and nothing is generated or
//! replaced while a job runs.

use std::time::Duration;

use craft_dialogs::{Answer, MessageResult};
use gcodekit5_ui_egui::commands::Command;
use gcodekit5_ui_egui::panes::designer::ToEditor;
use gcodekit5_ui_egui::workspace::PaneId;

use crate::test_designer_files::scratch;
use crate::test_designer_files_app::{
    add_rect, answer, app, default_state, discard, frames, save, shown,
};

/// The program `Generate G-Code` makes for the design (the rectangle's far corner).
const GENERATED: &str = "X60.000";

/// Asks the designer for Generate G-Code, as its toolbox button does.
fn generate(h: &mut egui_kittest::Harness<'static, gcodekit5_ui_egui::GcodeKitApp>) {
    h.state()
        .designer()
        .borrow_mut()
        .request_to_editor(ToEditor::Generate);
    frames(h, 3);
}

fn editor_text(h: &egui_kittest::Harness<'static, gcodekit5_ui_egui::GcodeKitApp>) -> String {
    h.state().editor().borrow().buffer().text().to_string()
}

/// The generated program has no file: the editor's next Save asks where instead of
/// overwriting the file the editor had open.
#[test]
fn test_generate_replaces_the_editor_program_and_forgets_its_file() -> std::io::Result<()> {
    let old = scratch("handoff_path")?.join("old.gcode");
    std::fs::write(&old, "G0 X0\n")?;
    let (mut h, script) = app(default_state());
    h.state_mut().open_gcode(&old);
    add_rect(&mut h);
    generate(&mut h);
    assert!(editor_text(&h).contains(GENERATED));
    assert!(h.state().editor().borrow().buffer().path().is_none());
    assert!(
        h.state().state().dirty.editor,
        "the generated program is unsaved"
    );
    // Work in the editor, then Save: it asks where (and cancelling writes nothing).
    let shown_editor = h
        .state_mut()
        .workspace_mut()
        .apply(craft_ui::docking::Action::Activate {
            panel: PaneId::Editor,
        });
    assert!(shown_editor.is_ok(), "{shown_editor:?}");
    frames(&mut h, 2);
    answer(&script, [Answer::Cancelled]);
    h.state_mut().dispatch(Command::FileSave);
    frames(&mut h, 3);
    assert_eq!(shown(&script), ["Save G-Code File"]);
    assert_eq!(std::fs::read_to_string(&old)?, "G0 X0\n");
    Ok(())
}

#[test]
fn test_frame_replaces_the_editor_program() {
    let (mut h, _script) = app(default_state());
    add_rect(&mut h);
    h.state()
        .designer()
        .borrow_mut()
        .request_to_editor(ToEditor::Frame);
    frames(&mut h, 3);
    assert!(editor_text(&h).contains("Laser Frame Start"));
}

/// Unsaved editor edits: Cancel keeps them and delivers nothing; Discard delivers.
#[test]
fn test_generate_over_unsaved_edits_asks_cancel_and_discard() {
    let (mut h, script) = app(default_state());
    add_rect(&mut h);
    h.state()
        .editor()
        .borrow_mut()
        .buffer_mut()
        .set_text("G1 X1\n".to_string());
    answer(&script, [Answer::Message(MessageResult::Cancel)]);
    generate(&mut h);
    assert_eq!(shown(&script), ["Unsaved changes"]);
    assert_eq!(editor_text(&h), "G1 X1\n");
    // A later frame does not deliver the cancelled program either.
    frames(&mut h, 2);
    assert_eq!(editor_text(&h), "G1 X1\n");

    answer(&script, [discard()]);
    generate(&mut h);
    assert_eq!(shown(&script).len(), 2);
    assert!(editor_text(&h).contains(GENERATED));
}

/// Save writes the edits to the editor's file, then delivers.
#[test]
fn test_generate_over_unsaved_edits_save_first() -> std::io::Result<()> {
    let file = scratch("handoff_save")?.join("kept.gcode");
    std::fs::write(&file, "G0 X0\n")?;
    let (mut h, script) = app(default_state());
    h.state_mut().open_gcode(&file);
    h.state()
        .editor()
        .borrow_mut()
        .buffer_mut()
        .set_text("G1 X5\n".to_string());
    add_rect(&mut h);
    answer(&script, [save()]);
    generate(&mut h);
    assert_eq!(shown(&script), ["Unsaved changes"]);
    assert_eq!(std::fs::read_to_string(&file)?, "G1 X5\n");
    assert!(editor_text(&h).contains(GENERATED));
    Ok(())
}

/// Save with no file asks where; cancelling that delivers nothing.
#[test]
fn test_generate_over_unsaved_edits_save_cancelled_delivers_nothing() {
    let (mut h, script) = app(default_state());
    h.state()
        .editor()
        .borrow_mut()
        .buffer_mut()
        .set_text("G1 X5\n".to_string());
    add_rect(&mut h);
    answer(&script, [save(), Answer::Cancelled]);
    generate(&mut h);
    assert_eq!(shown(&script), ["Unsaved changes", "Save G-Code File"]);
    assert_eq!(editor_text(&h), "G1 X5\n");
}

/// A prompt that cannot be shown delivers nothing.
#[test]
fn test_generate_with_unavailable_prompt_delivers_nothing() {
    let (mut h, script) = app(default_state());
    h.state()
        .editor()
        .borrow_mut()
        .buffer_mut()
        .set_text("G1 X5\n".to_string());
    add_rect(&mut h);
    answer(&script, [Answer::Unavailable("no display".into())]);
    generate(&mut h);
    frames(&mut h, 2);
    assert_eq!(editor_text(&h), "G1 X5\n");
}

/// Carry: like File > Open, sending G-code to the editor while a job streams is refused
/// (it would replace the program the job is drawing), before anything is generated.
#[test]
fn test_generate_is_refused_while_a_job_runs() {
    let (mut state, shared) = crate::machine_support::loopback_state(Duration::from_millis(1));
    crate::machine_support::start_held_stream(&mut state, &shared, 100);
    let (mut h, _script) = app(state);
    assert!(h.state().state().job_running());
    let before = editor_text(&h);
    add_rect(&mut h);
    generate(&mut h);
    assert_eq!(editor_text(&h), before);
    let mut designer = h.state().designer().borrow_mut();
    assert_eq!(
        designer.notice(),
        Some("Stop the running job before sending G-code to the editor.")
    );
    assert_eq!(designer.take_editor_gcode(), None, "nothing was generated");
}
