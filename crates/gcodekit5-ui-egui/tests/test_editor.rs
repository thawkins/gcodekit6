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

/// Build time of the editable view's frame after a keystroke, for a file one line under
/// [`LARGE_FILE_LINES`]: the worst of the frames that re-layout the whole text.
fn near_threshold_keystroke_frame() -> Result<Duration, Box<dyn std::error::Error>> {
    let text: String = (0..LARGE_FILE_LINES - 1)
        .map(|i| {
            format!(
                "N{i} G1 X{}.{} Y{} F1200 ; pass\n",
                i % 300,
                i % 10,
                i % 200
            )
        })
        .collect();
    let mut pane = EditorPane::new();
    pane.buffer_mut().set_text(text);
    assert!(!pane.is_large(), "{} lines", pane.buffer().line_count());
    let mut harness = pane_harness(pane, egui::vec2(1280.0, 800.0));
    // Warm-up: fonts and the first layout.
    harness.step();
    harness.get_by_label("G-Code Editor").focus();
    harness.step();
    let mut worst = Duration::ZERO;
    for _ in 0..3 {
        // The keystroke changes the text, so the next frame re-lays out all of it.
        harness.get_by_label("G-Code Editor").type_text("M");
        let start = Instant::now();
        harness.step();
        worst = worst.max(start.elapsed());
    }
    assert!(harness.state().buffer().text().contains('M'));
    Ok(worst)
}

/// A frame after a keystroke in a 49,999-line file (the editable path's worst case) stays
/// within [`EDIT_FRAME_BUDGET`].
#[test]
#[ignore = "50k-line file; CI runs it with --run-ignored only"]
fn test_near_threshold_edit_frame_time() -> TestResult {
    let worst = near_threshold_keystroke_frame()?;
    assert!(
        worst < EDIT_FRAME_BUDGET,
        "slowest edit frame {worst:?}, budget {EDIT_FRAME_BUDGET:?}"
    );
    Ok(())
}

/// A click in the large-file view's gutter makes that line current.
#[test]
fn test_large_file_gutter_click_sets_current_line() -> TestResult {
    let text: String = (0..LARGE_FILE_LINES + 10)
        .map(|i| format!("G1 X{}\n", i % 300))
        .collect();
    let mut pane = EditorPane::new();
    pane.buffer_mut().set_text(text);
    assert!(pane.is_large());
    let mut harness = pane_harness(pane, egui::vec2(800.0, 600.0));
    harness.run();
    let visible = harness.state().visible_lines();
    // Clicks in the gutter column at two heights select the lines under them, in order.
    let mut selected = Vec::new();
    for y in [200.0_f32, 320.0] {
        let pos = egui::pos2(15.0, y);
        harness.event(egui::Event::PointerMoved(pos));
        harness.step();
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.step();
        }
        harness.step();
        selected.push(harness.state().buffer().current_line());
    }
    assert!(
        selected[0] > 0 && selected[0] < selected[1] && visible.contains(&selected[1]),
        "{selected:?} in {visible:?}"
    );
    Ok(())
}

#[test]
fn test_save_as_writes_atomically_and_leaves_no_temp_file() -> TestResult {
    let dir = scratch("atomic_save")?;
    let path = write(&dir, "part.gcode", "G0 X0\n")?;
    let mode_before = std::fs::metadata(&path)?.permissions();
    let mut buffer = EditorBuffer::load(&path)?;
    buffer.set_text("G1 X5 F100\nM2\n".to_string());
    buffer.save()?;
    assert_eq!(std::fs::read_to_string(&path)?, "G1 X5 F100\nM2\n");
    assert_eq!(std::fs::metadata(&path)?.permissions(), mode_before);
    assert!(!buffer.is_dirty());
    // Save as a new name: that file holds the text, the old one is untouched.
    let other = dir.join("other.gcode");
    buffer.save_as(&other)?;
    assert_eq!(std::fs::read_to_string(&other)?, "G1 X5 F100\nM2\n");
    assert_eq!(buffer.path(), Some(other.as_path()));
    let names: Vec<String> = std::fs::read_dir(&dir)?
        .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, _>>()?;
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        ["other.gcode", "part.gcode"],
        "stray files: {names:?}"
    );
    // A directory that does not exist is an error, not a panic, and leaves nothing behind.
    assert!(
        buffer
            .save_as(&dir.join("missing").join("x.gcode"))
            .is_err()
    );
    Ok(())
}

