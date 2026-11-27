//! DXF import fidelity (audit T07): R12 POLYLINE bulges, skipped-entity report,
//! code-page (non-UTF-8) files and true circular arcs.

use gcodekit6_designer::dxf_parser::{DxfEntity, DxfParser, decode_dxf_bytes};
use gcodekit6_designer::import::DxfImporter;
use gcodekit6_designer::model::{DesignerShape, Shape};
use lyon::path::iterator::PathIterator;

/// Wraps entity text in a minimal ENTITIES section.
fn dxf(entities: &str) -> String {
    format!("0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n")
}

/// Points of the imported path flattened finely, plus the midpoint of every flattened
/// segment (where a chord strays furthest from the curve).
fn sample_path(shape: &Shape) -> Vec<(f64, f64)> {
    let Shape::Path(path) = shape else {
        panic!("expected a path, got {shape:?}");
    };
    let lyon_path = path.original_path.as_ref().expect("imported path geometry");
    let mut points = Vec::new();
    for event in lyon_path.iter().flattened(0.0005) {
        match event {
            lyon::path::Event::Begin { at } => points.push((at.x as f64, at.y as f64)),
            lyon::path::Event::Line { from, to } => {
                points.push((
                    (from.x as f64 + to.x as f64) / 2.0,
                    (from.y as f64 + to.y as f64) / 2.0,
                ));
                points.push((to.x as f64, to.y as f64));
            }
            lyon::path::Event::End {
                last,
                first,
                close: true,
            } => {
                points.push((
                    (last.x as f64 + first.x as f64) / 2.0,
                    (last.y as f64 + first.y as f64) / 2.0,
                ));
            }
            _ => {}
        }
    }
    points
}

/// An AutoCAD R12 POLYLINE keeps the bulge (group 42) of its VERTEX records, so a
/// bulge of 1 between (0,0) and (10,0) is a half circle of radius 5, not a line.
#[test]
fn test_dxf_bulged_r12_polyline_imports_arcs() {
    let content = dxf("0\nPOLYLINE\n8\n0\n66\n1\n70\n0\n\
0\nVERTEX\n8\n0\n10\n0.0\n20\n0.0\n42\n1.0\n\
0\nVERTEX\n8\n0\n10\n10.0\n20\n0.0\n\
0\nSEQEND\n");

    let file = DxfParser::parse(&content).expect("parse");
    match &file.entities[..] {
        [DxfEntity::Polyline(p)] => {
            assert_eq!(p.vertices.len(), 2);
            assert_eq!(p.bulges, vec![1.0, 0.0]);
        }
        other => panic!("expected one polyline, got {other:?}"),
    }
    assert!(file.unsupported_entities.is_empty());

    let design = DxfImporter::new(1.0, 0.0, 0.0)
        .import_string(&content)
        .expect("import");
    assert_eq!(design.shapes.len(), 1);
    let (x1, y1, x2, y2) = design.shapes[0].bounds();
    assert!(
        (x1 - 0.0).abs() < 0.01 && (x2 - 10.0).abs() < 0.01,
        "x {x1}..{x2}"
    );
    // A positive bulge turns counter-clockwise, so the arc from (0,0) to (10,0) dips to -5.
    assert!((y1 + 5.0).abs() < 0.01 && y2.abs() < 0.01, "y {y1}..{y2}");
}

/// A pre-2007 DXF is written in the drawing's code page; a Windows-1252 layer name
/// must not make the import fail.
#[test]
fn test_dxf_cp1252_file_loads() {
    let mut bytes = b"0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nM".to_vec();
    bytes.extend_from_slice(&[0xF6, b'b', b'e', b'l', 0x80]); // "Möbel€" in CP1252
    bytes.extend_from_slice(
        b"\n10\n0.0\n20\n0.0\n11\n10.0\n21\n0.0\n0\nENDSEC\n0\nEOF\n".as_slice(),
    );
    assert!(std::str::from_utf8(&bytes).is_err());
    assert!(decode_dxf_bytes(&bytes).contains("M\u{f6}bel\u{20ac}"));

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/test_dxf_cp1252_file_loads");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("cp1252.dxf");
    std::fs::write(&path, &bytes).expect("write fixture");

    let design = DxfImporter::new(1.0, 0.0, 0.0)
        .import_file(&path.to_string_lossy())
        .expect("a CP1252 DXF must load");
    assert_eq!(design.shapes.len(), 1);

    // UTF-8 content is read as UTF-8, not reinterpreted.
    assert_eq!(decode_dxf_bytes("M\u{f6}bel".as_bytes()), "M\u{f6}bel");
}

