//! The designer in the app: Undo/Redo reach it when it is the pane the user works in, its
//! modified flag reaches the quit prompt, and its keys stay with a focused text field.

use egui::{Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_designer::model::{DesignRectangle, Shape};
use gcodekit5_ui_egui::commands::Command;
use gcodekit5_ui_egui::workspace::PaneId;
use glam::Vec2;

use crate::test_designer_canvas::selected;

/// The app's designer, with the Designer tab shown.
pub fn app_with_designer() -> Harness<'static, gcodekit5_ui_egui::GcodeKitApp> {
    use gcodekit5_settings::SettingsManager;
    use gcodekit5_settings::config::Config;
    use gcodekit5_ui_egui::{AppState, GcodeKitApp};
    let app = GcodeKitApp::from_state(AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    ));
    let mut h = crate::machine_support::app_harness(app);
    h.state_mut().workspace_mut().show_pane(PaneId::Designer);
    h.run();
    h
}

/// Clicks `pos` in the app with the real pointer events.
pub fn app_click(h: &mut Harness<'_, gcodekit5_ui_egui::GcodeKitApp>, pos: Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    h.step();
    h.event(egui::Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.step();
    h.event(egui::Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.step();
    h.run();
}

#[test]
fn test_app_routes_undo_to_active_designer_and_tracks_dirty() {
    let mut h = app_with_designer();
    let designer = std::rc::Rc::clone(h.state().designer());
    designer
        .borrow_mut()
        .state_mut()
        .add_shape_with_undo(Shape::Rectangle(DesignRectangle::new(
            20.0, 20.0, 40.0, 40.0,
        )));
    h.run();
    assert!(
        h.state().state().dirty.designer,
        "the design's changes reach the quit prompt"
    );
    // Undo goes to the designer only once the user worked in it.
    h.state_mut().dispatch(Command::Undo);
    h.run();
    assert_eq!(designer.borrow().state().canvas.shape_count(), 1);
    let p = designer
        .borrow()
        .view()
        .world_to_screen(Vec2::new(40.0, 40.0));
    app_click(&mut h, p);
    assert_eq!(selected(&designer.borrow()).len(), 1);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Z);
    h.run();
    assert_eq!(designer.borrow().state().canvas.shape_count(), 0);
    h.state_mut().dispatch(Command::Redo);
    h.run();
    assert_eq!(designer.borrow().state().canvas.shape_count(), 1);
}

#[test]
fn test_canvas_keys_do_not_fire_while_typing() {
    let mut h = app_with_designer();
    let designer = std::rc::Rc::clone(h.state().designer());
    designer
        .borrow_mut()
        .state_mut()
        .add_shape_with_undo(Shape::Rectangle(DesignRectangle::new(
            20.0, 20.0, 40.0, 40.0,
        )));
    h.run();
    let p = designer
        .borrow()
        .view()
        .world_to_screen(Vec2::new(40.0, 40.0));
    app_click(&mut h, p);
    assert_eq!(selected(&designer.borrow()).len(), 1);
    // Give a text field (the inspector's X) the keyboard: Delete, Cmd+Z and Escape belong
    // to it now. Each key is its own frame, as typed: Escape ends the field's focus at the
    // start of its frame, so a Cmd+Z in that same frame would no longer be the field's.
    let field = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "X:");
    field.focus();
    h.run();
    h.key_press(egui::Key::Delete);
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Z);
    h.run();
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(designer.borrow().state().canvas.shape_count(), 1);
    assert_eq!(selected(&designer.borrow()).len(), 1);
}

/// GTK's empty state: with no shapes the canvas shows "No shapes yet" and the open and
/// import buttons; Load Design opens a design, each Import opens the import dialog, Import
/// STL shows only with STL import on, Import Image is disabled in CNC mode, and the empty
/// state goes once a shape is drawn.
#[test]
fn test_empty_design_shows_gtks_open_and_import_buttons() {
    let mut h = app_with_designer();
    let _ = h.get_by_label("No shapes yet");
    assert!(
        h.query_by_label("Import STL").is_none(),
        "STL import is off"
    );
    for (label, command) in [
        ("Load Design", Command::FileOpen),
        ("Import SVG", Command::FileImport),
        ("Import DXF", Command::FileImport),
        ("Import Image", Command::FileImportImage),
    ] {
        h.get_by_label(label).click();
        h.run();
        assert_eq!(h.state().last_dispatched(), Some(command), "{label}");
    }
    let mut config = h.state().state().config().clone();
    config.ui.enable_stl_import = true;
    h.state_mut().workspace_mut().configure(&config);
    h.run();
    h.get_by_label("Import STL").click();
    h.run();
    assert_eq!(h.state().last_dispatched(), Some(Command::FileImport));
    let designer = std::rc::Rc::clone(h.state().designer());
    designer
        .borrow_mut()
        .state_mut()
        .set_machine_mode(gcodekit5_designer::designer_state::MachineMode::Cnc3D);
    h.run();
    {
        use egui_kittest::kittest::NodeT;
        assert!(
            h.get_by_label("Import Image")
                .accesskit_node()
                .is_disabled()
        );
    }
    designer
        .borrow_mut()
        .state_mut()
        .add_shape_with_undo(Shape::Rectangle(DesignRectangle::new(0.0, 0.0, 10.0, 10.0)));
    h.run();
    assert!(h.query_by_label("No shapes yet").is_none());
}
