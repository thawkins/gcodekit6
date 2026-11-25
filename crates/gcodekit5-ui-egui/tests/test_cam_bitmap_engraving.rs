//! The Laser Image Engraver page: GTK's defaults render and wait for an image, Browse picks
//! the image through the non-blocking dialog and shows it, invalid input is shown at the
//! field and disables the actions, the program is exactly what the GTK page made from the
//! same small image (fixtures from `capture_cam_fixtures`, generation time masked), the
//! engraving runs off the UI thread and can be cancelled, and presets in GTK's file format
//! load and save.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_camtools::laser_engraver::{HalftoneMethod, RotationAngle, ScanDirection};
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_ui_egui::GcodeKitApp;
use gcodekit5_ui_egui::panes::cam_tools::bitmap_engraving::BitmapEngravingParams;
use gcodekit5_ui_egui::panes::cam_tools::{CamTool, Category};

use crate::test_cam_drill_press::{click, fixture, open_tool_tall, pane};
use crate::test_cam_tools::cam_app;
use crate::test_designer_files_app::{answer, default_state, frames, path, shown};

const METRIC: MeasurementSystem = MeasurementSystem::Metric;

/// The image the fixtures were engraved from.
fn image() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cam/bitmap/gradient.png")
}

/// `gcode` with the engraver's generation-time line masked as the fixtures hold it.
fn masked(gcode: &str) -> String {
    gcode
        .lines()
        .map(|line| {
            if line.starts_with("; Generated: ") {
                "; Generated: <time>"
            } else {
                line
            }
        })
        .map(|line| format!("{line}\n"))
        .collect()
}

/// The `bitmap_engraving_basic` case: GTK's defaults, 20 mm at 2 pixels per mm.
fn basic() -> BitmapEngravingParams {
    let mut p = BitmapEngravingParams::new(METRIC);
    p.set_image(Some(image()));
    p.width = "20".into();
    p.pixels_per_mm = "2".into();
    p
}

/// The `bitmap_engraving_custom` case: every transformation, imperial.
fn custom() -> BitmapEngravingParams {
    let mut p = BitmapEngravingParams::new(MeasurementSystem::Imperial);
    p.set_image(Some(image()));
    p.width = "1".into();
    p.feed_rate = "1500".into();
    p.travel_rate = "4000".into();
    p.min_power = "5".into();
    p.max_power = "80".into();
    p.pixels_per_mm = "1.5".into();
    p.line_spacing = "1.5".into();
    p.power_scale = "255".into();
    p.offset_x = "0.25".into();
    p.offset_y = "0.5".into();
    p.scan_direction = ScanDirection::Vertical;
    p.bidirectional = false;
    p.invert = true;
    p.mirror_x = true;
    p.mirror_y = true;
    p.rotation = RotationAngle::Degrees90;
    p.halftone = HalftoneMethod::FloydSteinberg;
    p.halftone_dot_size = "3".into();
    p.halftone_threshold = "100".into();
    p.home_before = true;
    p
}

/// Steps the app until `cond` holds on the CAM pane, or fails after 30 s.
fn wait_for(
    h: &mut Harness<'static, GcodeKitApp>,
    what: &str,
    cond: impl Fn(&mut gcodekit5_ui_egui::panes::cam_tools::CamToolsPane) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        frames(h, 1);
        if cond(&mut pane(h)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("timed out waiting for {what}");
}

/// The page open in a tall window with the `basic` input.
fn open_basic() -> Harness<'static, GcodeKitApp> {
    let mut h = open_tool_tall(CamTool::BitmapEngraving);
    *pane(&h).bitmap_engraving_mut() = basic();
    frames(&mut h, 2);
    h
}

