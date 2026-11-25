//! The GTK CAM pages' output for fixed inputs, kept as fixtures for the egui front end.
//!
//! Each case builds the page's entries the way the GTK page does (its initial widget values,
//! or explicit text), maps them with the page's own mapping and runs the same
//! `gcodekit5_camtools` generator. The GTK pages apply no post-processing to the generator's
//! output: Drill Press hands it to the machine, the spoilboard pages to the editor.
//!
//! `capture_cam_fixtures` is ignored; run it explicitly to (re)write the fixtures under
//! `crates/gcodekit5-ui-egui/tests/fixtures/cam/`:
//! `cargo nextest run -p gcodekit5-ui --run-ignored only -E 'test(capture_cam_fixtures)'`.
//! `test_cam_fixtures_match_gtk` checks, without writing, that the committed fixtures are
//! still what the GTK pages produce.

use std::path::PathBuf;

use gcodekit5_camtools::drill_press::DrillPressGenerator;
use gcodekit5_camtools::speeds_feeds::OperationType;
use gcodekit5_camtools::spoilboard_grid::SpoilboardGridGenerator;
use gcodekit5_camtools::spoilboard_surfacing::SpoilboardSurfacingGenerator;
use gcodekit5_core::data::materials::{MaterialId, init_standard_library as materials};
use gcodekit5_core::data::tools::{ToolId, init_standard_library as tools};
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_devicedb::model::DeviceProfile;
use gcodekit5_ui::ui::gtk::cam_tools::drill_press::{DrillPressEntries, drill_press_parameters};
use gcodekit5_ui::ui::gtk::cam_tools::speeds_feeds_page::{
    CONSERVATIVE, DEPTH_OF_CUT_MM, STEPOVER_MM, SpeedsFeedsSelection, TOOL_STICK_OUT_MM,
    speeds_feeds_texts,
};
use gcodekit5_ui::ui::gtk::cam_tools::spoilboard_grid::{
    SpoilboardGridEntries, spoilboard_grid_parameters,
};
use gcodekit5_ui::ui::gtk::cam_tools::spoilboard_surfacing::{
    SpoilboardSurfacingEntries, spoilboard_surfacing_parameters,
};

/// The egui crate's CAM fixture folder.
fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../gcodekit5-ui-egui/tests/fixtures/cam")
}

/// Drill press: the page's defaults (no GRBL travel known: centre 100, 100), and a helical
/// hole without pecking.
fn drill_press_cases() -> Vec<(&'static str, String)> {
    let default = DrillPressEntries::defaults(MeasurementSystem::Metric, (100.0, 100.0));
    let custom = DrillPressEntries {
        hole_diameter: "12".into(),
        tool_diameter: "6".into(),
        top_z: "0".into(),
        bottom_z: "-5".into(),
        peck_depth: "0".into(),
        safe_z: "3".into(),
        x: "50".into(),
        y: "25".into(),
        plunge_rate: "200".into(),
        feed_rate: "800".into(),
        spindle_speed: "12000".into(),
        home_before: true,
    };
    [("default", default), ("custom", custom)]
        .into_iter()
        .map(|(name, entries)| {
            let params = drill_press_parameters(&entries, MeasurementSystem::Metric)
                .expect("drill press entries parse");
            let gcode = DrillPressGenerator::new(params)
                .generate()
                .expect("drill press generates");
            (name, gcode)
        })
        .collect()
}

/// Spoilboard surfacing: the defaults, and an imperial case.
fn surfacing_cases() -> Vec<(&'static str, String)> {
    let default = (
        SpoilboardSurfacingEntries::defaults(MeasurementSystem::Metric),
        MeasurementSystem::Metric,
    );
    let custom = (
        SpoilboardSurfacingEntries {
            width: "12".into(),
            height: "8".into(),
            tool_diameter: "1".into(),
            feed_rate: "1200".into(),
            spindle_speed: "16000".into(),
            cut_depth: "0.02".into(),
            stepover_percent: "50".into(),
            safe_z: "0.25".into(),
            home_before: true,
        },
        MeasurementSystem::Imperial,
    );
    [("default", default), ("custom", custom)]
        .into_iter()
        .map(|(name, (entries, system))| {
            let params = spoilboard_surfacing_parameters(&entries, system);
            let gcode = SpoilboardSurfacingGenerator::new(params)
                .generate()
                .expect("surfacing generates");
            (name, gcode)
        })
        .collect()
}

