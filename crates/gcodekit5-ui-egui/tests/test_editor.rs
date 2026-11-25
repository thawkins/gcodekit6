//! The G-code editor: its buffer (open, save, dirty flag), syntax highlighting, find, the
//! line that follows a streaming job, the large-file view, and the app's File commands, the
//! startup files and the program the editor hands to the machine.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use craft_dialogs::{Answer, Request};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::MachineEvent;
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::commands::Command;
use gcodekit5_ui_egui::panes::editor::{
    EditorBuffer, EditorPane, LARGE_FILE_LINES, buffer::stream_line_map, find, highlight,
};
use gcodekit5_ui_egui::theme;
use gcodekit5_ui_egui::workspace::{Pane, PaneContext};
use gcodekit5_ui_egui::{AppState, GcodeKitApp};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A fresh scratch folder `target/temp/egui_editor/<name>-<pid>`.
fn scratch(name: &str) -> Result<PathBuf, std::io::Error> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/egui_editor")
        .join(format!("{name}-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Writes `text` to `dir/name` and returns the path.
fn write(dir: &Path, name: &str, text: &str) -> Result<PathBuf, std::io::Error> {
    let path = dir.join(name);
    std::fs::write(&path, text)?;
    Ok(path)
}

/// The pane in a harness of `size`, drawn with an English translator.
fn pane_harness(pane: EditorPane, size: egui::Vec2) -> Harness<'static, EditorPane> {
    let tr = Translator::new("en");
    Harness::builder().with_size(size).build_ui_state(
        move |ui, pane: &mut EditorPane| {
            let mut requested = Vec::new();
            let mut ctx = PaneContext::new(&tr, &mut requested);
            pane.ui(ui, &mut ctx);
        },
        pane,
    )
}

/// The app with default settings in a harness that runs frames as eframe does.
fn app_harness() -> Harness<'static, GcodeKitApp> {
    let state = AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    );
    crate::machine_support::app_harness(GcodeKitApp::from_state(state))
}

/// Answers every file dialog with `path` (or Cancel when `None`), recording what was asked.
fn answer_files(
    app: &mut GcodeKitApp,
    path: Option<PathBuf>,
) -> std::sync::Arc<std::sync::Mutex<Vec<String>>> {
    let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = std::sync::Arc::clone(&asked);
    app.dialogs_mut()
        .set_override(Some(Box::new(move |request: &Request| {
            let title = match request {
                Request::File(file) => format!("file:{}", file.title.clone().unwrap_or_default()),
                Request::Message(message) => format!("message:{}", message.title),
            };
            if let Ok(mut seen) = seen.lock() {
                seen.push(title);
            }
            match (request, &path) {
                (Request::File(_), Some(path)) => Answer::Paths(vec![path.clone()]),
                (Request::File(_) | Request::Message(_), _) => Answer::Cancelled,
            }
        })));
    asked
}

/// One app frame followed by the end-of-frame dialogs, as eframe runs them.
fn app_frame(harness: &mut Harness<'_, GcodeKitApp>) {
    harness.step();
    harness
        .state_mut()
        .dialogs_mut()
        .end_frame(None::<&eframe::Frame>);
}

#[test]
fn test_open_save_round_trip() -> TestResult {
    let dir = scratch("round_trip")?;
    let original = "G21\nG0 X0 Y0 ; home\nG1 X10 F100\n";
    let path = write(&dir, "part.gcode", original)?;
    let mut buffer = EditorBuffer::load(&path)?;
    assert_eq!(buffer.text(), original);
    assert_eq!(buffer.path(), Some(path.as_path()));
    assert_eq!(buffer.line_count(), 4);
    assert_eq!(buffer.line(1), "G0 X0 Y0 ; home");

    buffer.set_text("G21\nG1 X20 F200\n".to_string());
    buffer.save()?;
    assert_eq!(std::fs::read_to_string(&path)?, "G21\nG1 X20 F200\n");
    assert_eq!(EditorBuffer::load(&path)?.text(), "G21\nG1 X20 F200\n");

    let other = dir.join("copy.nc");
    buffer.save_as(&other)?;
    assert_eq!(buffer.path(), Some(other.as_path()));
    assert_eq!(std::fs::read_to_string(&other)?, "G21\nG1 X20 F200\n");

    // A buffer that was never saved has nowhere to save to.
    assert!(EditorBuffer::default().save().is_err());
    // A missing file is an error, not an empty buffer.
    assert!(EditorBuffer::load(&dir.join("missing.gcode")).is_err());
    Ok(())
}