/// An app whose machine is streaming a held job, with `text` in the editor.
fn app_with_running_job(text: &str) -> GcodeKitApp {
    let (mut state, shared) = crate::machine_support::loopback_state(Duration::from_millis(1));
    crate::machine_support::start_held_stream(&mut state, &shared, 5);
    let app = GcodeKitApp::from_state(state);
    app.editor()
        .borrow_mut()
        .buffer_mut()
        .set_text(text.to_string());
    app
}

#[test]
fn test_save_refuses_read_only_file_and_keeps_content() -> TestResult {
    let dir = scratch("read_only_save")?;
    let path = write(&dir, "locked.gcode", "G0 X0\n")?;
    let mut buffer = EditorBuffer::load(&path)?;
    buffer.set_text("G1 X9\n".to_string());
    let original = std::fs::metadata(&path)?.permissions();
    let mut locked = original.clone();
    locked.set_readonly(true);
    std::fs::set_permissions(&path, locked)?;
    let result = buffer.save();
    // Restore the original mode before asserting, so the scratch dir can be cleaned up.
    std::fs::set_permissions(&path, original)?;
    assert!(result.is_err(), "save over a read-only file must fail");
    assert_eq!(std::fs::read_to_string(&path)?, "G0 X0\n");
    Ok(())
}

#[test]
fn test_open_is_refused_while_a_job_runs() -> TestResult {
    let dir = scratch("open_job")?;
    let path = write(&dir, "other.gcode", "G1 X99 F100\n")?;
    let mut app = app_with_running_job("G1 X1 F100\n");
    assert!(app.state().job_running());
    let asked = answer_files(&mut app, Some(path.clone()));
    app.dispatch(Command::FileOpen);
    app.open_gcode(&path);
    assert!(
        asked.lock().map(|a| a.is_empty()).unwrap_or(false),
        "no dialog"
    );
    let editor = app.editor().borrow();
    assert_eq!(editor.buffer().text(), "G1 X1 F100\n", "editor untouched");
    assert_eq!(
        editor.notice(),
        Some("Stop the running job before opening a file.")
    );
    Ok(())
}

