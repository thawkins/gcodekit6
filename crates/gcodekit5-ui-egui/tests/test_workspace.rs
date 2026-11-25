//! Docking workspace: every pane is reachable by its tab, and the layout round-trips.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_app::i18n::Translator;
use gcodekit5_ui_egui::workspace::{PaneContext, PaneId, Workspace};

/// Draws the default workspace at `size` and returns the panes without a tab.
fn panes_without_tab(size: egui::Vec2) -> Vec<PaneId> {
    let tr = Translator::new("en");
    let mut workspace = Workspace::default_layout();
    let mut harness = Harness::builder().with_size(size).build_ui(|ui| {
        let mut queue = Vec::new();
        let mut ctx = PaneContext::new(&tr, &mut queue);
        workspace.show(ui, &mut ctx);
    });
    harness.run();
    PaneId::ALL
        .into_iter()
        .filter(|id| {
            let label = tr.tr(id.title_msgid());
            harness.query_by_role_and_label(Role::Tab, &label).is_none()
        })
        .collect()
}

#[test]
fn test_default_layout_has_all_panes() {
    assert_eq!(panes_without_tab(egui::vec2(1280.0, 800.0)), Vec::new());
}

/// At the window's minimum size (640x400) every pane still has its own tab.
#[test]
fn test_default_layout_survives_small_window() {
    assert_eq!(panes_without_tab(egui::vec2(640.0, 400.0)), Vec::new());
}

/// Below the minimum size, a pane whose tab no longer fits is listed in its group's
/// overflow ("Panels") menu, so it stays reachable.
#[test]
fn test_tiny_window_keeps_panes_in_overflow_menu() {
    let tr = Translator::new("en");
    let mut workspace = Workspace::default_layout();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(320.0, 200.0))
        .build_ui(|ui| {
            let mut queue = Vec::new();
            let mut ctx = PaneContext::new(&tr, &mut queue);
            workspace.show(ui, &mut ctx);
        });
    harness.run();
    let overflowed: Vec<PaneId> = PaneId::ALL
        .into_iter()
        .filter(|id| {
            harness
                .query_by_role_and_label(Role::Tab, &tr.tr(id.title_msgid()))
                .is_none()
        })
        .collect();
    assert!(
        !overflowed.is_empty(),
        "the tiny window should overflow a tab group"
    );
    harness
        .get_by_role_and_label(Role::Button, "Panels")
        .click();
    harness.run();
    for id in overflowed {
        let label = tr.tr(id.title_msgid());
        assert!(
            harness
                .query_by_role_and_label(Role::Button, &label)
                .is_some(),
            "{id:?} is neither a tab nor in the overflow menu"
        );
    }
}

#[test]
fn test_layout_persists() {
    let workspace = Workspace::default_layout();
    let json = serde_json::to_string(&workspace).expect("serialize workspace");
    let restored: Workspace = serde_json::from_str(&json).expect("deserialize workspace");
    assert_eq!(restored, workspace);
}

#[test]
fn test_pane_titles_are_translated() {
    let tr = Translator::new("de");
    let title = tr.tr(PaneId::MachineControl.title_msgid());
    assert_ne!(title, "Machine Control");
}
