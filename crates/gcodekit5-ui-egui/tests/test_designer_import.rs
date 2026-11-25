//! Designer import, in detail: one import is one undo step, files over the size limit and
//! malformed SVG / DXF content are refused with a notice and leave the design unchanged.

use gcodekit5_designer::designer_state::MachineMode;
use gcodekit5_ui_egui::panes::designer::DesignerPane;
use gcodekit5_ui_egui::panes::designer::file_ops::MAX_IMPORT_BYTES;

use crate::test_designer_canvas::{ids, pane_with_rects};
use crate::test_designer_files::{en, fixture, scratch};

#[test]
fn test_import_is_one_undo_step() {
    for name in ["shapes.svg", "shapes.dxf"] {
        let mut pane = pane_with_rects(&[(10.0, 10.0, 10.0, 10.0)]);
        assert!(pane.import_file(&fixture(name), &en()), "{name}");
        assert_eq!(ids(&pane).len(), 3, "{name}");
        pane.undo();
        assert_eq!(
            ids(&pane).len(),
            1,
            "{name}: one undo removes every imported shape"
        );
        pane.redo();
        assert_eq!(ids(&pane).len(), 3, "{name}: redo brings them back");
    }
}

/// Depth in a DXF switches to CNC 3D; the switch is not part of the undo step (the model
/// records no machine-mode command), so undo removes the shapes and keeps the mode.
#[test]
fn test_import_with_depth_switches_to_cnc_in_the_same_step() -> std::io::Result<()> {
    let path = scratch("import_depth")?.join("deep.dxf");
    std::fs::write(
        &path,
        "0\nSECTION\n2\nENTITIES\n0\nCIRCLE\n8\n0\n10\n25.0\n20\n25.0\n38\n2.0\n39\n3.0\n40\n10.0\n0\nENDSEC\n0\nEOF\n",
    )?;
    let mut pane = DesignerPane::new();
    assert!(pane.import_file(&path, &en()));
    assert_eq!(pane.state().machine_mode(), MachineMode::Cnc3D);
    assert!(
        pane.notice()
            .is_some_and(|n| n.contains("3D depth detected"))
    );
    let obj = pane
        .state()
        .canvas
        .shapes()
        .next()
        .cloned()
        .expect("circle");
    assert!(obj.use_custom_values);
    pane.undo();
    assert!(ids(&pane).is_empty());
    Ok(())
}

#[test]
fn test_import_refuses_files_over_the_size_limit() -> std::io::Result<()> {
    let dir = scratch("import_big")?;
    for name in ["big.svg", "big.png"] {
        let path = dir.join(name);
        // A sparse file: the size is checked before anything is read.
        std::fs::File::create(&path)?.set_len(MAX_IMPORT_BYTES + 1)?;
        let mut pane = pane_with_rects(&[(10.0, 10.0, 10.0, 10.0)]);
        let imported = if name.ends_with("png") {
            pane.import_image(&path, &en())
        } else {
            pane.import_file(&path, &en())
        };
        assert!(!imported, "{name}");
        assert_eq!(ids(&pane).len(), 1, "{name}");
        assert!(
            pane.notice()
                .is_some_and(|n| n.contains("The file is too large to import.")),
            "{name}: {:?}",
            pane.notice()
        );
    }
    Ok(())
}

#[test]
fn test_malformed_svg_and_dxf_change_nothing() -> std::io::Result<()> {
    let dir = scratch("import_malformed")?;
    for (name, content) in [
        (
            "bad.svg",
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><rect x=\"1\" <<<",
        ),
        ("unterminated.dxf", "0\nSECTION\n2\nENTITIES\n"),
        ("garbage.dxf", "\u{0}\u{1}binary junk"),
    ] {
        let path = dir.join(name);
        std::fs::write(&path, content)?;
        let mut pane = pane_with_rects(&[(10.0, 10.0, 10.0, 10.0)]);
        let undoable = pane.state().can_undo();
        let imported = pane.import_file(&path, &en());
        assert!(!imported, "{name}: {:?}", pane.notice());
        assert_eq!(ids(&pane).len(), 1, "{name}");
        assert_eq!(pane.state().can_undo(), undoable, "{name}");
        assert!(
            pane.notice()
                .is_some_and(|n| n.starts_with("Error importing file:")),
            "{name}: {:?}",
            pane.notice()
        );
    }
    Ok(())
}

/// A DXF value that is not a number is replaced by a default (the importer's documented
/// tolerance) and the user is warned that geometry may be wrong.
#[test]
fn test_dxf_with_a_bad_number_imports_with_a_warning() -> std::io::Result<()> {
    let path = scratch("import_bad_number")?.join("bad.dxf");
    std::fs::write(
        &path,
        "0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\nnot-a-number\n20\n0.0\n11\n50.0\n21\n0.0\n0\nENDSEC\n0\nEOF\n",
    )?;
    let mut pane = DesignerPane::new();
    assert!(pane.import_file(&path, &en()));
    assert!(
        pane.notice()
            .is_some_and(|n| n.contains("Invalid numbers replaced")),
        "{:?}",
        pane.notice()
    );
    Ok(())
}
