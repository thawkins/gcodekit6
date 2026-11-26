//! The Speeds and Feeds calculator: GTK's defaults render, Calculate waits for a tool, a
//! material and safe input, unsafe input is shown inline, and the results equal the GTK
//! page's computation for the same input (fixtures from `capture_cam_fixtures`).
//!
//! The fixtures in `tests/fixtures/cam/` are frozen references: `capture_cam_fixtures`, which
//! wrote them from the GTK pages, was in the GTK crate deleted in `26740e3` (its source is at
//! `6841d84`). Change a fixture only deliberately, by hand, in the commit that changes the
//! generator's output and says why.

use egui_kittest::kittest::Queryable;
use gcodekit5_camtools::speeds_feeds::OperationType;
use gcodekit5_core::data::materials::{MaterialId, init_standard_library as materials};
use gcodekit5_core::data::tools::{ToolId, init_standard_library as tools};
use gcodekit5_devicedb::model::DeviceProfile;
use gcodekit5_ui_egui::panes::cam_tools::CamTool;
use gcodekit5_ui_egui::panes::cam_tools::speeds_feeds::{SpeedsFeedsParams, SpeedsFeedsResults};

use crate::test_cam_drill_press::{fixture, open_tool, pane};
use crate::test_designer_files_app::frames;

/// Scrolls the page to the button `label` and presses it (the page is taller than the
/// pane).
fn press(h: &mut egui_kittest::Harness<'static, gcodekit5_ui_egui::GcodeKitApp>, label: &str) {
    h.get_by_label(label).scroll_to_me();
    frames(h, 2);
    h.get_by_label(label).click();
}

/// GTK's fixture device: a spindle of `max_rpm` and 1.5 kW, without coolant.
fn spindle_device(max_rpm: u32) -> DeviceProfile {
    DeviceProfile {
        has_spindle: true,
        max_spindle_speed_rpm: max_rpm,
        cnc_spindle_watts: 1500.0,
        has_coolant: false,
        ..Default::default()
    }
}

/// The page over the standard libraries on `device`, with `tool` and `material` chosen.
fn page(device: DeviceProfile, tool: &str, material: &str) -> SpeedsFeedsParams {
    let mut page = SpeedsFeedsParams::new(
        tools().get_all_tools().into_iter().cloned().collect(),
        materials()
            .get_all_materials()
            .into_iter()
            .cloned()
            .collect(),
        Some(device),
    );
    assert!(page.select_tool(&ToolId(tool.to_string())));
    assert!(page.select_material(&MaterialId(material.to_string())));
    page
}

/// The results as `name=value` lines, as the fixture holds them.
fn lines(r: &SpeedsFeedsResults) -> String {
    format!(
        "spindle_speed={}\nfeed_rate={}\nsurface_speed={}\nchip_load={}\nmrr={}\npower={}\n\
         deflection={}\n",
        r.spindle_speed, r.feed_rate, r.surface_speed, r.chip_load, r.mrr, r.power, r.deflection
    )
}

#[test]
fn test_cam_speeds_feeds_default_page_renders() {
    let h = open_tool(CamTool::SpeedsFeeds);
    for label in [
        "Calculated Results",
        "Active Device",
        "Select Tool...",
        "Select Material...",
        "No tool selected",
        "No material selected",
        "Operation Type:",
        "Depth of Cut (mm):",
        "Stepover (mm):",
        "Tool Stick-out (mm):",
        "Conservative Calculation",
        "Calculate",
        "Reset",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label} not shown");
    }
    let mut cam = pane(&h);
    let page = cam.speeds_feeds_mut();
    assert_eq!(page.depth_of_cut, "3.0");
    assert_eq!(page.stepover, "1.0");
    assert_eq!(page.stick_out, "25");
    assert!(page.conservative, "GTK's panel starts conservative");
}

#[test]
fn test_cam_speeds_feeds_matches_gtk_defaults() {
    let page = page(spindle_device(24000), "tool_1_4_flat", "metal_al_6061");
    let results = page.calculate().expect("calculates");
    assert_eq!(lines(&results), fixture("speeds_feeds_default.txt"));
}

