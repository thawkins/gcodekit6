//! Locking tests for SVG `<rect>` corner radii and root units without a viewBox
//! (audit T08: P2DES-5, P2DES-10).

use gcodekit5_designer::SvgImporter;
use gcodekit5_designer::model::{DesignerShape, Shape};

const EPS: f64 = 1e-3;

fn import_svg(svg: &str) -> Vec<Shape> {
    SvgImporter::new(1.0, 0.0, 0.0)
        .import_string(svg)
        .expect("svg import should succeed")
        .shapes
}

fn assert_bounds(actual: (f64, f64, f64, f64), expected: (f64, f64, f64, f64)) {
    let ok = (actual.0 - expected.0).abs() < EPS
        && (actual.1 - expected.1).abs() < EPS
        && (actual.2 - expected.2).abs() < EPS
        && (actual.3 - expected.3).abs() < EPS;
    assert!(ok, "bounds {actual:?}, expected {expected:?}");
}

#[test]
fn test_svg_rect_rx_imports_corner_radius() {
    let shapes = import_svg(
        r#"<svg width="100" height="100"><rect x="0" y="0" width="10" height="8" rx="2"/></svg>"#,
    );
    assert_eq!(shapes.len(), 1);
    let Shape::Rectangle(rect) = &shapes[0] else {
        panic!("expected a rectangle, got {:?}", shapes[0]);
    };
    assert!(
        (rect.corner_radius - 2.0).abs() < EPS,
        "{}",
        rect.corner_radius
    );
    assert!((rect.width - 10.0).abs() < EPS && (rect.height - 8.0).abs() < EPS);
}

#[test]
fn test_svg_rect_ry_only_and_scale_and_clamp() {
    // ry alone gives rx; the uniform scale(2) doubles the radius.
    let shapes = import_svg(
        r#"<svg width="100" height="100">
            <rect x="0" y="0" width="10" height="8" ry="1.5" transform="scale(2)"/>
            <rect x="0" y="0" width="4" height="4" rx="9"/>
        </svg>"#,
    );
    let radii: Vec<f64> = shapes
        .iter()
        .map(|s| match s {
            Shape::Rectangle(r) => r.corner_radius,
            other => panic!("expected a rectangle, got {other:?}"),
        })
        .collect();
    assert!((radii[0] - 3.0).abs() < EPS, "{radii:?}");
    // rx larger than half the side is clamped to it.
    assert!((radii[1] - 2.0).abs() < EPS, "{radii:?}");
}

#[test]
fn test_svg_rect_elliptical_corners_become_rounded_path() {
    let shapes = import_svg(
        r#"<svg width="100" height="100"><rect width="10" height="8" rx="3" ry="1"/></svg>"#,
    );
    assert_eq!(shapes.len(), 1);
    let Shape::Path(path) = &shapes[0] else {
        panic!("expected a path, got {:?}", shapes[0]);
    };
    assert_bounds(path.bounds(), (0.0, 0.0, 10.0, 8.0));
    // A rounded outline has curves, a plain rectangle only lines.
    let curves = path
        .rotated_render()
        .iter()
        .filter(|e| matches!(e, lyon::path::Event::Cubic { .. }))
        .count();
    assert_eq!(curves, 4);
}

#[test]
fn test_svg_no_viewbox_with_mm_width_uses_px_user_units() {
    // Without a viewBox one user unit is one CSS px (25.4/96 mm), whatever unit
    // the width is given in; the width only sets the document size.
    let svg =
        r#"<svg width="100mm" height="50mm"><rect x="0" y="0" width="96" height="48"/></svg>"#;
    let design = SvgImporter::new(1.0, 0.0, 0.0)
        .import_string(svg)
        .expect("svg import should succeed");
    assert_bounds(design.shapes[0].bounds(), (0.0, 0.0, 25.4, 12.7));
    assert!((design.dimensions.0 - 100.0).abs() < EPS);
    assert!((design.dimensions.1 - 50.0).abs() < EPS);
}

#[test]
fn test_svg_no_viewbox_cm_and_unitless_widths() {
    let cm = import_svg(r#"<svg width="10cm" height="10cm"><rect width="96" height="96"/></svg>"#);
    assert_bounds(cm[0].bounds(), (0.0, 0.0, 25.4, 25.4));
    // Unitless keeps the historical one user unit = 1 mm.
    let mm = import_svg(r#"<svg width="100" height="100"><rect width="96" height="96"/></svg>"#);
    assert_bounds(mm[0].bounds(), (0.0, 0.0, 96.0, 96.0));
}