#[test]
fn test_dirty_flag() -> TestResult {
    let dir = scratch("dirty")?;
    let path = write(&dir, "part.gcode", "G0 X0\n")?;
    let mut buffer = EditorBuffer::load(&path)?;
    assert!(!buffer.is_dirty());
    buffer.set_text("G0 X1\n".to_string());
    assert!(buffer.is_dirty());
    buffer.save()?;
    assert!(!buffer.is_dirty());

    // Typing in the pane marks it dirty.
    let mut pane = EditorPane::new();
    pane.load(&path)?;
    let mut harness = pane_harness(pane, egui::vec2(800.0, 600.0));
    harness.step();
    harness.get_by_label("G-Code Editor").focus();
    harness.step();
    harness.get_by_label("G-Code Editor").type_text("M3 ");
    harness.step();
    assert!(harness.state().buffer().is_dirty());
    assert!(harness.state().buffer().text().contains("M3 "));
    Ok(())
}

#[test]
fn test_dirty_flag_reaches_app_state() -> TestResult {
    let dir = scratch("dirty_app")?;
    let path = write(&dir, "part.gcode", "G0 X0\n")?;
    let mut harness = app_harness();
    harness.state_mut().open_gcode(&path);
    app_frame(&mut harness);
    assert!(!harness.state().state().dirty.editor);
    harness
        .state()
        .editor()
        .borrow_mut()
        .buffer_mut()
        .set_text("G0 X5\n".to_string());
    app_frame(&mut harness);
    assert!(harness.state().state().dirty.editor);
    Ok(())
}

#[test]
fn test_highlight_classes() {
    let style = highlight::HighlightStyle::new(
        egui::FontId::monospace(13.0),
        theme::palette(egui::Theme::Dark).syntax,
    );
    let job = highlight::highlight("G1 X10 ; cut", &style);
    let spans: Vec<(&str, egui::Color32)> = job
        .sections
        .iter()
        .map(|s| {
            (
                &job.text[s.byte_range.start.0..s.byte_range.end.0],
                s.format.color,
            )
        })
        .collect();
    let syntax = style.syntax;
    assert_eq!(
        spans,
        vec![
            ("G1", syntax.command),
            (" ", syntax.plain),
            ("X10", syntax.coordinate),
            (" ", syntax.plain),
            ("; cut", syntax.comment),
        ]
    );
    // Four classes, four colours.
    let mut colours: Vec<_> = spans.iter().map(|(_, c)| *c).collect();
    colours.sort_by_key(|c| c.to_array());
    colours.dedup();
    assert_eq!(colours.len(), 4);

    let job = highlight::highlight("N10 M3 S1000 (spindle)", &style);
    let spans: Vec<(&str, egui::Color32)> = job
        .sections
        .iter()
        .map(|s| {
            (
                &job.text[s.byte_range.start.0..s.byte_range.end.0],
                s.format.color,
            )
        })
        .collect();
    assert_eq!(
        spans,
        vec![
            ("N10", syntax.line_number),
            (" ", syntax.plain),
            ("M3", syntax.command),
            (" S1000 ", syntax.plain),
            ("(spindle)", syntax.comment),
        ]
    );
    // Light and dark differ.
    assert_ne!(
        theme::palette(egui::Theme::Light).syntax,
        theme::palette(egui::Theme::Dark).syntax
    );
}