#[test]
fn test_cam_bitmap_default_page_renders_and_waits_for_an_image() {
    let h = open_tool_tall(CamTool::BitmapEngraving);
    for label in [
        "Image Path:",
        "Browse...",
        "Width:",
        "Feed Rate:",
        "Travel Rate:",
        "Min Power (%):",
        "Max Power (%):",
        "Power Scale (S):",
        "Scan Direction:",
        "Pixels per mm:",
        "Line Spacing:",
        "Bidirectional:",
        "Invert:",
        "Mirror X:",
        "Mirror Y:",
        "Rotation:",
        "Method:",
        "Dot Size:",
        "Threshold:",
        "Offset X:",
        "Offset Y:",
        "Home Device Before Start",
        "Generate",
        "Send to Editor",
        "Preview",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label} not shown");
    }
    assert!(
        h.query_by_label("Please select an image file first.")
            .is_some()
    );
    let mut cam = pane(&h);
    let p = cam.bitmap_engraving_mut();
    assert_eq!(p.image(), None);
    assert_eq!(p.width, "100.000");
    assert_eq!(p.feed_rate, "1000");
    assert_eq!(p.travel_rate, "3000");
    assert_eq!(p.pixels_per_mm, "10");
    assert_eq!(p.line_spacing, "1.0");
    assert_eq!(p.power_scale, "1000");
    assert_eq!(p.offset_x, "10.000");
    assert!(p.bidirectional);
    assert_eq!(p.halftone, HalftoneMethod::None);
    // Only the image is missing.
    assert_eq!(p.parameters().map(|_| ()).err().map(|e| e.len()), Some(1));
}

#[test]
fn test_cam_bitmap_is_listed_under_engraving() {
    let (h, _script) = cam_app(default_state());
    let tr = h.state().state().translator;
    let mut cam = pane(&h);
    cam.set_category(Category::Engraving);
    assert_eq!(cam.listed_tools(&tr), [CamTool::BitmapEngraving]);
    cam.set_category(Category::All);
    let all = cam.listed_tools(&tr);
    let at = all.iter().position(|t| *t == CamTool::BitmapEngraving);
    // GTK's order: after the tabbed box, before the calculator.
    assert_eq!(at, Some(2), "{all:?}");
}

#[test]
fn test_cam_bitmap_engraving_matches_gtk() {
    let engraved = |p: BitmapEngravingParams| p.engrave().map(|g| masked(&g));
    assert_eq!(
        engraved(basic()),
        Ok(fixture("bitmap_engraving_basic.gcode"))
    );
    assert_eq!(
        engraved(custom()),
        Ok(fixture("bitmap_engraving_custom.gcode"))
    );
}

#[test]
fn test_cam_bitmap_generate_runs_off_the_ui_thread_and_matches_gtk() {
    let mut h = open_basic();
    click(&mut h, "Generate");
    wait_for(&mut h, "the engraving", |cam| cam.output().is_some());
    let output = pane(&h).output().map(masked);
    assert_eq!(output, Some(fixture("bitmap_engraving_basic.gcode")));
    click(&mut h, "Send to Editor");
    wait_for(&mut h, "the editor", |_| true);
    let deadline = Instant::now() + Duration::from_secs(30);
    while h.state().editor().borrow().buffer().text().is_empty() && Instant::now() < deadline {
        frames(&mut h, 1);
    }
    let editor = h.state().editor().borrow().buffer().text().to_string();
    assert_eq!(masked(&editor), fixture("bitmap_engraving_basic.gcode"));
}

#[test]
fn test_cam_bitmap_cancel_discards_the_engraving() {
    let mut h = open_basic();
    {
        let mut cam = pane(&h);
        let p = cam.bitmap_engraving_mut();
        // 4000 x 2666 pixels: long enough to still be running a frame later.
        p.width = "400".into();
        p.pixels_per_mm = "10".into();
    }
    frames(&mut h, 2);
    h.get_by_label("Generate").click();
    frames(&mut h, 1);
    assert!(pane(&h).bitmap_job_running(), "Generate started no job");
    // The progress row is drawn from the next frame on.
    frames(&mut h, 1);
    assert!(pane(&h).bitmap_job_running(), "the job ended too soon");
    assert!(
        h.query_by_label("Cancel").is_some(),
        "no Cancel while running"
    );
    h.get_by_label("Cancel").click();
    frames(&mut h, 2);
    assert!(!pane(&h).bitmap_job_running());
    // Whenever the worker finishes, its program is dropped.
    frames(&mut h, 5);
    assert_eq!(pane(&h).output(), None);
    assert!(h.state().editor().borrow().buffer().text().is_empty());
}

