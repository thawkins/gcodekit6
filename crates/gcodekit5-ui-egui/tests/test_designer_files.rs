//! Designer file operations on the pane: New, Open and Save of designs (round trip, atomic
//! save, view flags cleared), Import SVG / DXF / image, Export G-code and SVG (hidden shapes
//! are never exported), and Generate G-Code / Frame for the editor. The app's dialogs and
//! the quit prompt are in `test_designer_files_app.rs`.

use std::path::{Path, PathBuf};

use gcodekit5_app::i18n::Translator;
use gcodekit5_designer::designer_state::MachineMode;
use gcodekit5_designer::model::{DesignRectangle, DesignerShape, Shape};
use gcodekit5_ui_egui::panes::designer::DesignerPane;
use gcodekit5_ui_egui::panes::designer::export::ExportRefusal;

use crate::test_designer_canvas::{ids, pane_with_rects};

/// English.
pub fn en() -> Translator {
    Translator::new("en")
}

/// A checked-in fixture under `tests/fixtures/designer`.
pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/designer")
        .join(name)
}

/// A fresh scratch folder under `target/temp/egui_designer_files`.
pub fn scratch(name: &str) -> std::io::Result<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/egui_designer_files")
        .join(format!("{name}-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Whether `gcode` has a feed move: a G1 / G01 word, or a modal coordinate line after one.
fn has_cutting_moves(gcode: &str) -> bool {
    gcode.lines().any(|l| {
        let word = l.split_whitespace().next().unwrap_or_default();
        word == "G1" || word == "G01" || word.starts_with('X') || word.starts_with('Y')
    })
}

#[test]
fn test_import_svg_adds_its_shapes() {
    let mut pane = DesignerPane::new();
    assert!(pane.import_file(&fixture("shapes.svg"), &en()));
    assert_eq!(ids(&pane).len(), 2, "a rectangle and a circle");
    assert!(pane.is_modified());
    assert!(pane.notice().is_some_and(|n| n.starts_with("Imported:")));
}

#[test]
fn test_import_dxf_adds_its_shapes() {
    let mut pane = DesignerPane::new();
    assert!(pane.import_file(&fixture("shapes.dxf"), &en()));
    assert_eq!(ids(&pane).len(), 2, "a line and a circle");
    let (x1, _, x2, _) = pane
        .state()
        .canvas
        .shapes()
        .map(|o| o.shape.bounds())
        .fold((f64::MAX, 0.0, f64::MIN, 0.0), |a, b| {
            (a.0.min(b.0), 0.0, a.2.max(b.2), 0.0)
        });
    assert!(
        (x1 - 0.0).abs() < 1e-6 && (x2 - 50.0).abs() < 1e-6,
        "{x1} {x2}"
    );
}

#[test]
fn test_import_of_an_unsupported_file_changes_nothing() -> std::io::Result<()> {
    let path = scratch("import_bad")?.join("notes.txt");
    std::fs::write(&path, "hello")?;
    let mut pane = DesignerPane::new();
    assert!(!pane.import_file(&path, &en()));
    assert!(ids(&pane).is_empty());
    assert!(
        pane.notice()
            .is_some_and(|n| n.starts_with("Error importing file:"))
    );
    Ok(())
}

#[test]
fn test_import_image_in_laser_mode_only() {
    let mut pane = DesignerPane::new();
    pane.state_mut().set_machine_mode(MachineMode::Cnc3D);
    assert!(!pane.import_image(&fixture("checker.png"), &en()));
    assert!(ids(&pane).is_empty());
    assert_eq!(
        pane.notice(),
        Some("Raster image import is only available in 2D mode")
    );
    pane.state_mut().set_machine_mode(MachineMode::Laser2D);
    assert!(pane.import_image(&fixture("checker.png"), &en()));
    let shapes: Vec<_> = pane.state().canvas.shapes().collect();
    assert_eq!(shapes.len(), 1);
    assert!(matches!(shapes[0].shape, Shape::RasterImage(_)));
}

#[test]
fn test_save_then_open_round_trips_the_design() -> std::io::Result<()> {
    let path = scratch("round_trip")?.join("part.gckd");
    let mut pane = pane_with_rects(&[(10.0, 20.0, 30.0, 40.0), (100.0, 50.0, 25.0, 25.0)]);
    pane.state_mut().set_machine_mode(MachineMode::Cnc3D);
    pane.state_mut().set_feed_rate(777.0);
    assert!(pane.is_modified());
    assert!(pane.save_design(&path, &en()));
    assert!(!pane.is_modified(), "a save clears the modified flag");
    assert_eq!(pane.design_path(), Some(path.as_path()));
    assert!(pane.notice().is_some_and(|n| n.starts_with("Saved:")));
    // No temporary file is left beside the design.
    let names: Vec<_> = std::fs::read_dir(path.parent().expect("dir"))?
        .filter_map(|e| e.ok().map(|e| e.file_name()))
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");

    let mut other = DesignerPane::new();
    assert!(other.open_design(&path, &en()));
    assert!(!other.is_modified());
    assert_eq!(other.design_path(), Some(path.as_path()));
    assert_eq!(other.state().machine_mode(), MachineMode::Cnc3D);
    assert!((other.state().tool_settings.feed_rate - 777.0).abs() < 1e-9);
    let bounds = |p: &DesignerPane| {
        let mut b: Vec<_> = p
            .state()
            .canvas
            .shapes()
            .map(|o| o.shape.bounds())
            .map(|(a, b, c, d)| [a, b, c, d].map(|v| (v * 1000.0).round()))
            .collect();
        b.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
        b
    };
    assert_eq!(bounds(&other), bounds(&pane));
    // Ids continue after the loaded shapes: a new shape gets a fresh one.
    let before = ids(&other);
    other
        .state_mut()
        .add_shape_with_undo(Shape::Rectangle(DesignRectangle::new(0.0, 0.0, 5.0, 5.0)));
    let after = ids(&other);
    assert_eq!(after.len(), 3);
    assert!(after.iter().filter(|id| before.contains(id)).count() == 2);
    // Undo does not reach into the previous document.
    assert!(!other.state().can_redo());
    Ok(())
}

#[test]
fn test_open_failure_keeps_the_design() -> std::io::Result<()> {
    let path = scratch("open_bad")?.join("broken.gckd");
    std::fs::write(&path, "{ not json")?;
    let mut pane = pane_with_rects(&[(10.0, 10.0, 10.0, 10.0)]);
    assert!(!pane.open_design(&path, &en()));
    assert_eq!(ids(&pane).len(), 1);
    assert!(pane.notice().is_some_and(|n| n.starts_with("Open Failed")));
    Ok(())
}

#[test]
fn test_save_refuses_a_read_only_target_and_stays_modified() -> std::io::Result<()> {
    let path = scratch("read_only")?.join("locked.gckd");
    std::fs::write(&path, "old")?;
    let mut perms = std::fs::metadata(&path)?.permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&path, perms)?;
    let mut pane = pane_with_rects(&[(10.0, 10.0, 10.0, 10.0)]);
    assert!(!pane.save_design(&path, &en()));
    assert!(pane.is_modified());
    assert_eq!(std::fs::read_to_string(&path)?, "old");
    assert!(pane.notice().is_some_and(|n| n.starts_with("Save Failed")));
    assert_eq!(pane.design_path(), None);
    let mut perms = std::fs::metadata(&path)?.permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&path, perms)?;
    Ok(())
}

