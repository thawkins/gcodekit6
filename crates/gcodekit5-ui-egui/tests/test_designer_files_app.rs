//! Designer file commands in the app: File > Open / Save / Save As reach the design while a
//! designer pane is the active one, New asks about unsaved changes first, Save from the quit
//! prompt saves the design before quitting (after the editor's prompt; a failed save cancels
//! the quit), and Import and Export go through their dialogs. The editor hand-off (Generate
//! G-Code, Frame) is in `test_designer_editor_handoff.rs`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use craft_dialogs::{Answer, MessageResult, Request};
use egui::{ViewportCommand, ViewportId};
use egui_kittest::Harness;
use gcodekit5_designer::designer_state::MachineMode;
use gcodekit5_designer::model::{DesignRectangle, Shape};
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::app::QuitSource;
use gcodekit5_ui_egui::commands::Command;
use gcodekit5_ui_egui::workspace::PaneId;
use gcodekit5_ui_egui::{AppState, GcodeKitApp};

use crate::test_designer_files::{fixture, scratch};

/// The dialogs shown (titles) and the answers still to give, in order.
#[derive(Default)]
pub struct Script {
    shown: Vec<String>,
    /// The message boxes' texts, in order.
    details: Vec<String>,
    answers: VecDeque<Answer>,
}

pub type Shared = Arc<Mutex<Script>>;

/// The app with the Designer active, its dialogs answered from a script.
pub fn app(state: AppState) -> (Harness<'static, GcodeKitApp>, Shared) {
    let mut app = GcodeKitApp::from_state(state);
    let script: Shared = Arc::default();
    let s = Arc::clone(&script);
    app.dialogs_mut()
        .set_override(Some(Box::new(move |request: &Request| {
            let mut s = s.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let title = match request {
                Request::Message(m) => {
                    s.details.push(m.description.clone());
                    m.title.clone()
                }
                Request::File(f) => f.title.clone().unwrap_or_default(),
            };
            s.shown.push(title);
            s.answers.pop_front().unwrap_or(Answer::Cancelled)
        })));
    let mut h = crate::machine_support::app_harness(app);
    h.state_mut().workspace_mut().show_pane(PaneId::Designer);
    h.run();
    // A press in the designer makes it the active pane, as the user's work would.
    let p = h
        .state()
        .designer()
        .borrow()
        .view()
        .world_to_screen(glam::Vec2::new(200.0, 200.0));
    crate::test_designer_canvas::click(&mut h, p);
    (h, script)
}

pub fn default_state() -> AppState {
    AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    )
}

/// Queues the next dialog answers.
pub fn answer(script: &Shared, answers: impl IntoIterator<Item = Answer>) {
    if let Ok(mut s) = script.lock() {
        s.answers.extend(answers);
    }
}

pub fn shown(script: &Shared) -> Vec<String> {
    script.lock().map(|s| s.shown.clone()).unwrap_or_default()
}

/// The texts of the message boxes shown so far.
pub fn details(script: &Shared) -> Vec<String> {
    script.lock().map(|s| s.details.clone()).unwrap_or_default()
}

/// One frame as eframe runs it (logic, ui, then the dialogs); returns the root viewport's
/// commands.
pub fn frame(h: &mut Harness<'_, GcodeKitApp>) -> Vec<ViewportCommand> {
    h.step();
    let commands = h
        .output()
        .viewport_output
        .get(&ViewportId::ROOT)
        .map(|o| o.commands.clone())
        .unwrap_or_default();
    h.state_mut()
        .dialogs_mut()
        .end_frame(None::<&eframe::Frame>);
    commands
}

/// Runs a few frames, collecting their commands.
pub fn frames(h: &mut Harness<'_, GcodeKitApp>, n: usize) -> Vec<ViewportCommand> {
    (0..n).flat_map(|_| frame(h)).collect()
}

pub fn add_rect(h: &mut Harness<'_, GcodeKitApp>) {
    h.state()
        .designer()
        .borrow_mut()
        .state_mut()
        .add_shape_with_undo(Shape::Rectangle(DesignRectangle::new(
            20.0, 20.0, 40.0, 30.0,
        )));
    frame(h);
}

pub fn save() -> Answer {
    Answer::Message(MessageResult::Custom("Save".into()))
}
pub fn discard() -> Answer {
    Answer::Message(MessageResult::Custom("Discard".into()))
}
pub fn path(p: &std::path::Path) -> Answer {
    Answer::Paths(vec![p.to_path_buf()])
}