#[test]
fn test_stream_start_not_routed_when_editor_sync_cannot_run() {
    let (mut state, _shared) = crate::machine_support::loopback_state(Duration::from_millis(1));
    crate::machine_support::connect(&mut state);
    // The machine holds program A; the editor shows a newer program B.
    state.machine.set_program("G1 X1 F100\n").expect("program");
    let mut app = GcodeKitApp::from_state(state);
    app.editor()
        .borrow_mut()
        .buffer_mut()
        .set_text("G1 X2 F100\n".to_string());
    let editor = std::rc::Rc::clone(app.editor());
    let held = editor.borrow_mut();
    app.dispatch(Command::StreamStart);
    drop(held);
    assert!(
        !app.state().job_running(),
        "Start must not stream the previously fed program"
    );
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

/// Frame budget of the 1,000,000-line test (Review Focus 2), in debug and release alike: the
/// workspace's dev profile is optimised (`opt-level = 1`) and the virtualised view measures
/// about 3 ms per frame there, so 50 ms leaves room for a slow CI runner while a whole-text
/// layout (seconds at this size) still fails.
const FRAME_BUDGET: Duration = Duration::from_millis(50);

/// Budget for a frame after a keystroke in the editable view of a 49,999-line file. Measured
/// locally (dev profile, this machine): the slowest of three such frames took 1.99-2.06 s over
/// four runs, so the budget is 3x that. It guards against a regression, not against the
/// cost itself: re-laying out the whole text per keystroke is slow near the threshold.
const EDIT_FRAME_BUDGET: Duration = Duration::from_millis(6000);

/// Review Focus 2: a 1,000,000-line file draws a frame within [`FRAME_BUDGET`]. Ignored in
/// the default run because generating the file is slow; CI runs it with
/// `cargo nextest run -p gcodekit5-ui-egui --run-ignored only -E 'test(test_large_file_frame_time)'`.
#[test]
#[ignore = "1M-line file; CI runs it with --run-ignored only"]
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
    assert!(
        worst < FRAME_BUDGET,
        "slowest frame {worst:?}, budget {FRAME_BUDGET:?}"
    );
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

#[test]
fn test_launch_files_skip_non_gcode() -> TestResult {
    let dir = scratch("launch_filter")?;
    // Content that would stream if it were taken for G-code.
    let svg = write(&dir, "design.svg", "G1 X99\n<svg/>\n")?;
    let gcode = write(&dir, "part.NC", "G0 X1\n")?;
    let mut app = GcodeKitApp::from_state(AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    ));
    app.open_startup_files(&[svg.clone(), gcode.clone()]);
    assert_eq!(app.editor().borrow().buffer().path(), Some(gcode.as_path()));
    let program: Vec<String> = app
        .state()
        .machine
        .program()
        .map(|p| p.to_vec())
        .unwrap_or_default();
    assert_eq!(program, ["G0 X1"]);

    // Only non-G-code paths: nothing is opened and nothing is handed to the machine.
    let mut app = GcodeKitApp::from_state(AppState::new(
        SettingsManager::detached(Config::default()),
        crate::test_smoke::native_decision(),
    ));
    app.open_startup_files(&[svg, PathBuf::from("--stray-argument")]);
    assert_eq!(app.editor().borrow().buffer().path(), None);
    assert!(app.state().machine.program().is_none());
    Ok(())
}

/// An Open whose unsaved-changes prompt could not be shown is abandoned: the same prompt
/// answered later for another feature does not open a file dialog.
#[test]
fn test_open_prompt_unavailable_abandons_open() {
    let mut harness = app_harness();
    harness
        .state()
        .editor()
        .borrow_mut()
        .buffer_mut()
        .set_text("G1 X1\n".to_string());
    harness
        .state_mut()
        .dialogs_mut()
        .set_override(Some(Box::new(|_: &Request| {
            Answer::Unavailable("no parent window".into())
        })));
    harness.state_mut().dispatch(Command::FileOpen);
    app_frame(&mut harness);
    app_frame(&mut harness);

    let files = std::sync::Arc::new(std::sync::Mutex::new(0usize));
    let seen = std::sync::Arc::clone(&files);
    harness
        .state_mut()
        .dialogs_mut()
        .set_override(Some(Box::new(move |request: &Request| match request {
            Request::File(_) => {
                if let Ok(mut n) = seen.lock() {
                    *n += 1;
                }
                Answer::Cancelled
            }
            Request::Message(_) => {
                Answer::Message(craft_dialogs::MessageResult::Custom("Discard".into()))
            }
        })));
    harness
        .state_mut()
        .dialogs_mut()
        .confirm(gcodekit5_ui_egui::dialogs::ConfirmRequest::UnsavedGcode);
    app_frame(&mut harness);
    app_frame(&mut harness);
    app_frame(&mut harness);
    assert_eq!(files.lock().map(|n| *n).unwrap_or(usize::MAX), 0);
}