/// Hide/lock are view state: a new or opened design must not inherit them by shape id.
#[test]
fn test_new_and_open_clear_the_view_flags() -> std::io::Result<()> {
    let path = scratch("flags")?.join("design.gckd");
    let mut pane = pane_with_rects(&[(10.0, 10.0, 10.0, 10.0)]);
    assert!(pane.save_design(&path, &en()));
    let id = ids(&pane)[0];
    pane.set_hidden(id, true);
    pane.set_locked(id, true);
    assert!(pane.open_design(&path, &en()));
    assert_eq!(ids(&pane), [id], "the opened design reuses the id");
    assert!(!pane.flags().is_hidden(id) && !pane.flags().is_locked(id));

    pane.set_hidden(id, true);
    pane.new_design(MachineMode::Cnc3D, &en());
    assert!(ids(&pane).is_empty());
    assert!(!pane.is_modified());
    assert_eq!(pane.design_path(), None);
    assert_eq!(pane.state().machine_mode(), MachineMode::Cnc3D);
    assert!(!pane.state().can_undo(), "no undo into the old design");
    assert_eq!(pane.notice(), Some("New 3D design created"));
    pane.state_mut()
        .add_shape_with_undo(Shape::Rectangle(DesignRectangle::new(0.0, 0.0, 5.0, 5.0)));
    let new_id = ids(&pane)[0];
    assert!(!pane.flags().is_hidden(new_id));
    Ok(())
}

