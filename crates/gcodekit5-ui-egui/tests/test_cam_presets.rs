//! CAM presets: Save then Load gives the page back, presets written by the GTK pages load
//! (fixtures captured by `capture_cam_fixtures`), and a file that is not a preset leaves the
//! page as it was with a notice.

use egui_kittest::kittest::Queryable;
use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_ui_egui::panes::cam_tools::CamTool;
use gcodekit5_ui_egui::panes::cam_tools::common::format_len;
use gcodekit5_ui_egui::panes::cam_tools::drill_press::DrillPressParams;
use gcodekit5_ui_egui::panes::cam_tools::spoilboard_grid::{LaserMode, SpoilboardGridParams};
use gcodekit5_ui_egui::panes::cam_tools::spoilboard_surfacing::SpoilboardSurfacingParams;

use crate::test_cam_drill_press::{fixture, pane};
use crate::test_cam_tools::cam_app;
use crate::test_designer_files::scratch;
use crate::test_designer_files_app::{answer, default_state, frames, path, shown};

const METRIC: MeasurementSystem = MeasurementSystem::Metric;

fn tr() -> Translator {
    Translator::new("en")
}

fn mm(v: f64) -> String {
    format_len(v, METRIC)
}

/// A drill page with every field changed from GTK's defaults.
fn custom_drill() -> DrillPressParams {
    let mut p = DrillPressParams::with_center(METRIC, (100.0, 100.0));
    p.hole_diameter = mm(12.7);
    p.tool_diameter = mm(3.175);
    p.x = mm(42.0);
    p.y = mm(17.25);
    p.top_z = mm(1.0);
    p.bottom_z = mm(-7.5);
    p.peck_depth = mm(1.5);
    p.plunge_rate = "150".into();
    p.feed_rate = "650".into();
    p.spindle_speed = "9000".into();
    p.safe_z = mm(4.0);
    p.home_before = true;
    p
}

#[test]
fn test_cam_preset_round_trip_keeps_every_field() {
    let tr = tr();
    let drill = custom_drill();
    let mut loaded = DrillPressParams::with_center(METRIC, (100.0, 100.0));
    let json = drill.preset_json(&tr).expect("saves");
    loaded.apply_preset(&json).expect("loads");
    assert_eq!(loaded, drill);

    let mut surfacing = SpoilboardSurfacingParams::new(METRIC);
    surfacing.width = mm(500.0);
    surfacing.height = mm(250.5);
    surfacing.tool_diameter = mm(19.05);
    surfacing.feed_rate = "1500".into();
    surfacing.spindle_speed = "16000".into();
    surfacing.cut_depth = mm(0.25);
    surfacing.stepover_percent = "60".into();
    surfacing.safe_z = mm(8.0);
    let mut loaded = SpoilboardSurfacingParams::new(METRIC);
    let json = surfacing.preset_json(&tr).expect("saves");
    loaded.apply_preset(&json).expect("loads");
    assert_eq!(loaded, surfacing);

    let mut grid = SpoilboardGridParams::new(METRIC);
    grid.width = mm(600.0);
    grid.height = mm(400.0);
    grid.grid_spacing = mm(25.4);
    grid.feed_rate = "2000".into();
    grid.laser_power = "800".into();
    grid.laser_mode = LaserMode::M3;
    let mut loaded = SpoilboardGridParams::new(METRIC);
    let json = grid.preset_json(&tr).expect("saves");
    loaded.apply_preset(&json).expect("loads");
    assert_eq!(loaded, grid);
}

/// The presets the GTK pages wrote load, and egui writes the same text for the same input.
#[test]
fn test_cam_preset_loads_the_gtk_files() {
    let tr = tr();
    let drill_json = fixture("preset_drill_press.json");
    let mut drill = DrillPressParams::with_center(METRIC, (100.0, 100.0));
    drill.apply_preset(&drill_json).expect("loads");
    assert_eq!(drill, custom_drill());
    assert_eq!(drill.preset_json(&tr).as_ref(), Ok(&drill_json));

    let surfacing_json = fixture("preset_spoilboard_surfacing.json");
    let mut surfacing = SpoilboardSurfacingParams::new(METRIC);
    surfacing.apply_preset(&surfacing_json).expect("loads");
    assert_eq!(surfacing.tool_diameter, mm(19.05));
    assert_eq!(surfacing.height, mm(250.5));
    assert_eq!(surfacing.stepover_percent, "60");
    assert_eq!(surfacing.spindle_speed, "16000");
    assert_eq!(surfacing.preset_json(&tr).as_ref(), Ok(&surfacing_json));

    let grid_json = fixture("preset_spoilboard_grid.json");
    let mut grid = SpoilboardGridParams::new(METRIC);
    grid.apply_preset(&grid_json).expect("loads");
    assert_eq!(grid.grid_spacing, mm(25.4));
    assert_eq!(grid.laser_power, "800");
    assert_eq!(grid.laser_mode, LaserMode::M3);
    assert_eq!(grid.preset_json(&tr).as_ref(), Ok(&grid_json));
}

