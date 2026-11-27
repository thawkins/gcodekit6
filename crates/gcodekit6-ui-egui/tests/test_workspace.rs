//! Page shell: GTK's eight page tabs in GTK's order, each page reachable by its tab, the
//! panes keep their state across page switches, and a docking layout saved by an earlier
//! egui build does not stop the app from starting.

use std::path::Path;

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use gcodekit6_app::i18n::Translator;
use gcodekit6_settings::SettingsManager;
use gcodekit6_settings::config::Config;
use gcodekit6_ui_egui::workspace::{Page, PaneId};
use gcodekit6_ui_egui::{AppState, GcodeKitApp};

use crate::machine_support::app_harness;

/// The app over default settings, drawn once.
fn app() -> Harness<'static, GcodeKitApp> {
    let state = AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    );
    let mut h = app_harness(GcodeKitApp::from_state(state));
    h.run_steps(2);
    h
}

/// Clicks the page tab labelled `label`.
fn click_tab(h: &mut Harness<'_, GcodeKitApp>, label: &str) {
    h.get_by_role_and_label(Role::Tab, label).click();
    h.run_steps(2);
}

#[test]
fn test_pages_are_in_gtk_order() {
    let tr = Translator::new("en");
    let titles: Vec<String> = Page::ALL
        .into_iter()
        .map(|p| tr.tr(p.title_msgid()).into_owned())
        .collect();
    assert_eq!(
        titles,
        [
            "Designer",
            "Visualizer",
            "Machine Control",
            "CAM Tools",
            "Device Manager",
            "Device Config",
            "CNC Tools",
            "Materials",
        ]
    );
}

#[test]
fn test_app_starts_on_machine_control() {
    let h = app();
    assert_eq!(h.state().page(), Page::MachineControl);
    let tab = h.get_by_role_and_label(Role::Tab, "Machine Control");
    assert!(tab.accesskit_node().is_selected().unwrap_or(false));
}

/// Every page has a tab; clicking it shows the page: its panes are drawn, or, for a
/// manager page, the manager's own controls.
#[test]
fn test_each_page_is_reachable_by_its_tab() {
    let mut h = app();
    let tr = Translator::new("en");
    for page in Page::ALL {
        let label = tr.tr(page.title_msgid()).into_owned();
        click_tab(&mut h, &label);
        assert_eq!(h.state().page(), page, "{label}");
        let tab = h.get_by_role_and_label(Role::Tab, &label);
        assert!(
            tab.accesskit_node().is_selected().unwrap_or(false),
            "{label}"
        );
        for pane in page.panes() {
            assert!(h.state().workspace().is_drawn(*pane), "{label}: {pane:?}");
        }
        let marker = match page {
            Page::DeviceManager => Some("Devices"),
            Page::DeviceConfig => Some("Retrieve"),
            Page::CncTools => Some("New Tool"),
            Page::Materials => Some("New Material"),
            Page::Designer | Page::Visualizer | Page::MachineControl | Page::CamTools => None,
        };
        if let Some(marker) = marker {
            assert!(h.query_by_label(marker).is_some(), "{label}: {marker}");
        }
        for other in PaneId::ALL {
            if !page.panes().contains(&other) {
                assert!(!h.state().workspace().is_drawn(other), "{label}: {other:?}");
            }
        }
    }
}

/// The G-code editor and the console sit in the Machine Control page, beside its controls.
#[test]
fn test_machine_control_page_holds_editor_and_console() {
    assert_eq!(
        Page::MachineControl.panes(),
        [PaneId::MachineControl, PaneId::Editor, PaneId::Console]
    );
    let h = app();
    assert!(
        h.query_by_role_and_label(Role::TextInput, "G-Code Editor")
            .is_some()
    );
}

/// Switching pages keeps each pane's state: the editor's text survives a visit to the
/// Designer.
#[test]
fn test_switching_pages_keeps_pane_state() {
    let mut h = app();
    h.state()
        .editor()
        .borrow_mut()
        .buffer_mut()
        .replace_with_generated("G0 X1\n".to_string());
    click_tab(&mut h, "Designer");
    click_tab(&mut h, "Machine Control");
    assert_eq!(h.state().editor().borrow().buffer().text(), "G0 X1\n");
}

/// The docking arrangement an earlier egui build could serialize.
const OLD_LAYOUT: &str = r#"{"dock":{"version":2,"layout":{"root":{"Split":{"axis":"Horizontal","size":{"Ratio":0.22},"first":{"Tabs":{"panels":["MachineControl"],"active":0}},"second":{"Split":{"axis":"Horizontal","size":{"Ratio":0.75},"first":{"Split":{"axis":"Vertical","size":{"Ratio":0.75},"first":{"Tabs":{"panels":["Visualizer","Editor","Designer","CamTools"],"active":0}},"second":{"Tabs":{"panels":["Console"],"active":0}}}},"second":{"Split":{"axis":"Vertical","size":{"Ratio":0.5},"first":{"Tabs":{"panels":["DesignerLayers"],"active":0}},"second":{"Tabs":{"panels":["DesignerProperties"],"active":0}}}}}}}},"floating":[]},"hidden":[],"saved":null,"locked":false}}"#;

/// A settings file holding an old docking layout (in the `ui` table, where an earlier build
/// would have kept it) loads without a problem, keeps the user's other settings, and the
/// app starts on its default page.
#[test]
fn test_old_docking_layout_is_ignored() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/egui_workspace")
        .join(format!("old-layout-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let dirs = gcodekit6_settings::migrate::ConfigDirs::new(dir.join("legacy"), dir.join("config"));
    // Written through the settings store so the file has every field the loader expects.
    {
        let (mut settings, _) = SettingsManager::open(dirs.clone());
        let mut config = settings.config().clone();
        config.ui.language = "de".to_string();
        settings.apply(config).expect("write settings");
    }
    let file = dirs.config.join("config.toml");
    let text = std::fs::read_to_string(&file).expect("settings written");
    let layout = format!("workspace_layout = '{OLD_LAYOUT}'\n");
    let text = text.replacen("[ui]\n", &format!("[ui]\n{layout}"), 1);
    assert!(text.contains("workspace_layout"), "no [ui] table: {text}");
    std::fs::write(&file, text).expect("write old layout");

    let (settings, problem) = SettingsManager::open(dirs);
    assert_eq!(problem, None);
    assert_eq!(settings.config().ui.language, "de");
    let state = AppState::new(settings, crate::test_smoke::native_decision());
    let mut h = app_harness(GcodeKitApp::from_state(state));
    h.run_steps(2);
    assert_eq!(h.state().page(), Page::MachineControl);
    assert!(h.state().workspace().is_drawn(PaneId::MachineControl));
}

#[test]
fn test_page_titles_are_translated() {
    let tr = Translator::new("de");
    let title = tr.tr(Page::MachineControl.title_msgid());
    assert_ne!(title, "Machine Control");
}
