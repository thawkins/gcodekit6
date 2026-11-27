//! The designer pane's export uses the model's rules (audit T31, UIPANES-3): the Safe Z
//! check skips hidden shapes, and Generate G-Code for a selected image engraves it as the
//! design export does (its placed width and height).

use std::path::{Path, PathBuf};

use gcodekit5_app::i18n::Translator;
use gcodekit5_designer::designer_state::MachineMode;
use gcodekit5_designer::model::{Point, RasterImage, Shape};
use gcodekit5_ui_egui::panes::designer::DesignerPane;
use gcodekit5_ui_egui::panes::designer::export::ExportRefusal;

use crate::test_designer_canvas::{ids, pane_with_rects};

/// The program without its `; Generated:` timestamp line.
fn untimed(gcode: &str) -> Vec<&str> {
    gcode
        .lines()
        .filter(|l| !l.starts_with("; Generated:"))
        .collect()
}

/// A 7x3 grey ramp written under `target/temp/egui_designer_export_rules`.
fn ramp_png() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/egui_designer_export_rules")
        .join(std::process::id().to_string());
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("ramp.png");
    image::GrayImage::from_fn(7, 3, |x, y| image::Luma([(x * 30 + y * 20) as u8]))
        .save(&path)
        .expect("write ramp.png");
    path
}

#[test]
fn test_hidden_shapes_above_safe_z_do_not_block_export() {
    let mut pane = pane_with_rects(&[(10.0, 10.0, 20.0, 20.0), (50.0, 10.0, 20.0, 20.0)]);
    pane.state_mut().set_machine_mode(MachineMode::Cnc3D);
    let ids = ids(&pane);
    for (id, depth) in ids.iter().zip([-100.0, -50.0]) {
        if let Some(obj) = pane.state_mut().canvas.get_shape_mut(*id) {
            obj.start_depth = depth;
            obj.use_custom_values = true;
        }
    }
    assert!(matches!(
        pane.export_check(),
        Err(ExportRefusal::AboveSafeZ { count: 2, .. })
    ));
    pane.set_hidden(ids[0], true);
    match pane.export_check() {
        Err(ExportRefusal::AboveSafeZ {
            count, max_start_z, ..
        }) => {
            assert_eq!(count, 1);
            assert!(
                max_start_z < 100.0,
                "the hidden shape is the highest: {max_start_z}"
            );
        }
        other => panic!("expected AboveSafeZ, got {other:?}"),
    }
}

/// A 30 x 20 mm box holding a 7 x 3 px image: the editor's engraving fills the box, exactly
/// as the design export's raster section does.
#[test]
fn test_generate_for_a_selected_image_engraves_its_placed_size() {
    let mut pane = DesignerPane::new();
    let state = pane.state_mut();
    state.set_machine_mode(MachineMode::Laser2D);
    state.add_shape_with_undo(Shape::RasterImage(RasterImage::new(
        0,
        Point::new(20.0, 15.0),
        30.0,
        20.0,
        Vec::new(),
        Some(ramp_png()),
    )));
    let id = ids(&pane)[0];
    pane.state_mut().canvas.select_shape(id, false);
    pane.generate_to_editor(&Translator::new("en"));
    let editor = pane.take_editor_gcode().expect("image G-code");
    assert!(
        editor.contains("; Image size: 30.00mm x 20.00mm"),
        "{editor}"
    );

    pane.state_mut().deselect_all();
    let design = pane.generate_gcode(None);
    let raster = design
        .find("; Laser Image Engraving")
        .map(|at| &design[at..])
        .expect("the design export engraves the image");
    let editor = untimed(&editor);
    assert_eq!(untimed(raster)[..editor.len()], editor[..]);
}
