//! The CAM Tools page and the four manager pages (Device Manager, Device Config, CNC Tools,
//! Materials) in GTK's layout: at 1280x800 (1x and 2x) each page's primary buttons are inside
//! the page without scrolling, and a snapshot of each page at 1440x900 dark.

use std::path::{Path, PathBuf};

use egui_kittest::kittest::Queryable;
use egui_kittest::{Harness, SnapshotOptions};
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::workspace::Page;
use gcodekit5_ui_egui::{AppState, GcodeKitApp};

/// A fresh scratch folder `target/temp/egui_manager_pages/<name>-<pid>`.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/egui_manager_pages")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

/// The app without a machine connection, its profiles (the GTK fixture), tools and
/// materials in a scratch folder.
fn app(name: &str) -> GcodeKitApp {
    let dir = scratch(name);
    let devices = dir.join("devices.json");
    std::fs::write(&devices, crate::test_device_manager::GTK_DEVICES).expect("devices.json");
    let state = AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    )
    .with_device_file(devices)
    .with_tools_file(dir.join("custom_tools.json"))
    .with_materials_file(dir.join("custom_materials.json"));
    GcodeKitApp::from_state(state)
}

/// A few GRBL settings as Device Config's Save writes them, to fill its list.
const SETTINGS: &str = r#"[
  {"number": 0, "name": "Step pulse time", "value": "10", "numeric_value": 10.0,
   "description": "Sets time length per step", "range": null, "read_only": false,
   "unit": "microseconds"},
  {"number": 1, "name": "Step idle delay", "value": "25", "numeric_value": 25.0,
   "description": "Sets a short hold delay when stopping", "range": null,
   "read_only": false, "unit": "milliseconds"},
  {"number": 100, "name": "X-axis travel resolution", "value": "800.000",
   "numeric_value": 800.0, "description": "X-axis travel resolution in steps per millimeter",
   "range": null, "read_only": false, "unit": "step/mm"},
  {"number": 110, "name": "X-axis maximum rate", "value": "3000.000",
   "numeric_value": 3000.0, "description": "X-axis maximum rate", "range": null,
   "read_only": false, "unit": "mm/min"}
]"#;

/// Builds a harness of `size` points at `ppp`, dark, showing `page`.
fn page_harness(
    name: &str,
    page: Page,
    size: egui::Vec2,
    ppp: f32,
    wgpu: bool,
) -> Harness<'static, GcodeKitApp> {
    let builder = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(ppp)
        .with_theme(egui::Theme::Dark);
    let builder = if wgpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_ui_state(
        |ui, app: &mut GcodeKitApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        app(name),
    );
    gcodekit5_ui_egui::theme::install(&harness.ctx);
    if page == Page::DeviceConfig {
        // A path relative to the crate (nextest's working directory), so the notice that
        // names it is the same on every machine and run.
        let dir = Path::new("../../target/temp/egui_manager_pages");
        std::fs::create_dir_all(dir).expect("scratch folder");
        let file = dir.join(format!("{name}.json"));
        std::fs::write(&file, SETTINGS).expect("settings.json");
        let tr = harness.state().state().translator;
        harness
            .state_mut()
            .firmware_settings_mut()
            .load_from(&file, &tr);
    }
    harness.state_mut().show_page(page);
    harness.run_steps(3);
    assert_eq!(harness.state().page(), page);
    harness
}

/// Clicks each of `labels` in turn (a list entry, then a button), stepping after each.
fn select(harness: &mut Harness<'static, GcodeKitApp>, labels: &[&str]) {
    for label in labels {
        harness.get_by_label(label).click();
        harness.run_steps(3);
    }
}

/// Snapshots `harness` as `name` under `tests/snapshots`.
fn snapshot(harness: &mut Harness<'static, GcodeKitApp>, name: &str) {
    let options = SnapshotOptions::new()
        .threshold(1.0)
        .max_failed_pixels(40)
        .output_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots"));
    if let Err(error) = harness.try_snapshot_options(name, &options) {
        panic!("{error}");
    }
}

/// A page, what to click first (a list entry, a button), and the buttons that must be
/// visible.
struct PageCase {
    page: Page,
    name: &'static str,
    select: &'static [&'static str],
    primary: &'static [&'static str],
}

