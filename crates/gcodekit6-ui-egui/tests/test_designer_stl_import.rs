//! STL import in the Designer, behind GTK's Settings switch ("STL File Import",
//! `ui.enable_stl_import`): with it on, Import offers STL files and adds the model's shadow
//! (`StlImporter`, unscaled, not centred, as GTK); with it off, STL files are not offered and
//! an STL file is refused with GTK's message.

use std::path::PathBuf;

use gcodekit6_designer::model::DesignerShape;
use gcodekit6_settings::config::Config;
use gcodekit6_ui_egui::dialogs::{FileFilter, FilePurpose, import_design_filters};
use gcodekit6_ui_egui::panes::designer::DesignerPane;
use gcodekit6_ui_egui::workspace::Pane;

use crate::test_designer_canvas::ids;
use crate::test_designer_files::{en, scratch};

/// A flat 10 x 20 mm rectangle at z = 0 as an ASCII STL file in a scratch folder.
fn plate_stl(name: &str) -> std::io::Result<PathBuf> {
    let path = scratch(name)?.join("plate.stl");
    let facet = |a: [f32; 3], b: [f32; 3], c: [f32; 3]| {
        format!(
            "facet normal 0 0 1\nouter loop\nvertex {} {} {}\nvertex {} {} {}\n\
             vertex {} {} {}\nendloop\nendfacet\n",
            a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]
        )
    };
    let body = format!(
        "solid plate\n{}{}endsolid plate\n",
        facet([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 20.0, 0.0]),
        facet([0.0, 0.0, 0.0], [10.0, 20.0, 0.0], [0.0, 20.0, 0.0])
    );
    std::fs::write(&path, body)?;
    Ok(path)
}

/// A pane with the STL switch `on`.
fn pane_with_stl(on: bool) -> DesignerPane {
    let mut pane = DesignerPane::new();
    let mut config = Config::default();
    config.ui.enable_stl_import = on;
    pane.configure(&config);
    pane
}

#[test]
fn test_stl_import_adds_the_models_shadow_when_enabled() -> std::io::Result<()> {
    let path = plate_stl("stl_on")?;
    let mut pane = pane_with_stl(true);
    assert!(pane.import_file(&path, &en()), "{:?}", pane.notice());
    assert!(!ids(&pane).is_empty());
    let (x1, y1, x2, y2) = pane
        .state()
        .canvas
        .shapes()
        .map(|o| o.shape.bounds())
        .fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, b| {
            (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
        });
    let near = |a: f64, b: f64| (a - b).abs() < 0.01;
    assert!(
        near(x1, 0.0) && near(y1, 0.0) && near(x2, 10.0) && near(y2, 20.0),
        "not centred, unscaled: {x1} {y1} {x2} {y2}"
    );
    // One import is one undo step.
    pane.undo();
    assert!(ids(&pane).is_empty());
    Ok(())
}

#[test]
fn test_stl_import_is_refused_when_disabled() -> std::io::Result<()> {
    let path = plate_stl("stl_off")?;
    let mut pane = pane_with_stl(false);
    assert!(!pane.import_file(&path, &en()));
    assert!(ids(&pane).is_empty());
    let notice = pane.notice().unwrap_or_default();
    assert!(
        notice.contains("STL import requires the STL import feature to be enabled in settings"),
        "{notice}"
    );
    Ok(())
}

#[test]
fn test_import_dialog_offers_stl_only_when_enabled() {
    assert_eq!(
        import_design_filters(false),
        FilePurpose::ImportDesign.filters()
    );
    let off: Vec<&str> = import_design_filters(false)
        .iter()
        .flat_map(|f| f.extensions.iter().copied())
        .collect();
    assert!(!off.contains(&"stl"), "{off:?}");
    let on = import_design_filters(true);
    assert_eq!(on[0].extensions, ["svg", "dxf", "stl"], "Supported Files");
    assert!(on.contains(&FileFilter::STL));
    assert_eq!(FileFilter::STL.name_msgid, "STL Files");
}