/// Entities the importer skips (block references, multiline text, ...) are counted so
/// the UI can say what was discarded; group values of "0" are not mistaken for types.
#[test]
fn test_dxf_insert_and_mtext_are_counted() {
    let content = dxf(
        "0\nLINE\n8\n0\n62\n0\n10\n0.0\n20\n0.0\n11\n10.0\n21\n0.0\n\
0\nINSERT\n8\n0\n2\nBLOCK1\n10\n0.0\n20\n0.0\n\
0\nMTEXT\n8\n0\n10\n0.0\n20\n0.0\n40\n2.5\n1\nhello\n\
0\nMTEXT\n8\n0\n10\n5.0\n20\n0.0\n40\n2.5\n1\nworld\n",
    );

    let design = DxfImporter::new(1.0, 0.0, 0.0)
        .import_string(&content)
        .expect("import");
    assert_eq!(design.shapes.len(), 1);
    assert_eq!(
        design.unsupported_entities,
        vec![("INSERT".to_string(), 1), ("MTEXT".to_string(), 2)]
    );
}

/// A DXF circle is imported as true arcs: at r = 500 mm the chord sag stays within
/// 0.01 mm (a fixed 64-gon strayed 0.6 mm).
#[test]
fn test_dxf_large_circle_sag_within_tolerance() {
    let content = dxf("0\nCIRCLE\n8\n0\n10\n0.0\n20\n0.0\n40\n500.0\n");
    let design = DxfImporter::new(1.0, 0.0, 0.0)
        .import_string(&content)
        .expect("import");
    assert_eq!(design.shapes.len(), 1);
    let points = sample_path(&design.shapes[0]);
    assert!(points.len() > 64);
    let worst = points
        .iter()
        .map(|(x, y)| ((x * x + y * y).sqrt() - 500.0).abs())
        .fold(0.0, f64::max);
    assert!(worst <= 0.01, "circle strays {worst} mm from r = 500");
}

/// ARC and ELLIPSE entities follow the true curve too, from the same start point.
#[test]
fn test_dxf_arc_and_ellipse_follow_the_curve() {
    let arc = dxf("0\nARC\n8\n0\n10\n0.0\n20\n0.0\n40\n400.0\n50\n30.0\n51\n300.0\n");
    let design = DxfImporter::new(1.0, 0.0, 0.0)
        .import_string(&arc)
        .expect("import arc");
    let points = sample_path(&design.shapes[0]);
    let (sx, sy) = points[0];
    let start = 30f64.to_radians();
    assert!((sx - 400.0 * start.cos()).abs() < 0.01 && (sy - 400.0 * start.sin()).abs() < 0.01);
    for (x, y) in &points {
        assert!(
            ((x * x + y * y).sqrt() - 400.0).abs() <= 0.01,
            "arc point ({x}, {y})"
        );
    }

    // Full ellipse: major axis (300, 300) (length 424.26, rotated 45 deg), ratio 0.5.
    let ellipse = dxf("0\nELLIPSE\n8\n0\n10\n0.0\n20\n0.0\n11\n300.0\n21\n300.0\n\
40\n0.5\n41\n0.0\n42\n6.283185307179586\n");
    let design = DxfImporter::new(1.0, 0.0, 0.0)
        .import_string(&ellipse)
        .expect("import ellipse");
    let a = (2.0f64 * 300.0 * 300.0).sqrt();
    let b = a * 0.5;
    let (c, s) = (
        std::f64::consts::FRAC_PI_4.cos(),
        std::f64::consts::FRAC_PI_4.sin(),
    );
    for (x, y) in sample_path(&design.shapes[0]) {
        // Rotate into the ellipse frame and measure the radial error along the ray.
        let (u, v) = (x * c + y * s, -x * s + y * c);
        let r = (u * u + v * v).sqrt();
        let t = v.atan2(u);
        let expected = a * b / ((b * t.cos()).powi(2) + (a * t.sin()).powi(2)).sqrt();
        assert!(
            (r - expected).abs() <= 0.01,
            "ellipse point ({x}, {y}): {r} vs {expected}"
        );
    }
}