#[test]
fn test_cam_speeds_feeds_matches_gtk_custom() {
    let mut page = page(spindle_device(6000), "tool_1_8_flat", "wood_oak_red");
    page.operation = OperationType::Adaptive;
    page.depth_of_cut = "6".into();
    page.stepover = "0".into();
    page.stick_out = "30".into();
    page.conservative = false;
    let results = page.calculate().expect("calculates");
    assert_eq!(lines(&results), fixture("speeds_feeds_custom.txt"));
}

/// Calculate in the pane shows the results on the page.
#[test]
fn test_cam_speeds_feeds_calculate_shows_results() {
    let mut h = open_tool(CamTool::SpeedsFeeds);
    *pane(&h).speeds_feeds_mut() = page(spindle_device(24000), "tool_1_4_flat", "metal_al_6061");
    frames(&mut h, 2);
    press(&mut h, "Calculate");
    frames(&mut h, 2);
    let spindle = pane(&h)
        .speeds_feeds_mut()
        .results()
        .map(|r| r.spindle_speed.clone());
    assert_eq!(spindle.as_deref(), Some("22497"));
    h.get_by_label("Calculated Results").scroll_to_me();
    frames(&mut h, 2);
    assert!(
        h.query_by_label("22497").is_some(),
        "spindle speed not shown"
    );
}

/// Makes the page's input unsafe.
type Breaker = fn(&mut SpeedsFeedsParams);

#[test]
fn test_cam_speeds_feeds_invalid_input_shows_error_and_disables_calculate() {
    let cases: [(Breaker, &str); 5] = [
        (|p| p.depth_of_cut = "-3".into(), "Must be greater than 0"),
        (|p| p.depth_of_cut = "80".into(), "Out of range"),
        (|p| p.stepover = "-1".into(), "Must not be negative"),
        (|p| p.stick_out = "deep".into(), "Not a number"),
        (
            |p| {
                if let Some(tool) = p.tool.as_mut() {
                    tool.diameter = 0.0;
                }
            },
            "Must be greater than 0",
        ),
    ];
    for (break_it, message) in cases {
        let mut h = open_tool(CamTool::SpeedsFeeds);
        let mut ready = page(spindle_device(24000), "tool_1_4_flat", "metal_al_6061");
        break_it(&mut ready);
        *pane(&h).speeds_feeds_mut() = ready;
        frames(&mut h, 2);
        assert!(h.query_by_label(message).is_some(), "{message} not shown");
        press(&mut h, "Calculate");
        frames(&mut h, 2);
        assert_eq!(
            pane(&h).speeds_feeds_mut().results().cloned(),
            None,
            "{message}: calculated"
        );
    }
}

/// GTK refused a device without a spindle, or with its RPM or power unset.
#[test]
fn test_cam_speeds_feeds_needs_a_configured_spindle() {
    let no_spindle = DeviceProfile {
        has_spindle: false,
        ..spindle_device(24000)
    };
    let unset = DeviceProfile {
        cnc_spindle_watts: 0.0,
        ..spindle_device(24000)
    };
    let mut none = page(spindle_device(24000), "tool_1_4_flat", "metal_al_6061");
    none.device = None;
    for (ready, message) in [
        (
            page(no_spindle, "tool_1_4_flat", "metal_al_6061"),
            NO_SPINDLE,
        ),
        (page(unset, "tool_1_4_flat", "metal_al_6061"), SPINDLE_UNSET),
        (none, NO_SPINDLE),
    ] {
        assert!(ready.calculate().is_err());
        let mut h = open_tool(CamTool::SpeedsFeeds);
        *pane(&h).speeds_feeds_mut() = ready;
        frames(&mut h, 2);
        assert!(h.query_by_label(message).is_some(), "{message} not shown");
    }
}

/// The page's refusal when the device has no spindle.
const NO_SPINDLE: &str = "The active device does not have a spindle configured. Please configure a spindle in the Device Manager.";
/// The page's refusal when the spindle's RPM or power is unset.
const SPINDLE_UNSET: &str = "Set the spindle's maximum RPM and power in the Device Manager.";