#[test]
fn test_find_next() -> TestResult {
    let text = "G0 X0\nG1 x10\nG1 Y5\nG1 X10\n";
    let first = find::find_next(text, "X10", 0);
    assert_eq!(first, Some(9..12), "case-insensitive");
    assert_eq!(find::find_next(text, "X10", 12), Some(22..25));
    assert_eq!(find::find_next(text, "X10", 25), Some(9..12), "wraps");
    assert_eq!(find::find_previous(text, "X10", 22), Some(9..12));
    assert_eq!(find::find_previous(text, "X10", 9), Some(22..25), "wraps");
    assert_eq!(find::find_next(text, "Z", 0), None);
    assert_eq!(find::find_next(text, "", 0), None);
    assert_eq!(find::count(text, "g1"), 3);

    // The pane moves its current line to each match.
    let dir = scratch("find")?;
    let path = write(&dir, "part.gcode", text)?;
    let mut pane = EditorPane::new();
    pane.load(&path)?;
    pane.set_find_query("x10");
    pane.find_next();
    assert_eq!(pane.buffer().current_line(), 1);
    pane.find_next();
    assert_eq!(pane.buffer().current_line(), 3);
    pane.find_next();
    assert_eq!(pane.buffer().current_line(), 1);
    pane.find_previous();
    assert_eq!(pane.buffer().current_line(), 3);
    Ok(())
}

#[test]
fn test_current_line_follows_stream() -> TestResult {
    // Every third line is a comment or blank, which the stream drops, so streamed line
    // numbers and editor line numbers differ.
    let text: String = (0..2000)
        .map(|i| match i % 3 {
            0 => format!("; step {i}\n"),
            1 => format!("G1 X{i} F100 (move)\n"),
            _ => format!("G1 Y{i}\n"),
        })
        .collect();
    let map = stream_line_map(&text);
    let prepared = gcodekit5_app::machine::grbl_stream::prepare_program(&text, false, 0)?;
    assert_eq!(map.len(), prepared.lines().count());

    let dir = scratch("follow")?;
    let path = write(&dir, "part.gcode", &text)?;
    let mut pane = EditorPane::new();
    pane.load(&path)?;
    pane.program_fed();
    let tr = Translator::new("en");
    // Streamed line 500 (1-based) is in progress: 499 lines acknowledged.
    pane.machine_events(
        &[MachineEvent::StreamProgress {
            sent: 510,
            acked: 499,
            total: map.len(),
        }],
        &tr,
    );
    let expected = map[499];
    assert_eq!(pane.buffer().current_line(), expected);
    assert!(expected > 600, "the mapping skips the dropped lines");

    let mut harness = pane_harness(pane, egui::vec2(800.0, 600.0));
    harness.step();
    harness.step();
    let visible = harness.state().visible_lines();
    assert!(
        visible.contains(&expected),
        "line {expected} not in view {visible:?}"
    );
    Ok(())
}

#[test]
fn test_large_file_is_read_only_until_edit_anyway() -> TestResult {
    let dir = scratch("large_mode")?;
    let text: String = (0..LARGE_FILE_LINES + 10)
        .map(|i| format!("G1 X{i}\n"))
        .collect();
    let path = write(&dir, "big.gcode", &text)?;
    let mut pane = EditorPane::new();
    pane.load(&path)?;
    assert!(pane.is_large());
    let mut harness = pane_harness(pane, egui::vec2(800.0, 600.0));
    harness.step();
    assert!(
        harness.query_by_label("G-Code Editor").is_none(),
        "read-only view has no text field"
    );
    harness.get_by_label("Edit anyway").click();
    harness.step();
    harness.step();
    assert!(harness.query_by_label("G-Code Editor").is_some());
    Ok(())
}

