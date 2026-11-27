//! A rotated path is drawn, exported and hit-tested where it is cut: `rotated_render`
//! matches the G-code generator's geometry, and `distance_to_point` follows the rotation.

use gcodekit6_designer::ToolpathGenerator;
use gcodekit6_designer::model::{DesignPath, Point};

/// A 20 x 10 mm rectangle at the origin, rotated 90 degrees about its centre (10, 5): it
/// then spans x 5..15, y -5..15.
fn rotated_rectangle() -> DesignPath {
    let mut b = lyon::path::Path::builder();
    b.begin(lyon::math::point(0.0, 0.0));
    b.line_to(lyon::math::point(20.0, 0.0));
    b.line_to(lyon::math::point(20.0, 10.0));
    b.line_to(lyon::math::point(0.0, 10.0));
    b.end(true);
    let mut path = DesignPath::from_lyon_path(&b.build());
    path.rotation = 90.0;
    path
}

/// The vertices of the rotated render.
fn rendered_vertices(path: &DesignPath) -> Vec<Point> {
    path.rotated_render()
        .iter()
        .filter_map(|event| match event {
            lyon::path::Event::Begin { at } => Some(at),
            lyon::path::Event::Line { to, .. } => Some(to),
            _ => None,
        })
        .map(|p| Point::new(f64::from(p.x), f64::from(p.y)))
        .collect()
}

#[test]
fn test_path_rotation_render_matches_gcode_geometry() {
    let path = rotated_rectangle();
    let drawn = rendered_vertices(&path);
    assert!(
        drawn
            .iter()
            .any(|p| p.distance_to(&Point::new(5.0, -5.0)) < 0.01),
        "not rotated: {drawn:?}"
    );
    let toolpaths = ToolpathGenerator::new().generate_path_contour(&path, 1.0);
    let cut: Vec<Point> = toolpaths
        .iter()
        .flat_map(|tp| tp.segments.iter())
        .flat_map(|s| [s.start, s.end])
        .filter(|p| p.x != 0.0 || p.y != 0.0)
        .collect();
    assert!(!cut.is_empty());
    for p in &cut {
        assert!(
            drawn.iter().any(|d| d.distance_to(p) < 0.01),
            "cut point {p:?} not on the drawn outline {drawn:?}"
        );
    }
}

#[test]
fn test_path_rotation_hit_test_follows_rotation() {
    let path = rotated_rectangle();
    // A corner of the rotated outline is on the path; the unrotated far corner is not.
    assert!(path.distance_to_point(&Point::new(15.0, 15.0)) < 0.01);
    assert!(path.distance_to_point(&Point::new(20.0, 0.0)) > 1.0);
}