/// Carry: a save clears both the model's flag and the app's dirty cache; a later edit
/// makes it dirty again.
#[test]
fn test_save_as_writes_the_design_and_clears_dirty() -> std::io::Result<()> {
    let target = scratch("app_save")?.join("part");
    let (mut h, script) = app(default_state());
    add_rect(&mut h);
    assert!(h.state().state().dirty.designer);
    answer(&script, [path(&target)]);
    h.state_mut().dispatch(Command::FileSave);
    frames(&mut h, 3);
    assert_eq!(shown(&script), ["Save Design File"]);
    let saved = target.with_extension("gckd");
    assert!(saved.exists(), "the extension is added");
    assert!(!h.state().state().dirty.designer);
    assert!(!h.state().designer().borrow().is_modified());
    // Save again: the file is known, no dialog.
    add_rect(&mut h);
    assert!(
        h.state().state().dirty.designer,
        "an edit makes it dirty again"
    );
    h.state_mut().dispatch(Command::FileSave);
    frames(&mut h, 2);
    assert_eq!(shown(&script).len(), 1);
    assert!(!h.state().state().dirty.designer);
    Ok(())
}

#[test]
fn test_open_routes_to_the_designer_while_it_is_active() -> std::io::Result<()> {
    let file = scratch("app_open")?.join("in.gckd");
    {
        let mut pane = crate::test_designer_canvas::pane_with_rects(&[(1.0, 1.0, 5.0, 5.0)]);
        assert!(pane.save_design(&file, &crate::test_designer_files::en()));
    }
    let (mut h, script) = app(default_state());
    answer(&script, [path(&file)]);
    h.state_mut().dispatch(Command::FileOpen);
    frames(&mut h, 3);
    assert_eq!(shown(&script), ["Open Design File"]);
    assert_eq!(
        h.state().designer().borrow().state().canvas.shape_count(),
        1
    );
    assert!(h.state().editor().borrow().buffer().path().is_none());
    Ok(())
}

#[test]
fn test_new_with_unsaved_changes_asks_first() {
    let (mut h, script) = app(default_state());
    add_rect(&mut h);
    // Cancel: nothing changes.
    answer(&script, [Answer::Message(MessageResult::Cancel)]);
    h.state_mut().dispatch(Command::FileNew3d);
    frames(&mut h, 3);
    assert_eq!(shown(&script), ["Unsaved changes"]);
    let designer = std::rc::Rc::clone(h.state().designer());
    assert_eq!(designer.borrow().state().canvas.shape_count(), 1);
    // Discard: a new 3D design.
    answer(&script, [discard()]);
    h.state_mut().dispatch(Command::FileNew3d);
    frames(&mut h, 3);
    assert_eq!(designer.borrow().state().canvas.shape_count(), 0);
    assert_eq!(designer.borrow().state().machine_mode(), MachineMode::Cnc3D);
    assert!(!h.state().state().dirty.designer);
    // Clean: no question.
    h.state_mut().dispatch(Command::FileNew);
    frames(&mut h, 3);
    assert_eq!(shown(&script).len(), 2);
    assert_eq!(
        designer.borrow().state().machine_mode(),
        MachineMode::Laser2D
    );
}

#[test]
fn test_new_with_save_saves_first() -> std::io::Result<()> {
    let target = scratch("app_new_save")?.join("kept.gckd");
    let (mut h, script) = app(default_state());
    add_rect(&mut h);
    answer(&script, [save(), path(&target)]);
    h.state_mut().dispatch(Command::FileNew);
    frames(&mut h, 4);
    assert_eq!(shown(&script), ["Unsaved changes", "Save Design File"]);
    assert!(target.exists());
    assert_eq!(
        h.state().designer().borrow().state().canvas.shape_count(),
        0
    );
    Ok(())
}

/// Save from the quit prompt saves the design (asking where), then quits; cancelling the
/// save dialog keeps the app open.
#[test]
fn test_quit_save_saves_the_design_then_quits() -> std::io::Result<()> {
    let target = scratch("app_quit_save")?.join("quit.gckd");
    let (mut h, script) = app(default_state());
    add_rect(&mut h);
    answer(&script, [save(), Answer::Cancelled]);
    h.state_mut().request_quit(QuitSource::MenuQuit);
    let commands = frames(&mut h, 4);
    assert_eq!(shown(&script), ["Unsaved changes", "Save Design File"]);
    assert!(!commands.contains(&ViewportCommand::Close), "{commands:?}");
    assert!(!h.state().quit_confirmed());

    answer(&script, [save(), path(&target)]);
    h.state_mut().request_quit(QuitSource::MenuQuit);
    let commands = frames(&mut h, 4);
    assert!(target.exists());
    assert!(commands.contains(&ViewportCommand::Close), "{commands:?}");
    assert!(h.state().quit_confirmed());
    Ok(())
}