/// Review Focus 2: a 1,000,000-line file draws a frame in under 50 ms. Timing is only
/// meaningful in an optimised build, so the test is ignored in debug builds; run it with
/// `cargo nextest run --release -p gcodekit5-ui-egui -E 'test(test_large_file_frame_time)'`.
#[test]
#[cfg_attr(debug_assertions, ignore = "timing needs a release build")]
fn test_large_file_frame_time() -> TestResult {
    let dir = scratch("large_frame")?;
    let text: String = (0..1_000_000)
        .map(|i| {
            format!(
                "N{i} G1 X{}.{} Y{} F1200 ; pass\n",
                i % 300,
                i % 10,
                i % 200
            )
        })
        .collect();
    let path = write(&dir, "huge.gcode", &text)?;
    let mut pane = EditorPane::new();
    pane.load(&path)?;
    pane.program_fed();
    let mut harness = pane_harness(pane, egui::vec2(1280.0, 800.0));
    // Warm-up: fonts and the first layout.
    harness.step();
    let tr = Translator::new("en");
    harness.state_mut().machine_events(
        &[MachineEvent::StreamProgress {
            sent: 500_010,
            acked: 500_000,
            total: 1_000_000,
        }],
        &tr,
    );
    let mut worst = Duration::ZERO;
    for _ in 0..5 {
        let start = Instant::now();
        harness.step();
        worst = worst.max(start.elapsed());
    }
    assert!(
        harness.state().visible_lines().contains(&500_000),
        "{:?}",
        harness.state().visible_lines()
    );
    assert!(worst < Duration::from_millis(50), "slowest frame {worst:?}");
    Ok(())
}

#[test]
fn test_file_open_command_loads_editor_and_program() -> TestResult {
    let dir = scratch("open_command")?;
    let path = write(&dir, "part.gcode", "; header\nG21\nG1 X10 F100\n")?;
    let mut harness = app_harness();
    let asked = answer_files(harness.state_mut(), Some(path.clone()));
    harness.state_mut().dispatch(Command::FileOpen);
    app_frame(&mut harness);
    app_frame(&mut harness);
    app_frame(&mut harness);
    assert_eq!(
        asked.lock().map(|a| a.clone()).unwrap_or_default(),
        ["file:Open G-Code File"]
    );
    let app = harness.state();
    assert_eq!(app.editor().borrow().buffer().path(), Some(path.as_path()));
    let program: Vec<String> = app
        .state()
        .machine
        .program()
        .map(|p| p.to_vec())
        .unwrap_or_default();
    assert_eq!(program, ["G21", "G1 X10 F100"]);
    Ok(())
}

#[test]
fn test_file_save_writes_to_path_or_asks() -> TestResult {
    let dir = scratch("save_command")?;
    let path = write(&dir, "part.gcode", "G0 X0\n")?;
    let mut harness = app_harness();
    // No path yet: Save asks where.
    let target = dir.join("new.gcode");
    let asked = answer_files(harness.state_mut(), Some(target.clone()));
    harness
        .state()
        .editor()
        .borrow_mut()
        .buffer_mut()
        .set_text("G1 X1\n".to_string());
    harness.state_mut().dispatch(Command::FileSave);
    app_frame(&mut harness);
    app_frame(&mut harness);
    assert_eq!(
        asked.lock().map(|a| a.clone()).unwrap_or_default(),
        ["file:Save G-Code File"]
    );
    assert_eq!(std::fs::read_to_string(&target)?, "G1 X1\n");
    assert!(!harness.state().state().dirty.editor);

    // With a path: Save writes without asking.
    harness.state_mut().open_gcode(&path);
    harness
        .state()
        .editor()
        .borrow_mut()
        .buffer_mut()
        .set_text("G0 X9\n".to_string());
    let asked = answer_files(harness.state_mut(), None);
    harness.state_mut().dispatch(Command::FileSave);
    app_frame(&mut harness);
    assert!(asked.lock().map(|a| a.is_empty()).unwrap_or(false));
    assert_eq!(std::fs::read_to_string(&path)?, "G0 X9\n");
    Ok(())
}

#[test]
fn test_launch_files_open_in_editor() -> TestResult {
    let dir = scratch("launch")?;
    let path = write(&dir, "start.nc", "G0 X1\n")?;
    let mut app = GcodeKitApp::from_state(AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    ));
    app.open_startup_files(std::slice::from_ref(&path));
    assert_eq!(app.editor().borrow().buffer().text(), "G0 X1\n");
    Ok(())
}