/// Carry: export goes through `DesignerPane::generate_gcode`, so hidden shapes are never
/// cut; with every shape hidden nothing is exported and the user is told why.
#[test]
fn test_export_with_every_shape_hidden_cuts_nothing() -> std::io::Result<()> {
    let mut pane = pane_with_rects(&[(10.0, 10.0, 20.0, 20.0), (50.0, 50.0, 20.0, 20.0)]);
    for id in ids(&pane) {
        pane.set_hidden(id, true);
    }
    assert!(!has_cutting_moves(&pane.generate_gcode(None)));
    assert_eq!(pane.export_gcode(None), Err(ExportRefusal::AllHidden));
    let path = scratch("all_hidden")?.join("out.nc");
    assert!(!pane.write_gcode(&path, &en()));
    assert!(!path.exists());
    assert_eq!(
        pane.notice(),
        Some("Nothing to export: all shapes are hidden")
    );
    pane.generate_to_editor(&en());
    assert_eq!(pane.take_editor_gcode(), None);
    assert_eq!(
        pane.notice(),
        Some("Nothing to export: all shapes are hidden")
    );
    let empty = DesignerPane::new();
    assert_eq!(empty.export_gcode(None), Err(ExportRefusal::Empty));
    Ok(())
}

#[test]
fn test_export_gcode_writes_only_visible_shapes() -> std::io::Result<()> {
    let mut pane = pane_with_rects(&[(10.0, 10.0, 20.0, 20.0), (70.0, 10.0, 20.0, 20.0)]);
    let hidden = ids(&pane)[1];
    pane.set_hidden(hidden, true);
    let path = scratch("export_gcode")?.join("out.nc");
    assert!(pane.write_gcode(&path, &en()));
    let gcode = std::fs::read_to_string(&path)?;
    assert!(has_cutting_moves(&gcode));
    assert!(gcode.contains("X30.000"), "{gcode}");
    assert!(
        !gcode.contains("X90.000"),
        "the hidden shape is cut:\n{gcode}"
    );
    assert!(
        pane.notice()
            .is_some_and(|n| n.starts_with("Exported G-Code:"))
    );
    Ok(())
}

#[test]
fn test_export_is_blocked_by_objects_above_safe_z() {
    let mut pane = pane_with_rects(&[(10.0, 10.0, 20.0, 20.0)]);
    pane.state_mut().set_machine_mode(MachineMode::Cnc3D);
    let id = ids(&pane)[0];
    if let Some(obj) = pane.state_mut().canvas.get_shape_mut(id) {
        // Start depth below zero puts the object above the stock top and safe Z.
        obj.start_depth = -100.0;
        obj.use_custom_values = true;
    }
    assert!(matches!(
        pane.export_gcode(None),
        Err(ExportRefusal::AboveSafeZ { count: 1, .. })
    ));
}

#[test]
fn test_export_svg_writes_visible_shapes() -> std::io::Result<()> {
    let mut pane = pane_with_rects(&[(10.0, 10.0, 20.0, 20.0), (70.0, 10.0, 20.0, 20.0)]);
    pane.set_hidden(ids(&pane)[1], true);
    let path = scratch("export_svg")?.join("out.svg");
    assert!(pane.write_svg(&path, &en()));
    let svg = std::fs::read_to_string(&path)?;
    assert!(svg.starts_with("<?xml"));
    assert_eq!(svg.matches("<rect").count(), 1, "{svg}");
    assert!(svg.trim_end().ends_with("</svg>"));
    let empty = DesignerPane::new().write_svg(&path, &en());
    assert!(!empty);
    Ok(())
}

#[test]
fn test_generate_and_frame_hand_gcode_to_the_editor() {
    let mut pane = pane_with_rects(&[(10.0, 20.0, 30.0, 40.0)]);
    pane.generate_to_editor(&en());
    let gcode = pane.take_editor_gcode().expect("generated G-code");
    assert!(has_cutting_moves(&gcode));
    assert_eq!(pane.take_editor_gcode(), None, "taken once");
    assert_eq!(pane.notice(), Some("G-Code generated"));
    pane.frame_to_editor();
    let frame = pane.take_editor_gcode().expect("frame G-code");
    assert!(frame.contains("G0 X10.000 Y20.000"), "{frame}");
    assert!(frame.contains("G1 X40.000 Y20.000"), "{frame}");
    assert!(frame.contains("X40.000 Y60.000"), "{frame}");
}