const CASES: [PageCase; 7] = [
    PageCase {
        page: Page::CamTools,
        name: "cam_tools_page_dark",
        select: &[],
        primary: &["Tabbed Box Maker", "Probe Tools", "Open"],
    },
    PageCase {
        page: Page::CamTools,
        name: "cam_tool_spoilboard_grid_dark",
        select: &["Create Spoilboard Grid", "Open"],
        primary: &[
            "Back",
            "Generate",
            "Send to Editor",
            "Preview",
            "Load",
            "Save",
        ],
    },
    PageCase {
        page: Page::CamTools,
        name: "cam_tool_probe_dark",
        select: &["Probe Tools", "Open"],
        primary: &["Back", "Generate", "Probe"],
    },
    PageCase {
        page: Page::DeviceManager,
        name: "device_manager_page_dark",
        select: &["Router 4040"],
        primary: &["Add Device", "Save", "Cancel", "Delete", "Set Active"],
    },
    PageCase {
        page: Page::DeviceConfig,
        name: "device_config_page_dark",
        select: &[],
        primary: &["Retrieve", "Save", "Load"],
    },
    PageCase {
        page: Page::CncTools,
        name: "cnc_tools_page_dark",
        select: &["1/4\" Flat End Mill"],
        primary: &[
            "New Tool",
            "Save",
            "Cancel",
            "Delete",
            "Reset Custom Tools...",
        ],
    },
    PageCase {
        page: Page::Materials,
        name: "materials_page_dark",
        select: &["Acrylic"],
        primary: &["New Material", "Save", "Cancel", "Delete"],
    },
];

#[test]
fn test_manager_page_primary_buttons_visible_at_1280x800() {
    for ppp in [1.0, 2.0] {
        for case in &CASES {
            let size = egui::vec2(1280.0, 800.0);
            let name = format!("{}-{ppp}", case.name);
            let mut harness = page_harness(&name, case.page, size, ppp, false);
            select(&mut harness, case.select);
            let bottom = harness.get_by_label("eStop").rect().top();
            let page = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(size.x, bottom));
            for label in case.primary {
                let nodes: Vec<_> = harness.query_all_by_label(label).collect();
                assert!(
                    !nodes.is_empty(),
                    "no {label} on {:?} at ppp {ppp}",
                    case.page
                );
                for node in nodes {
                    let rect = node.rect();
                    assert!(
                        page.contains_rect(rect),
                        "{label} at {rect:?} outside {page:?} on {:?} at ppp {ppp}",
                        case.page
                    );
                }
            }
        }
    }
}

#[test]
fn test_snapshot_manager_pages_dark() {
    for case in &CASES {
        let mut harness = page_harness(case.name, case.page, egui::vec2(1440.0, 900.0), 1.0, true);
        select(&mut harness, case.select);
        // Park the pointer on empty status bar room, away from every control.
        harness.hover_at(egui::pos2(1400.0, 880.0));
        harness.run_steps(2);
        snapshot(&mut harness, case.name);
    }
}

#[test]
fn test_cam_tool_rows_hold_their_wrapped_descriptions_at_1280x800() {
    use gcodekit5_ui_egui::panes::cam_tools::{CAPTION_SIZE, CamTool};
    let harness = page_harness(
        "cam_rows",
        Page::CamTools,
        egui::vec2(1280.0, 800.0),
        1.0,
        false,
    );
    let tr = harness.state().state().translator;
    let body = egui::TextStyle::Body.resolve(&harness.ctx.global_style());
    let mut previous_bottom: Option<f32> = None;
    let mut grew = false;
    for tool in CamTool::ALL {
        let title = tr.tr(tool.title_msgid()).into_owned();
        let row = harness.get_by_label(&title).rect();
        // The row's text column: the row less its margins and the icon (12 + 24 + 12 + 12).
        let wrap = row.width() - 60.0;
        let (title_h, text_h) = harness.ctx.fonts_mut(|f| {
            let mut layout = |text: String, font: egui::FontId| {
                f.layout(text, font, egui::Color32::WHITE, wrap).size().y
            };
            (
                layout(title.clone(), body.clone()),
                layout(
                    tr.tr(tool.description_msgid()).into_owned(),
                    egui::FontId::proportional(CAPTION_SIZE),
                ),
            )
        });
        let needed = 10.0 + title_h + 4.0 + text_h + 10.0;
        assert!(
            row.height() + 0.5 >= needed,
            "{title}: row {} pt tall, its text needs {needed}",
            row.height()
        );
        grew |= text_h > CAPTION_SIZE * 1.6;
        if let Some(bottom) = previous_bottom {
            assert!(row.top() + 0.5 >= bottom, "{title} overlaps the row above");
        }
        previous_bottom = Some(row.bottom());
    }
    // At 1280 the Probe Tools caption wraps, so at least one row must have grown.
    assert!(
        grew,
        "no description wrapped at 1280; the test checks nothing"
    );
}
