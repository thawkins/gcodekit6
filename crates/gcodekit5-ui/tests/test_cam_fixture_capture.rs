//! The GTK CAM pages' output for fixed inputs, kept as fixtures for the egui front end.
//!
//! Each case builds the page's entries the way the GTK page does (its initial widget values,
//! or explicit text), maps them with the page's own mapping and runs the same
//! `gcodekit5_camtools` generator. The GTK pages apply no post-processing to the generator's
//! output: Drill Press hands it to the machine, the spoilboard pages to the editor.
//!
//! The jigsaw page makes paths for the designer, not a program; its fixture lists each
//! path with whether the page imports it closed. The tabbed box page also has "Generate
//! Shapes": its fixture lists the panel outlines. The Gerber pages read the files under
//! `fixtures/cam/gerber/` (a small top copper layer and a board outline).
//!
//! The bitmap engraving cases engrave `fixtures/cam/bitmap/gradient.png` (24 x 16 pixels).
//! The engraver writes the time of generation into its header; that line is replaced by
//! [`GENERATED_MASK`] in the fixtures, and the egui tests mask it the same way. The probe
//! cases are the programs Generate G-code makes from a fixed start position (the routines
//! move relative to the probe; the position is only written into the header).
//!
//! `capture_cam_fixtures` is ignored; run it explicitly to (re)write the fixtures under
//! `crates/gcodekit5-ui-egui/tests/fixtures/cam/`:
//! `cargo nextest run -p gcodekit5-ui --run-ignored only -E 'test(capture_cam_fixtures)'`.
//! `test_cam_fixtures_match_gtk` checks, without writing, that the committed fixtures are
//! still what the GTK pages produce.

use std::path::PathBuf;

use gcodekit5_camtools::drill_press::DrillPressGenerator;
use gcodekit5_camtools::gerber::GerberConverter;
use gcodekit5_camtools::jigsaw_puzzle::JigsawPuzzleMaker;
use gcodekit5_camtools::laser_engraver::BitmapImageEngraver;
use gcodekit5_camtools::probe_routines::ProbeRoutineEngine;
use gcodekit5_camtools::speeds_feeds::OperationType;
use gcodekit5_camtools::spoilboard_grid::SpoilboardGridGenerator;
use gcodekit5_camtools::spoilboard_surfacing::SpoilboardSurfacingGenerator;
use gcodekit5_camtools::tabbed_box::TabbedBoxMaker;
use gcodekit5_core::Position;
use gcodekit5_core::data::materials::{MaterialId, init_standard_library as materials};
use gcodekit5_core::data::tools::{ToolId, init_standard_library as tools};
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_devicedb::model::DeviceProfile;
use gcodekit5_ui::ui::gtk::cam_tools::bitmap_engraving::{
    BitmapEngravingEntries, bitmap_engraving_parameters, bitmap_engraving_preset,
};
use gcodekit5_ui::ui::gtk::cam_tools::drill_press::{DrillPressEntries, drill_press_parameters};
use gcodekit5_ui::ui::gtk::cam_tools::gerber::{GerberEntries, gerber_parameters};
use gcodekit5_ui::ui::gtk::cam_tools::jigsaw::{
    JigsawEntries, jigsaw_parameters, jigsaw_path_closed,
};
use gcodekit5_ui::ui::gtk::cam_tools::probe_routines::{
    ProbeEntries, probe_options, probe_preset, probe_routine,
};
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