/// Save asks where (GTK's file name), writes the preset; Load reads it back into the page.
#[test]
fn test_cam_preset_save_and_load_through_the_dialogs() -> std::io::Result<()> {
    let file = scratch("cam_preset")?.join("drill.json");
    let (mut h, script) = cam_app(default_state());
    h.get_by_label("Drill Press").click();
    frames(&mut h, 1);
    h.get_by_label("Open").click();
    frames(&mut h, 2);
    *pane(&h).drill_press_mut() = custom_drill();
    answer(&script, [path(&file)]);
    h.get_by_label("Save").click();
    frames(&mut h, 3);
    let saved = std::fs::read_to_string(&file)?;
    assert_eq!(Ok(saved), custom_drill().preset_json(&tr()));

    pane(&h).drill_press_mut().hole_diameter = mm(30.0);
    answer(&script, [path(&file)]);
    h.get_by_label("Load").click();
    frames(&mut h, 3);
    assert_eq!(*pane(&h).drill_press_mut(), custom_drill());
    assert_eq!(shown(&script), ["Save Parameters", "Load Parameters"]);
    assert_eq!(pane(&h).notice(), None);
    Ok(())
}

/// A file that is not the page's preset is reported and changes nothing.
#[test]
fn test_cam_preset_malformed_file_shows_an_error_and_keeps_the_page() -> std::io::Result<()> {
    let dir = scratch("cam_preset_bad")?;
    let (mut h, script) = cam_app(default_state());
    h.get_by_label("Spoilboard Surfacing").click();
    frames(&mut h, 1);
    h.get_by_label("Open").click();
    frames(&mut h, 2);
    for (name, text) in [("broken.json", "{\"width\": 5"), ("list.json", "[1, 2]")] {
        let file = dir.join(name);
        std::fs::write(&file, text)?;
        let before = pane(&h).spoilboard_surfacing_mut().clone();
        answer(&script, [path(&file)]);
        h.get_by_label("Load").click();
        frames(&mut h, 3);
        let notice = pane(&h).notice().map(str::to_string);
        assert!(
            notice
                .as_deref()
                .is_some_and(|n| n.starts_with("Load Error: ")),
            "{name}: {notice:?}"
        );
        assert_eq!(*pane(&h).spoilboard_surfacing_mut(), before, "{name}");
    }
    // A drill page refuses a surfacing preset (its keys are missing).
    let mut drill = DrillPressParams::with_center(METRIC, (100.0, 100.0));
    let before = drill.clone();
    assert!(
        drill
            .apply_preset(&fixture("preset_spoilboard_surfacing.json"))
            .is_err()
    );
    assert_eq!(drill, before);
    Ok(())
}

/// Save refuses a length it cannot read, as GTK did, naming the field.
#[test]
fn test_cam_preset_save_refuses_an_unreadable_length() {
    let mut p = SpoilboardSurfacingParams::new(METRIC);
    p.width = "wide".into();
    assert_eq!(
        p.preset_json(&tr()),
        Err("Width: Not a valid length".to_string())
    );
    let mut h = crate::test_cam_drill_press::open_tool(CamTool::SpoilboardGrid);
    pane(&h).spoilboard_grid_mut().height = "tall".into();
    frames(&mut h, 2);
    h.get_by_label("Save").scroll_to_me();
    frames(&mut h, 2);
    h.get_by_label("Save").click();
    frames(&mut h, 2);
    assert_eq!(
        pane(&h).notice(),
        Some("Save Error: Height: Not a valid length")
    );
    assert_eq!(pane(&h).take_preset_request(), None, "no dialog is asked");
}
