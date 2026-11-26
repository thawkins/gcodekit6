//! The rows the GTK Preferences notebook shows on each page: `ui/gtk/settings.rs` fills a page
//! with `SettingsController::get_settings_for_ui(Some(category))`, so a page shows exactly the
//! settings `SettingsPersistence::populate_dialog` puts in its category.

use gcodekit5_settings::controller::SettingsController;
use gcodekit5_settings::{SettingsCategory, SettingsDialog, SettingsPersistence};
use std::cell::RefCell;
use std::rc::Rc;

/// The GTK controller over the default settings, as the GTK window builds it.
fn gtk_controller() -> SettingsController {
    let persistence = SettingsPersistence::new();
    let mut dialog = SettingsDialog::new();
    persistence.populate_dialog(&mut dialog);
    SettingsController::new(
        Rc::new(RefCell::new(dialog)),
        Rc::new(RefCell::new(persistence)),
    )
}

/// GTK's "Controller" page has no rows: no setting is in that category (the connection rows
/// moved to the Device Manager), so the egui Preferences window needs no Controller page.
#[test]
fn test_gtk_controller_page_has_no_rows() {
    let controller = gtk_controller();
    let rows = controller.get_settings_for_ui(Some(SettingsCategory::Controller));
    assert!(
        rows.is_empty(),
        "{:?}",
        rows.iter().map(|r| &r.id).collect::<Vec<_>>()
    );
}

/// GTK's "Shortcuts" page has no rows either: the shortcuts `populate_dialog` lists are never
/// shown or saved, so GTK users have no shortcut customisations to carry over.
#[test]
fn test_gtk_shortcuts_page_has_no_rows() {
    let controller = gtk_controller();
    let rows = controller.get_settings_for_ui(Some(SettingsCategory::KeyboardShortcuts));
    assert!(
        rows.is_empty(),
        "{:?}",
        rows.iter().map(|r| &r.id).collect::<Vec<_>>()
    );
    // The other pages do have rows, so the filter works.
    for category in [
        SettingsCategory::General,
        SettingsCategory::Machine,
        SettingsCategory::UserInterface,
        SettingsCategory::FileProcessing,
        SettingsCategory::Advanced,
    ] {
        assert!(
            !controller
                .get_settings_for_ui(Some(category.clone()))
                .is_empty(),
            "{category} has no rows"
        );
    }
}