/// While a job streams the text is read-only, saying why, so its lines keep matching the
/// stream.
#[test]
fn test_editor_read_only_while_job_runs() {
    let (mut state, shared) = crate::machine_support::loopback_state(Duration::from_millis(1));
    crate::machine_support::start_held_stream(&mut state, &shared, 50);
    let mut harness = crate::machine_support::app_harness(GcodeKitApp::from_state(state));
    harness
        .state()
        .editor()
        .borrow_mut()
        .buffer_mut()
        .set_text("G1 X1\n".to_string());
    // Show the editor's page (Machine Control).
    harness
        .state_mut()
        .workspace_mut()
        .show_pane(gcodekit5_ui_egui::workspace::PaneId::Editor);
    harness.step();
    harness.step();
    assert!(harness.state().editor().borrow().is_locked());
    assert!(
        harness
            .query_by_label("Read-only while a job is running.")
            .is_some()
    );
    let before = harness.state().editor().borrow().buffer().generation();
    let field = harness.get_by_role_and_label(egui::accesskit::Role::TextInput, "G-Code Editor");
    field.focus();
    field.type_text("M3 ");
    harness.step();
    harness.step();
    assert_eq!(
        harness.state().editor().borrow().buffer().generation(),
        before,
        "no edit while the job runs"
    );
    // The edit made before the job is not handed to the machine mid-job either.
    assert!(harness.state().editor().borrow().program_stale());
}

#[test]
fn test_notice_cleared_by_success() {
    let mut pane = EditorPane::new();
    pane.program_rejected("Cannot stream this program: too long".to_string());
    assert!(pane.notice().is_some());
    pane.program_fed();
    assert_eq!(pane.notice(), None);
}

/// Answers every message dialog with Yes, or No when `yes` is false.
fn answer_messages(app: &mut GcodeKitApp, yes: bool) {
    app.dialogs_mut()
        .set_override(Some(Box::new(move |_: &Request| {
            Answer::Message(if yes {
                craft_dialogs::MessageResult::Yes
            } else {
                craft_dialogs::MessageResult::No
            })
        })));
}

/// Clear editor asks first when the editor holds G-code: No keeps the text, Yes removes it
/// (an edit, so the file is unsaved).
#[test]
fn test_clear_editor_asks_then_clears() {
    let mut harness = app_harness();
    harness
        .state()
        .editor()
        .borrow_mut()
        .buffer_mut()
        .set_text("G1 X1\n".to_string());
    harness.step();
    harness.get_by_label("Clear editor").click();
    harness.step();
    assert_eq!(harness.state().dialogs().waiting(), 1, "no question asked");
    answer_messages(harness.state_mut(), false);
    app_frame(&mut harness);
    app_frame(&mut harness);
    assert_eq!(harness.state().editor().borrow().buffer().text(), "G1 X1\n");

    harness.get_by_label("Clear editor").click();
    harness.step();
    answer_messages(harness.state_mut(), true);
    app_frame(&mut harness);
    app_frame(&mut harness);
    let editor = harness.state().editor();
    let editor = editor.borrow();
    assert_eq!(editor.buffer().text(), "");
    assert!(editor.buffer().is_dirty());
}

/// An empty editor is cleared without a question.
#[test]
fn test_clear_editor_without_text_asks_nothing() {
    let mut harness = app_harness();
    harness
        .state()
        .editor()
        .borrow_mut()
        .buffer_mut()
        .set_text("  \n".to_string());
    harness.step();
    harness.get_by_label("Clear editor").click();
    harness.step();
    harness.step();
    assert_eq!(harness.state().dialogs().waiting(), 0);
    assert_eq!(harness.state().editor().borrow().buffer().text(), "");
}

/// While a job runs Clear editor is disabled and refused; a Yes given after a job started
/// leaves the text as it is.
#[test]
fn test_clear_editor_refused_while_job_runs() {
    let mut harness = crate::machine_support::app_harness(app_with_running_job("G1 X1\n"));
    harness.step();
    harness.step();
    use egui_kittest::kittest::NodeT;
    assert!(
        harness
            .get_by_label("Clear editor")
            .accesskit_node()
            .is_disabled(),
        "Clear editor enabled during a job"
    );
    let tr = Translator::new("en");
    harness.state().editor().borrow_mut().request_clear(&tr);
    harness.step();
    assert_eq!(harness.state().dialogs().waiting(), 0, "asked during a job");
    assert_eq!(harness.state().editor().borrow().buffer().text(), "G1 X1\n");

    // A question answered Yes after the job started does not clear either.
    let mut requested = Vec::new();
    let machine = &harness.state().state().machine;
    let mut ctx = PaneContext::new(&tr, &mut requested).with_machine(machine);
    let editor = harness.state().editor();
    let took = editor.borrow_mut().confirmed(
        gcodekit5_ui_egui::dialogs::ConfirmRequest::ClearEditor,
        gcodekit5_ui_egui::dialogs::ConfirmChoice::Yes,
        &mut ctx,
    );
    assert!(took);
    assert_eq!(editor.borrow().buffer().text(), "G1 X1\n");
}