#[test]
fn test_quit_save_to_known_file_does_not_ask_where() -> std::io::Result<()> {
    let target = scratch("app_quit_known")?.join("known.gckd");
    let (mut h, script) = app(default_state());
    add_rect(&mut h);
    let tr = crate::test_designer_files::en();
    assert!(h.state().designer().borrow_mut().save_design(&target, &tr));
    add_rect(&mut h);
    answer(&script, [save()]);
    h.state_mut().request_quit(QuitSource::WindowClose);
    let commands = frames(&mut h, 3);
    assert_eq!(shown(&script), ["Unsaved changes"]);
    assert!(commands.contains(&ViewportCommand::Close), "{commands:?}");
    let mut reopened = gcodekit5_ui_egui::panes::designer::DesignerPane::new();
    assert!(reopened.open_design(&target, &tr));
    assert_eq!(
        reopened.state().canvas.shape_count(),
        2,
        "the second edit was saved"
    );
    Ok(())
}

#[test]
fn test_import_and_export_go_through_their_dialogs() -> std::io::Result<()> {
    let dir = scratch("app_import_export")?;
    let (mut h, script) = app(default_state());
    answer(
        &script,
        [
            path(&fixture("shapes.svg")),
            path(&dir.join("out")),
            path(&dir.join("out.svg")),
        ],
    );
    h.state_mut().dispatch(Command::FileImport);
    frames(&mut h, 3);
    assert_eq!(
        h.state().designer().borrow().state().canvas.shape_count(),
        2
    );
    h.state_mut().dispatch(Command::FileExportGcode);
    frames(&mut h, 3);
    h.state_mut().dispatch(Command::FileExportSvg);
    frames(&mut h, 3);
    assert_eq!(
        shown(&script),
        ["Import Design File", "Export G-Code", "Export SVG"]
    );
    assert!(dir.join("out.nc").exists(), "the .nc extension is added");
    assert!(dir.join("out.svg").exists());
    Ok(())
}

#[test]
fn test_export_gcode_with_every_shape_hidden_asks_nothing() {
    let (mut h, script) = app(default_state());
    add_rect(&mut h);
    let id = h
        .state()
        .designer()
        .borrow()
        .state()
        .canvas
        .shapes()
        .map(|o| o.id)
        .next()
        .expect("shape");
    h.state().designer().borrow_mut().set_hidden(id, true);
    h.state_mut().dispatch(Command::FileExportGcode);
    frames(&mut h, 2);
    assert!(shown(&script).is_empty());
    assert_eq!(
        h.state().designer().borrow().notice(),
        Some("Nothing to export: all shapes are hidden")
    );
}

/// With both documents dirty, the quit asks about the editor first, then the design.
#[test]
fn test_quit_asks_about_the_editor_then_the_design() {
    let (mut h, script) = app(default_state());
    h.state()
        .editor()
        .borrow_mut()
        .buffer_mut()
        .set_text("G1 X1\n".to_string());
    add_rect(&mut h);
    answer(&script, [discard(), discard()]);
    h.state_mut().request_quit(QuitSource::MenuQuit);
    let commands = frames(&mut h, 4);
    assert_eq!(shown(&script), ["Unsaved changes", "Unsaved changes"]);
    let details = details(&script);
    assert!(details[0].contains("G-code in the editor"), "{details:?}");
    assert!(details[1].contains("current design"), "{details:?}");
    assert!(commands.contains(&ViewportCommand::Close), "{commands:?}");
}

/// A design save that fails during the quit (a read-only file) cancels the quit.
#[test]
fn test_quit_save_that_fails_cancels_the_quit() -> std::io::Result<()> {
    let target = scratch("app_quit_fail")?.join("locked.gckd");
    let (mut h, script) = app(default_state());
    add_rect(&mut h);
    let tr = crate::test_designer_files::en();
    assert!(h.state().designer().borrow_mut().save_design(&target, &tr));
    let mut perms = std::fs::metadata(&target)?.permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&target, perms)?;
    add_rect(&mut h);
    answer(&script, [save()]);
    h.state_mut().request_quit(QuitSource::MenuQuit);
    let commands = frames(&mut h, 3);
    assert!(!commands.contains(&ViewportCommand::Close), "{commands:?}");
    assert!(!h.state().quit_confirmed());
    assert!(h.state().state().dirty.designer);
    assert!(
        h.state()
            .designer()
            .borrow()
            .notice()
            .is_some_and(|n| n.starts_with("Save Failed"))
    );
    let mut perms = std::fs::metadata(&target)?.permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&target, perms)?;
    Ok(())
}
