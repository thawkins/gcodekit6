//! Pocket generator bounds: the adaptive strategy clears the centre once per Z pass, and a
//! degenerate stepover or step-down cannot make the generators loop for millions of
//! iterations (the old guard was a `debug_assert!`, absent from release builds).

use gcodekit5_designer::model::Point;
use gcodekit5_designer::pocket_operations::{PocketGenerator, PocketOperation, PocketStrategy};
use gcodekit5_designer::{Toolpath, ToolpathSegmentType};

/// A 20 x 20 mm square.
fn square() -> Vec<Point> {
    vec![
        Point::new(0.0, 0.0),
        Point::new(20.0, 0.0),
        Point::new(20.0, 20.0),
        Point::new(0.0, 20.0),
    ]
}

/// The centre cleanups in `toolpath`: each crosses the square's centre (10, 10) with a
/// leg from the centre to one tool radius (1.5 mm) left of it.
fn cleanups(toolpath: &Toolpath) -> usize {
    let near = |p: Point, x: f64, y: f64| (p.x - x).abs() < 1e-6 && (p.y - y).abs() < 1e-6;
    toolpath
        .segments
        .iter()
        .filter(|s| {
            s.segment_type == ToolpathSegmentType::LinearMove
                && near(s.start, 10.0, 10.0)
                && near(s.end, 8.5, 10.0)
        })
        .count()
}

#[test]
fn test_pocket_adaptive_one_centre_cleanup_per_pass() {
    let mut op = PocketOperation::new("p".into(), -3.0, 3.0);
    op.set_strategy(PocketStrategy::Adaptive);
    let toolpaths = PocketGenerator::new(op).generate_polygon_pocket(&square(), 1.0);
    assert_eq!(toolpaths.len(), 3, "one toolpath per Z pass");
    for toolpath in &toolpaths {
        assert_eq!(cleanups(toolpath), 1);
    }
}

/// A 1e-6 mm stepover and step-down still return, with a bounded number of passes.
#[test]
fn test_pocket_tiny_steps_are_bounded() {
    for strategy in [PocketStrategy::ContourParallel, PocketStrategy::Adaptive] {
        let mut op = PocketOperation::new("p".into(), -0.05, 3.0);
        op.set_parameters(1e-6, 100.0, 10000);
        op.set_strategy(strategy);
        let started = std::time::Instant::now();
        let toolpaths = PocketGenerator::new(op).generate_polygon_pocket(&square(), 1e-6);
        assert!(toolpaths.len() <= 5, "{} Z passes", toolpaths.len());
        assert!(
            started.elapsed().as_secs() < 60,
            "took {:?}",
            started.elapsed()
        );
    }
}