/// Replace All replaces every match, ignoring ASCII case as find does, left to right
/// without overlaps.
#[test]
fn test_find_replace_all_text() {
    assert_eq!(
        find::replace_all("G1 x10\nG1 X10 Y1\n", "X10", "X20"),
        ("G1 X20\nG1 X20 Y1\n".to_string(), 2)
    );
    assert_eq!(find::replace_all("aaaa", "aa", "b"), ("bb".to_string(), 2));
    assert_eq!(find::replace_all("G0", "", "X"), ("G0".to_string(), 0));
    assert_eq!(find::replace_all("G0", "Z", "X"), ("G0".to_string(), 0));
}

/// Replace replaces the current match and moves to the next one; with no current match it
/// moves to the first.
#[test]
fn test_editor_replace_current_match() {
    let tr = Translator::new("en");
    let mut pane = EditorPane::new();
    pane.buffer_mut()
        .set_text("G1 X10\nG1 Y5\nG1 x10\n".to_string());
    pane.set_find_query("X10");
    pane.set_replace_text("X30");
    pane.replace(&tr);
    assert_eq!(
        pane.buffer().text(),
        "G1 X10\nG1 Y5\nG1 x10\n",
        "nothing selected"
    );
    assert_eq!(pane.buffer().current_line(), 0);
    pane.replace(&tr);
    assert_eq!(pane.buffer().text(), "G1 X30\nG1 Y5\nG1 x10\n");
    assert_eq!(pane.buffer().current_line(), 2, "moved to the next match");
    pane.replace(&tr);
    assert_eq!(pane.buffer().text(), "G1 X30\nG1 Y5\nG1 X30\n");
}

/// The editor pane in the app, with `text`, on the Machine Control page.
fn editor_app(text: &str) -> Harness<'static, GcodeKitApp> {
    let mut harness = app_harness();
    harness
        .state()
        .editor()
        .borrow_mut()
        .buffer_mut()
        .set_text(text.to_string());
    harness
        .state_mut()
        .workspace_mut()
        .show_pane(gcodekit5_ui_egui::workspace::PaneId::Editor);
    harness.step();
    harness.step();
    harness
}

/// Types `text` into the field labelled `label`.
fn type_into(harness: &mut Harness<'_, GcodeKitApp>, label: &str, text: &str) {
    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, label)
        .click();
    harness.step();
    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, label)
        .type_text(text);
    harness.step();
}

/// Replace All in the find row replaces every match, the count follows, and one undo in the
/// text field brings the whole text back.
#[test]
fn test_editor_replace_all_is_one_undo_step() {
    let original = "G1 X10\nG1 x10 Y2\nG0 X10\n";
    let mut harness = editor_app(original);
    type_into(&mut harness, "Search...", "X10");
    assert!(harness.query_by_label("Matches: 3").is_some());
    type_into(&mut harness, "Replace with ...", "X40");
    harness.get_by_label("Replace All").click();
    harness.step();
    harness.step();
    let editor = std::rc::Rc::clone(harness.state().editor());
    assert_eq!(
        editor.borrow().buffer().text(),
        "G1 X40\nG1 X40 Y2\nG0 X40\n"
    );
    assert!(editor.borrow().buffer().is_dirty());
    assert!(harness.query_by_label("Matches: 0").is_some());

    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "G-Code Editor")
        .click();
    harness.step();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    harness.step();
    harness.step();
    assert_eq!(editor.borrow().buffer().text(), original, "one undo");
}

