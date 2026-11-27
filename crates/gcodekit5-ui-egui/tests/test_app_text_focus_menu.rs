//! Edit > Copy chosen from the in-window menu while a text field had the keyboard acts on
//! that field (the app's `TextFocus`): the field gets the focus back and the command is not
//! dispatched to the panes. With no text field it is dispatched as usual.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::commands::Command;

use crate::test_machine_pane::{entry, focus_entry, pane_app};

/// Clicks Edit > Copy on the in-window menu bar.
fn copy_from_menu(h: &mut Harness<'_, GcodeKitApp>) {
    h.get_by_role_and_label(Role::MenuItem, "Edit").click();
    h.run();
    h.get_by_role_and_label(Role::MenuItem, "Copy").click();
    h.run();
}

#[test]
fn test_app_text_focus_menu_copy_returns_to_the_field() {
    let (mut h, _shared) = pane_app(std::time::Duration::from_millis(1), true);
    focus_entry(&mut h);
    copy_from_menu(&mut h);
    assert!(entry(&h).is_focused(), "the entry gets the focus back");
    assert_ne!(h.state().last_dispatched(), Some(Command::Copy));
}

#[test]
fn test_app_text_focus_menu_copy_dispatches_without_a_field() {
    let (mut h, _shared) = pane_app(std::time::Duration::from_millis(1), true);
    h.run();
    copy_from_menu(&mut h);
    assert_eq!(h.state().last_dispatched(), Some(Command::Copy));
}