use gcodekit5_ui::ui::gtk::cam_tools::tabbed_box::{
    TabbedBoxEntries, box_path_points, tabbed_box_parameters,
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

/// The axes the capture assumes the active device has (the GTK pages ask the device; the
/// default device has three).
const AXES: u8 = 3;

/// Paths as the fixtures list them: one `path N closed=B points=P` line each, then one
/// `x y` line per point (shortest exact `f64` text).
fn paths_text(paths: &[Vec<(f64, f64)>], closed: impl Fn(usize, usize) -> bool) -> String {
    let mut text = String::new();
    for (i, path) in paths.iter().enumerate() {
        let closed = closed(i, paths.len());
        text.push_str(&format!("path {i} closed={closed} points={}\n", path.len()));
        for (x, y) in path {
            text.push_str(&format!("{x} {y}\n"));
        }
    }
    text
}

/// The jigsaw page with every field changed, in millimetres.
fn jigsaw_custom() -> JigsawEntries {
    JigsawEntries {
        width: "300".into(),
        height: "200".into(),
        pieces_across: "5".into(),
        pieces_down: "4".into(),
        seed: "7".into(),
        tab_size: "25".into(),
        jitter: "8".into(),
        corner_radius: "3".into(),
    }
}

/// The tabbed box page with every field changed, in millimetres.
fn tabbed_box_preset_entries() -> TabbedBoxEntries {
    TabbedBoxEntries {
        width: "120".into(),
        depth: "90.5".into(),
        height: "60".into(),
        outside: true,
        thickness: "4".into(),
        burn: "0.15".into(),
        finger_width: "1.5".into(),
        space_width: "2.5".into(),
        surrounding_spaces: "1".into(),
        play: "0.05".into(),
        extra_length: "0.25".into(),
        box_type: "2".into(),
        dividers_x: "1".into(),
        dividers_y: "2".into(),
        divider_keying: "3".into(),
        optimize_layout: true,
        passes: "2".into(),
        power: "800".into(),
        feed_rate: "400".into(),
        z_step_down: "0.2".into(),
        offset_x: "12".into(),
        offset_y: "8".into(),
        home_before: true,
    }
}

/// Jigsaw: the page's defaults, and a larger puzzle with another seed, bigger tabs and more
/// jitter. The page imports the paths into the designer, the last one (the border) closed.
fn jigsaw_cases() -> Vec<(&'static str, String)> {
    let default = JigsawEntries::defaults(MeasurementSystem::Metric);
    [("default", default), ("custom", jigsaw_custom())]
        .into_iter()
        .map(|(name, entries)| {
            let params = jigsaw_parameters(&entries, MeasurementSystem::Metric);
            let mut maker = JigsawPuzzleMaker::new(params).expect("jigsaw parameters are valid");
            maker.generate().expect("jigsaw generates");
            (name, paths_text(&maker.paths_as_f64(), jigsaw_path_closed))
        })
        .collect()
}

/// Tabbed box G-code: the defaults, and an inside-dimension no-top box with dividers, a
/// bigger material, another joint and a homed start in imperial.
fn tabbed_box_cases() -> Vec<(&'static str, String)> {
    let default = (
        TabbedBoxEntries::defaults(MeasurementSystem::Metric),
        MeasurementSystem::Metric,
    );
    let custom = (
        TabbedBoxEntries {
            width: "5".into(),
            depth: "4".into(),
            height: "3".into(),
            outside: true,
            thickness: "0.2".into(),
            burn: "0.004".into(),
            finger_width: "1.5".into(),
            space_width: "2.5".into(),
            surrounding_spaces: "1".into(),
            play: "0.002".into(),
            extra_length: "0".into(),
            box_type: "1".into(),
            dividers_x: "1".into(),
            dividers_y: "2".into(),
            divider_keying: "1".into(),
            optimize_layout: true,
            passes: "2".into(),
            power: "800".into(),
            feed_rate: "400".into(),
            z_step_down: "0.01".into(),
            offset_x: "0.5".into(),
            offset_y: "0.5".into(),
            home_before: true,
        },
        MeasurementSystem::Imperial,
    );
    [("default", default), ("custom", custom)]
        .into_iter()
        .map(|(name, (entries, system))| {
            let params = tabbed_box_parameters(&entries, system, AXES);
            let mut maker = TabbedBoxMaker::new(params).expect("box parameters are valid");
            maker.generate().expect("box generates");
            (name, maker.to_gcode())
        })
        .collect()
}

/// The tabbed box's "Generate Shapes" outlines for the defaults, every one closed.
fn tabbed_box_shapes_cases() -> Vec<(&'static str, String)> {
    let params = tabbed_box_parameters(
        &TabbedBoxEntries::defaults(MeasurementSystem::Metric),
        MeasurementSystem::Metric,
        AXES,
    );
    let mut maker = TabbedBoxMaker::new(params).expect("box parameters are valid");
    maker.generate().expect("box generates");
    vec![("default", paths_text(&box_path_points(&maker), |_, _| true))]
}

/// The Gerber project folder the cases read.
fn gerber_dir() -> PathBuf {
    fixture_dir().join("gerber")
}

/// Gerber: the defaults on the top copper layer; a case with rubout against the board
/// outline, alignment holes, wider isolation and a bigger tool; and the board outline layer.
fn gerber_cases() -> Vec<(&'static str, String)> {
    let system = MeasurementSystem::Metric;
    let default = GerberEntries::defaults(system);
    let custom = GerberEntries {
        feed_rate: "300".into(),
        spindle_speed: "12000".into(),
        board_width: "40".into(),
        board_height: "30".into(),
        offset_x: "5".into(),
        offset_y: "5".into(),
        generate_alignment_holes: true,
        cut_depth: "-0.2".into(),
        safe_z: "3".into(),
        tool_diameter: "0.4".into(),
        isolation_width: "0.2".into(),
        rubout: true,
        use_board_outline: true,
        ..GerberEntries::defaults(system)
    };
    let outline = GerberEntries {
        layer_type: "BoardOutline".into(),
        ..GerberEntries::defaults(system)
    };
    let directory = Some(gerber_dir().to_string_lossy().to_string());
    [
        ("default", "board.gtl", default),
        ("custom", "board.gtl", custom),
        ("outline", "board.gko", outline),
    ]
    .into_iter()
    .map(|(name, file, entries)| {
        let params = gerber_parameters(&entries, system, directory.clone(), AXES);
        let content = std::fs::read_to_string(gerber_dir().join(file)).expect("a Gerber fixture");
        let gcode = GerberConverter::generate(&params, &content).expect("Gerber generates");
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

/// Presets as the GTK pages' Save writes them (metric entries): the jigsaw, tabbed box and
/// Gerber pages serialise their generator parameters with `serde_json::to_string_pretty`
/// (`save_params`; Gerber without a project folder); Drill Press serialises
/// `DrillPressParameters` with `serde_json::to_string_pretty` (`save_preset_json`, after the
/// same length and number parsing as `drill_press_parameters`); the spoilboard pages write a
/// `serde_json::json!` object of the lengths in millimetres as `f32` (`preset_length`) and the
/// other fields' text (`save_params` in `spoilboard_surfacing.rs` / `spoilboard_grid.rs`).
fn preset_cases() -> Vec<(&'static str, String)> {
    let mm = |text: &str| {
        gcodekit5_core::units::parse_length(text, MeasurementSystem::Metric).expect("a length")
    };
    let drill = DrillPressEntries {
        hole_diameter: "12.7".into(),
        tool_diameter: "3.175".into(),
        top_z: "1".into(),
        bottom_z: "-7.5".into(),
        peck_depth: "1.5".into(),
        safe_z: "4".into(),
        x: "42".into(),
        y: "17.25".into(),
        plunge_rate: "150".into(),
        feed_rate: "650".into(),
        spindle_speed: "9000".into(),
        home_before: true,
    };
    let drill = drill_press_parameters(&drill, MeasurementSystem::Metric).expect("entries parse");
    let surfacing = serde_json::json!({
        "width": mm("500"),
        "height": mm("250.5"),
        "tool_diameter": mm("19.05"),
        "feed_rate": "1500",
        "spindle_speed": "16000",
        "cut_depth": mm("0.25"),
        "stepover_percent": "60",
        "safe_z": mm("8"),
    });
    let grid = serde_json::json!({
        "width": mm("600"),
        "height": mm("400"),
        "grid_spacing": mm("25.4"),
        "feed_rate": "2000",
        "laser_power": "800",
        "laser_mode": "M3",
    });
    let system = MeasurementSystem::Metric;
    let puzzle = jigsaw_parameters(&jigsaw_custom(), system);
    let tabbed_box = tabbed_box_parameters(&tabbed_box_preset_entries(), system, AXES);
    let gerber = gerber_parameters(
        &GerberEntries {
            layer_type: "BottomCopper".into(),
            feed_rate: "250".into(),
            spindle_speed: "9000".into(),
            board_width: "80".into(),
            board_height: "60".into(),
            offset_x: "2".into(),
            offset_y: "3".into(),
            generate_alignment_holes: true,
            alignment_hole_diameter: "3".into(),
            alignment_hole_margin: "6".into(),
            alignment_hole_depth: "-2".into(),
            cut_depth: "-0.15".into(),
            safe_z: "4".into(),
            tool_diameter: "0.2".into(),
            isolation_width: "0.1".into(),
            rubout: true,
            use_board_outline: true,
        },
        system,
        None,
        AXES,
    );
    let pretty = |value: serde_json::Result<String>| value.expect("serialises");
    vec![
        ("jigsaw", pretty(serde_json::to_string_pretty(&puzzle))),
        (
            "tabbed_box",
            pretty(serde_json::to_string_pretty(&tabbed_box)),
        ),
        ("gerber", pretty(serde_json::to_string_pretty(&gerber))),
        ("drill_press", pretty(serde_json::to_string_pretty(&drill))),
        (
            "spoilboard_surfacing",
            pretty(serde_json::to_string_pretty(&surfacing)),
        ),
        (
            "spoilboard_grid",
            pretty(serde_json::to_string_pretty(&grid)),
        ),
    ]
}

/// The engraver's generation-time header line, as the fixtures hold it.
const GENERATED_MASK: &str = "; Generated: <time>";

/// `gcode` with the engraver's generation-time line replaced by [`GENERATED_MASK`].
fn mask_generated(gcode: &str) -> String {
    gcode
        .lines()
        .map(|line| {
            if line.starts_with("; Generated: ") {
                GENERATED_MASK
            } else {
                line
            }
        })
        .map(|line| format!("{line}\n"))
        .collect()
}

/// The bitmap the engraving cases read.
fn bitmap_image() -> PathBuf {
    fixture_dir().join("bitmap/gradient.png")
}

/// The bitmap engraving page with a small output (20 mm at 2 pixels per mm, the rest
/// GTK's defaults), and a case with every transformation: imperial, vertical one-way scans,
/// inverted, mirrored, rotated 90 degrees, Floyd-Steinberg, homing.
fn bitmap_entries() -> Vec<(&'static str, BitmapEngravingEntries, MeasurementSystem)> {
    let image = bitmap_image().to_string_lossy().to_string();
    let basic = BitmapEngravingEntries {
        image_path: image.clone(),
        width: "20".into(),
        pixels_per_mm: "2".into(),
        ..BitmapEngravingEntries::defaults(MeasurementSystem::Metric)
    };
    let custom = BitmapEngravingEntries {
        image_path: image,
        width: "1".into(),
        feed_rate: "1500".into(),
        travel_rate: "4000".into(),
        min_power: "5".into(),
        max_power: "80".into(),
        pixels_per_mm: "1.5".into(),
        line_spacing: "1.5".into(),
        power_scale: "255".into(),
        offset_x: "0.25".into(),
        offset_y: "0.5".into(),
        scan_direction: "1".into(),
        bidirectional: false,
        invert: true,
        mirror_x: true,
        mirror_y: true,
        rotation: "90".into(),
        halftone: "floyd".into(),
        halftone_dot_size: "3".into(),
        halftone_threshold: "100".into(),
        home_before: true,
    };
    vec![
        ("basic", basic, MeasurementSystem::Metric),
        ("custom", custom, MeasurementSystem::Imperial),
    ]
}

/// The bitmap engraving programs (generation time masked).
fn bitmap_cases() -> Vec<(&'static str, String)> {
    bitmap_entries()
        .into_iter()
        .map(|(name, entries, system)| {
            let params = bitmap_engraving_parameters(&entries, system, AXES);
            let gcode = BitmapImageEngraver::from_file(&entries.image_path, params)
                .and_then(|engraver| engraver.generate_gcode())
                .expect("the bitmap engraves");
            (name, mask_generated(&gcode))
        })
        .collect()
}

/// The probe page's start position for the cases (machine coordinates, mm).
fn probe_start() -> Position {
    Position::new(12.5, -30.25, -4.0)
}

/// The probe page's entries for each case: every routine at GTK's defaults, then Z-touch,
/// edge and corner with Auto-Update WCS and compensation.
fn probe_entries() -> Vec<(&'static str, ProbeEntries)> {
    let routine = |id: &str| ProbeEntries {
        routine: Some(id.to_string()),
        ..ProbeEntries::default()
    };
    vec![
        ("z_touch", routine("z_touch")),
        ("edge_find", routine("edge_find")),
        ("corner_find", routine("corner_find")),
        ("bore_center", routine("bore_center")),
        ("boss_center", routine("boss_center")),
        ("tool_length", routine("tool_length")),
        (
            "z_touch_wcs",
            ProbeEntries {
                wcs: Some("G55".into()),
                auto_update: true,
                plate_thickness: "1.5".into(),
                backoff: "1.25".into(),
                ..routine("z_touch")
            },
        ),
        (
            "edge_find_wcs",
            ProbeEntries {
                wcs: Some("G56".into()),
                auto_update: true,
                edge_axis: Some("Y".into()),
                edge_dir: Some("positive".into()),
                edge_dist: "15".into(),
                tip_radius: "1".into(),
                probe_depth: "4".into(),
                ..routine("edge_find")
            },
        ),
        (
            "corner_find_wcs",
            ProbeEntries {
                auto_update: true,
                corner: Some("xmax_ymax".into()),
                tip_radius: "0.5".into(),
                ..routine("corner_find")
            },
        ),
    ]
}

/// The probe page's Generate G-code programs.
fn probe_cases() -> Vec<(&'static str, String)> {
    probe_entries()
        .into_iter()
        .map(|(name, entries)| {
            let routine = probe_routine(&entries).expect("a routine");
            let options = probe_options(&entries).expect("options");
            let output = ProbeRoutineEngine::with_options(routine, options)
                .generate(probe_start())
                .expect("the routine generates");
            (name, output.gcode)
        })
        .collect()
}

/// The bitmap engraving and probe presets as the GTK pages' Save writes them
/// (`collect_preset` then `to_json`).
fn bitmap_and_probe_preset_cases() -> Vec<(&'static str, String)> {
    let bitmap = BitmapEngravingEntries {
        image_path: "gradient.png".into(),
        ..bitmap_entries()
            .into_iter()
            .map(|(_, entries, _)| entries)
            .next()
            .expect("a bitmap case")
    };
    let bitmap = bitmap_engraving_preset(&bitmap, MeasurementSystem::Metric)
        .expect("entries parse")
        .to_json()
        .expect("serialises");
    let probe = probe_entries()
        .into_iter()
        .map(|(_, entries)| entries)
        .nth(7)
        .expect("the edge case");
    let probe = probe_preset(&probe)
        .and_then(|p| p.to_json())
        .expect("serialises");
    vec![("bitmap_engraving", bitmap), ("probe", probe)]
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
        named("jigsaw", "txt", jigsaw_cases()),
        named("tabbed_box", "gcode", tabbed_box_cases()),
        named("tabbed_box_shapes", "txt", tabbed_box_shapes_cases()),
        named("gerber", "gcode", gerber_cases()),
        named("preset", "json", preset_cases()),
        named("bitmap_engraving", "gcode", bitmap_cases()),
        named("probe", "gcode", probe_cases()),
        named("preset", "json", bitmap_and_probe_preset_cases()),
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