/// GTK's clamp: a spindle slower than the calculator's 1000 RPM floor gets the clamped RPM
/// and a scaled feed, formatted as GTK did.
#[test]
fn test_cam_speeds_feeds_matches_gtk_clamped() {
    let page = page(spindle_device(500), "tool_1_4_flat", "metal_al_6061");
    let results = page.calculate().expect("calculates");
    assert!(
        results.spindle_speed.contains("clamped from"),
        "{results:?}"
    );
    assert_eq!(lines(&results), fixture("speeds_feeds_clamped.txt"));
}

/// The results are withdrawn as soon as the input changes.
#[test]
fn test_cam_speeds_feeds_results_clear_when_the_input_changes() {
    let mut h = open_tool(CamTool::SpeedsFeeds);
    *pane(&h).speeds_feeds_mut() = page(spindle_device(24000), "tool_1_4_flat", "metal_al_6061");
    frames(&mut h, 2);
    press(&mut h, "Calculate");
    frames(&mut h, 2);
    assert!(pane(&h).speeds_feeds_mut().results().is_some());
    pane(&h).speeds_feeds_mut().depth_of_cut = "2.0".into();
    frames(&mut h, 2);
    assert_eq!(pane(&h).speeds_feeds_mut().results(), None);
    assert!(h.query_by_label("22497").is_none());
}

/// Calculate waits for a tool and a material.
#[test]
fn test_cam_speeds_feeds_waits_for_a_selection() {
    let mut h = open_tool(CamTool::SpeedsFeeds);
    press(&mut h, "Calculate");
    frames(&mut h, 2);
    assert_eq!(pane(&h).speeds_feeds_mut().results().cloned(), None);
}

/// The tool picker: search, choose a row, Select.
#[test]
fn test_cam_speeds_feeds_tool_picker_selects_a_tool() {
    let mut h = open_tool(CamTool::SpeedsFeeds);
    h.get_by_label("Select Tool...").click();
    frames(&mut h, 2);
    assert!(h.query_by_label("Select Tool").is_some(), "picker not open");
    pane(&h).speeds_feeds_mut().set_picker_search("1/4\" Flat");
    frames(&mut h, 2);
    h.get_by_label_contains("1/4\" Flat End Mill").click();
    frames(&mut h, 1);
    h.get_by_label("Select").click();
    frames(&mut h, 2);
    let chosen = pane(&h)
        .speeds_feeds_mut()
        .tool
        .as_ref()
        .map(|t| t.id.0.clone());
    assert_eq!(chosen.as_deref(), Some("tool_1_4_flat"));
}

/// The app gives the calculator the active device from the GTK build's `devices.json` in the
/// settings folder.
#[test]
fn test_cam_speeds_feeds_reads_the_active_device() -> anyhow::Result<()> {
    use gcodekit5_settings::SettingsManager;
    use gcodekit5_settings::migrate::ConfigDirs;
    let dir = crate::test_designer_files::scratch("cam_devices")?;
    let manager = gcodekit5_devicedb::DeviceManager::new(dir.join("legacy/devices.json"));
    std::fs::create_dir_all(dir.join("legacy"))?;
    let device = DeviceProfile {
        name: "Router 4040".to_string(),
        ..spindle_device(18000)
    };
    manager.save_profile(device.clone())?;
    manager.set_active_profile(&device.id)?;
    let (settings, _problem) =
        SettingsManager::open(ConfigDirs::new(dir.join("legacy"), dir.join("config")));
    let state = gcodekit5_ui_egui::AppState::new(settings, crate::test_smoke::native_decision());
    let app = gcodekit5_ui_egui::GcodeKitApp::from_state(state);
    let mut cam = app.cam_tools().borrow_mut();
    let active = cam
        .speeds_feeds_mut()
        .device
        .as_ref()
        .map(|d| d.name.clone());
    assert_eq!(active.as_deref(), Some("Router 4040"));
    Ok(())
}