#[test]
fn test_cam_bitmap_browse_picks_the_image_and_shows_it() {
    let (mut h, script) = cam_app(default_state());
    let title = h
        .state()
        .state()
        .translator
        .tr(CamTool::BitmapEngraving.title_msgid())
        .into_owned();
    h.get_by_label(&title).click();
    frames(&mut h, 1);
    h.get_by_label("Open").click();
    frames(&mut h, 2);
    h.set_size(egui::vec2(1280.0, 2400.0));
    frames(&mut h, 2);
    answer(&script, [path(&image())]);
    click(&mut h, "Browse...");
    assert_eq!(shown(&script), ["Select Image"]);
    assert_eq!(
        pane(&h).bitmap_engraving_mut().image(),
        Some(image().as_path())
    );
    wait_for(&mut h, "the preview", |cam| {
        cam.bitmap_preview_size() == Some([24, 16])
    });
    assert!(
        h.query_by_label("Please select an image file first.")
            .is_none()
    );
}

#[test]
fn test_cam_bitmap_invalid_input_shows_error_and_disables_the_actions() {
    type Breaker = fn(&mut BitmapEngravingParams);
    let cases: [(Breaker, &str); 14] = [
        (|p| p.set_image(None), "Please select an image file first."),
        (|p| p.width = "0".into(), "Must be greater than 0"),
        (|p| p.width = "wide".into(), "Not a valid length"),
        (|p| p.feed_rate = "0".into(), "Must be greater than 0"),
        (|p| p.travel_rate = "-1".into(), "Must be greater than 0"),
        (|p| p.min_power = "-1".into(), "Must be between 0 and 100"),
        (|p| p.max_power = "101".into(), "Must be between 0 and 100"),
        (
            |p| {
                p.min_power = "60".into();
                p.max_power = "40".into();
            },
            "Must not be more than the maximum power",
        ),
        (|p| p.pixels_per_mm = "0".into(), "Must be greater than 0"),
        (|p| p.line_spacing = "0".into(), "Must be greater than 0"),
        (|p| p.power_scale = "0".into(), "Must be greater than 0"),
        (|p| p.halftone_dot_size = "0".into(), "Must be at least 1"),
        (
            |p| p.halftone_threshold = "300".into(),
            "Must be between 0 and 255",
        ),
        (
            |p| {
                p.width = "2000".into();
                p.pixels_per_mm = "10".into();
            },
            "The engraving is too large: at most 10000 pixels per line",
        ),
    ];
    // Control: the valid page's Generate makes the program.
    let mut h = open_basic();
    click(&mut h, "Generate");
    wait_for(&mut h, "the control engraving", |cam| {
        cam.output().is_some()
    });
    for (break_it, message) in cases {
        let mut h = open_basic();
        break_it(pane(&h).bitmap_engraving_mut());
        frames(&mut h, 2);
        assert!(h.query_by_label(message).is_some(), "{message} not shown");
        for button in ["Generate", "Send to Editor", "Preview"] {
            click(&mut h, button);
            assert!(!pane(&h).bitmap_job_running(), "{message}: {button} ran");
            assert_eq!(
                pane(&h).output(),
                None,
                "{message}: {button} made a program"
            );
        }
    }
}

#[test]
fn test_cam_bitmap_follows_the_measurement_system() {
    let mut p = BitmapEngravingParams::new(METRIC);
    p.set_units(MeasurementSystem::Imperial);
    assert_eq!(p.width, "3.937");
    assert_eq!(p.offset_x, "0.394");
    p.set_units(METRIC);
    assert_eq!(p.width, "100.000");
}

#[test]
fn test_cam_bitmap_preset_loads_the_gtk_file_and_round_trips() {
    let gtk = fixture("preset_bitmap_engraving.json");
    let mut p = BitmapEngravingParams::new(METRIC);
    assert_eq!(p.apply_preset(&gtk), Ok(()));
    let mut expected = basic();
    expected.set_image(Some(PathBuf::from("gradient.png")));
    assert_eq!(p.width, expected.width.clone() + ".000");
    assert_eq!(p.pixels_per_mm, expected.pixels_per_mm);
    assert_eq!(p.image(), Some(Path::new("gradient.png")));
    let tr = gcodekit5_app::i18n::Translator::new("en");
    assert_eq!(p.preset_json(&tr), Ok(gtk));
    // A file that is not a preset leaves the page as it was.
    let before = format!("{p:?}");
    assert!(p.apply_preset("[1, 2]").is_err());
    assert_eq!(format!("{p:?}"), before);
}