/// Replace and Replace All are disabled while a job runs, and refused if asked anyway.
#[test]
fn test_editor_replace_refused_while_job_runs() {
    use egui_kittest::kittest::NodeT;
    let mut harness = crate::machine_support::app_harness(app_with_running_job("G1 X1\n"));
    harness.step();
    harness.step();
    let tr = Translator::new("en");
    {
        let editor = harness.state().editor();
        let mut editor = editor.borrow_mut();
        editor.set_find_query("X1");
        editor.set_replace_text("X2");
    }
    harness.step();
    for label in ["Replace", "Replace All"] {
        assert!(
            harness.get_by_label(label).accesskit_node().is_disabled(),
            "{label} enabled during a job"
        );
    }
    let editor = harness.state().editor();
    editor.borrow_mut().replace_all(&tr);
    editor.borrow_mut().find_next();
    editor.borrow_mut().replace(&tr);
    assert_eq!(editor.borrow().buffer().text(), "G1 X1\n");
    assert_eq!(
        editor.borrow().notice(),
        Some("Read-only while a job is running.")
    );
}

/// The replace row fits the editor column of a 1280 px window.
#[test]
fn test_editor_replace_row_fits_at_1280() {
    let harness = editor_app("G1 X1\n");
    // The column starts at the find row's first widget.
    let left = harness.get_by_label("Search...").rect().left();
    for label in ["Replace with ...", "Replace", "Replace All", "Clear editor"] {
        let rect = harness.get_by_label(label).rect();
        assert!(
            rect.right() <= 1280.0 && rect.left() >= left,
            "{label} at {rect:?} outside the editor column (from {left})"
        );
    }
}

/// Types `text` into the editor's text field, then presses Ctrl+Z there.
fn edit_then_undo(harness: &mut Harness<'_, GcodeKitApp>, text: Option<&str>) {
    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "G-Code Editor")
        .click();
    harness.step();
    if let Some(text) = text {
        harness
            .get_by_role_and_label(egui::accesskit::Role::TextInput, "G-Code Editor")
            .type_text(text);
        harness.step();
        harness.step();
    } else {
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
        harness.step();
        harness.step();
    }
}

/// Opening another file starts a fresh undo history: Ctrl+Z cannot bring back the first
/// file's text into the second.
#[test]
fn test_editor_open_resets_undo() -> TestResult {
    let dir = scratch("undo_open")?;
    let a = write(&dir, "a.gcode", "G0 X1\n")?;
    let b = write(&dir, "b.gcode", "G0 Y2\n")?;
    let mut harness = editor_app("");
    harness.state().editor().borrow_mut().load(&a)?;
    harness.step();
    edit_then_undo(&mut harness, Some("M3 "));
    harness.state().editor().borrow_mut().load(&b)?;
    harness.step();
    edit_then_undo(&mut harness, None);
    assert_eq!(harness.state().editor().borrow().buffer().text(), "G0 Y2\n");
    Ok(())
}

/// Clear editor starts a fresh undo history too.
#[test]
fn test_editor_clear_resets_undo() {
    let mut harness = editor_app("G1 X1\n");
    edit_then_undo(&mut harness, Some("M3 "));
    harness.get_by_label("Clear editor").click();
    harness.step();
    answer_messages(harness.state_mut(), true);
    app_frame(&mut harness);
    app_frame(&mut harness);
    assert_eq!(harness.state().editor().borrow().buffer().text(), "");
    edit_then_undo(&mut harness, None);
    assert_eq!(harness.state().editor().borrow().buffer().text(), "");
}

/// Generated G-code handed to the editor starts a fresh undo history.
#[test]
fn test_editor_generated_program_resets_undo() {
    let mut harness = editor_app("G1 X1\n");
    edit_then_undo(&mut harness, Some("M3 "));
    harness
        .state()
        .editor()
        .borrow_mut()
        .buffer_mut()
        .replace_with_generated("G0 Z5\n".to_string());
    harness.step();
    edit_then_undo(&mut harness, None);
    assert_eq!(harness.state().editor().borrow().buffer().text(), "G0 Z5\n");
}
