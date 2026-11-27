//! Design file round trip: a load reports the shapes it cannot restore (unknown shape type,
//! raster image without its file) instead of dropping them silently, and a re-save keeps
//! the file's metadata, the tool start depth and the stock origin.

use std::path::{Path, PathBuf};

use gcodekit6_designer::serialization::DesignFile;
use gcodekit6_designer::{DesignerState, Rectangle, Shape};

/// A fresh scratch folder under `target/temp/designer_round_trip`.
fn scratch(name: &str) -> std::io::Result<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/designer_round_trip")
        .join(format!("{name}-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Saves a design with one rectangle to `path`.
fn save_one_rectangle(path: &Path) -> anyhow::Result<()> {
    let mut state = DesignerState::new();
    state.add_shape_with_undo(Shape::Rectangle(Rectangle::new(10.0, 10.0, 20.0, 30.0)));
    state.save_to_file(path)
}

#[test]
fn test_unknown_shape_type_and_missing_raster_are_reported() -> anyhow::Result<()> {
    let path = scratch("report")?.join("part.gckd");
    save_one_rectangle(&path)?;

    let mut design = DesignFile::load_from_file(&path)?;
    let mut unknown = design.shapes[0].clone();
    unknown.id = 7;
    unknown.name = "Future".into();
    unknown.shape_type = "hexagon_v9".into();
    let mut raster = design.shapes[0].clone();
    raster.id = 8;
    raster.shape_type = "raster_image".into();
    raster.original_path = Some("/no/such/dir/missing.png".into());
    design.shapes.push(unknown);
    design.shapes.push(raster);
    design.save_to_file(&path)?;

    let mut state = DesignerState::new();
    let report = state.load_from_file_report(&path)?;
    assert!(!report.is_clean());
    assert_eq!(report.skipped.len(), 1, "{report:?}");
    assert_eq!(report.skipped[0].id, 7);
    assert_eq!(report.skipped[0].shape_type, "hexagon_v9");
    assert!(report.skipped[0].label().contains("Future"));
    assert!(report.skipped[0].detail.contains("hexagon_v9"));
    assert_eq!(report.missing_images.len(), 1, "{report:?}");
    assert_eq!(report.missing_images[0].id, 8);
    assert_eq!(report.missing_images[0].detail, "/no/such/dir/missing.png");
    // The known shapes still load: the rectangle and the raster (kept so a save keeps its path).
    assert_eq!(state.canvas.shape_count(), 2);
    Ok(())
}

#[test]
fn test_known_shapes_load_with_a_clean_report() -> anyhow::Result<()> {
    let path = scratch("clean")?.join("part.gckd");
    save_one_rectangle(&path)?;
    let mut state = DesignerState::new();
    let report = state.load_from_file_report(&path)?;
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(state.canvas.shape_count(), 1);
    Ok(())
}

#[test]
fn test_metadata_start_depth_and_stock_origin_survive_resave() -> anyhow::Result<()> {
    let dir = scratch("resave")?;
    let first = dir.join("first.gckd");
    let second = dir.join("second.gckd");

    let mut state = DesignerState::new();
    state.tool_settings.start_depth = 1.25;
    if let Some(stock) = state.stock_material.as_mut() {
        stock.origin = (5.0, -3.0, 2.0);
    }
    state.save_to_file(&first)?;

    let created = chrono::DateTime::parse_from_rfc3339("2020-01-02T03:04:05Z")?.to_utc();
    let mut design = DesignFile::load_from_file(&first)?;
    design.metadata.created = created;
    design.metadata.author = "Ada".into();
    design.metadata.description = "Bracket".into();
    design.save_to_file(&first)?;

    let mut loaded = DesignerState::new();
    loaded.load_from_file(&first)?;
    assert!((loaded.tool_settings.start_depth - 1.25).abs() < 1e-9);
    let origin = loaded.stock_material.as_ref().map(|s| s.origin);
    assert_eq!(origin, Some((5.0, -3.0, 2.0)));
    loaded.save_to_file(&second)?;

    let resaved = DesignFile::load_from_file(&second)?;
    assert_eq!(resaved.metadata.created, created);
    assert_eq!(resaved.metadata.author, "Ada");
    assert_eq!(resaved.metadata.description, "Bracket");
    assert!((resaved.toolpath_params.start_depth - 1.25).abs() < 1e-9);
    assert_eq!(
        (
            resaved.toolpath_params.stock_origin_x,
            resaved.toolpath_params.stock_origin_y,
            resaved.toolpath_params.stock_origin_z,
        ),
        (5.0, -3.0, 2.0)
    );

    let mut again = DesignerState::new();
    again.load_from_file(&second)?;
    assert!((again.tool_settings.start_depth - 1.25).abs() < 1e-9);
    assert_eq!(
        again.stock_material.as_ref().map(|s| s.origin),
        Some((5.0, -3.0, 2.0))
    );
    Ok(())
}

#[test]
fn test_format_1_1_file_without_new_fields_loads() -> anyhow::Result<()> {
    let path = scratch("v11")?.join("old.gckd");
    let json = r#"{
  "version": "1.1",
  "metadata": {
    "name": "old",
    "created": "2024-05-01T00:00:00Z",
    "modified": "2024-05-01T00:00:00Z"
  },
  "viewport": { "zoom": 1.0, "pan_x": 0.0, "pan_y": 0.0 },
  "shapes": [],
  "toolpath_params": { "feed_rate": 900.0, "stock_width": 150.0 }
}"#;
    std::fs::write(&path, json)?;
    let mut state = DesignerState::new();
    let report = state.load_from_file_report(&path)?;
    assert!(report.is_clean());
    assert_eq!(state.design_name, "old");
    assert!((state.tool_settings.feed_rate - 900.0).abs() < 1e-9);
    assert_eq!(state.tool_settings.start_depth, 0.0);
    let stock = state.stock_material.as_ref().map(|s| (s.width, s.origin));
    assert_eq!(stock, Some((150.0, (0.0, 0.0, 0.0))));
    Ok(())
}