/// Spoilboard grid: the defaults, and M3 with homing.
fn grid_cases() -> Vec<(&'static str, String)> {
    let default = SpoilboardGridEntries::defaults(MeasurementSystem::Metric);
    let custom = SpoilboardGridEntries {
        width: "200".into(),
        height: "150".into(),
        grid_spacing: "25".into(),
        feed_rate: "1500".into(),
        laser_power: "500".into(),
        laser_mode: "M3".into(),
        home_before: true,
    };
    [("default", default), ("custom", custom)]
        .into_iter()
        .map(|(name, entries)| {
            let params = spoilboard_grid_parameters(&entries, MeasurementSystem::Metric);
            let gcode = SpoilboardGridGenerator::new(params)
                .generate()
                .expect("grid generates");
            (name, gcode)
        })
        .collect()
}

/// A device with a spindle of `max_rpm` and 1.5 kW, without coolant.
fn spindle_device(max_rpm: u32) -> DeviceProfile {
    DeviceProfile {
        has_spindle: true,
        max_spindle_speed_rpm: max_rpm,
        cnc_spindle_watts: 1500.0,
        has_coolant: false,
        ..Default::default()
    }
}

/// Speeds and feeds: the panel's defaults (1/4" flat end mill in 6061 aluminium on a
/// 24000 RPM spindle without coolant); a custom case (1/8" flat end mill in red oak,
/// adaptive, operation-default stepover, not conservative, 6000 RPM spindle), whose RPM the
/// calculator itself keeps under the spindle's maximum; and a clamped case (the defaults on a
/// 500 RPM spindle: the calculator raises the RPM to its 1000 RPM floor, so the page clamps
/// it and scales the feed, "(clamped from ...)" / "(adjusted)"). Each result is one
/// `name=value` line.
fn speeds_feeds_cases() -> Vec<(&'static str, String)> {
    let tool = |id: &str| {
        tools()
            .get_tool(&ToolId(id.to_string()))
            .cloned()
            .expect("standard tool")
    };
    let material = |id: &str| {
        materials()
            .get_material(&MaterialId(id.to_string()))
            .cloned()
            .expect("standard material")
    };
    let default = (
        SpeedsFeedsSelection {
            tool: tool("tool_1_4_flat"),
            material: material("metal_al_6061"),
            operation: OperationType::Slotting,
            depth_of_cut: DEPTH_OF_CUT_MM,
            width_of_cut: Some(STEPOVER_MM),
            tool_stick_out: TOOL_STICK_OUT_MM,
            coolant_enabled: false,
            conservative: CONSERVATIVE,
        },
        spindle_device(24000),
    );
    let custom = (
        SpeedsFeedsSelection {
            tool: tool("tool_1_8_flat"),
            material: material("wood_oak_red"),
            operation: OperationType::Adaptive,
            depth_of_cut: 6.0,
            width_of_cut: None,
            tool_stick_out: 30.0,
            coolant_enabled: false,
            conservative: false,
        },
        spindle_device(6000),
    );
    let clamped = (default.0.clone(), spindle_device(500));
    [
        ("default", default),
        ("custom", custom),
        ("clamped", clamped),
    ]
    .into_iter()
    .map(|(name, (selection, device))| {
        let t = speeds_feeds_texts(&selection, Some(&device)).expect("spindle configured");
        let text = format!(
            "spindle_speed={}\nfeed_rate={}\nsurface_speed={}\nchip_load={}\nmrr={}\n\
                 power={}\ndeflection={}\n",
            t.spindle_speed,
            t.feed_rate,
            t.surface_speed,
            t.chip_load,
            t.mrr,
            t.power,
            t.deflection
        );
        (name, text)
    })
    .collect()
}

/// Every fixture: file name and content.
fn fixtures() -> Vec<(String, String)> {
    let named = |tool: &str, ext: &str, cases: Vec<(&'static str, String)>| {
        cases
            .into_iter()
            .map(|(case, text)| (format!("{tool}_{case}.{ext}"), text))
            .collect::<Vec<_>>()
    };
    [
        named("drill_press", "gcode", drill_press_cases()),
        named("spoilboard_surfacing", "gcode", surfacing_cases()),
        named("spoilboard_grid", "gcode", grid_cases()),
        named("speeds_feeds", "txt", speeds_feeds_cases()),
    ]
    .concat()
}

#[test]
#[ignore = "writes the egui CAM fixtures; run explicitly to capture them"]
fn capture_cam_fixtures() {
    let dir = fixture_dir();
    std::fs::create_dir_all(&dir).expect("create the fixture folder");
    for (name, text) in fixtures() {
        std::fs::write(dir.join(&name), text).expect("write a fixture");
    }
}

#[test]
fn test_cam_fixtures_match_gtk() {
    let dir = fixture_dir();
    for (name, text) in fixtures() {
        let fixture = std::fs::read_to_string(dir.join(&name)).expect("read a fixture");
        assert_eq!(fixture, text, "{name} differs from the GTK page's output");
    }
}
