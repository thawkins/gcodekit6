//! Locking tests for the T35 designer hygiene pass: the shared
//! `ToolpathToGcode` constructor, the logged `safe_parallel_offset` helper and
//! the DXF `$EXTMIN`/`$EXTMAX` header extents.

use cavalier_contours::polyline::{PlineSource, PlineSourceMut, PlineVertex, Polyline};
use gcodekit5_core::Units;
use gcodekit5_designer::ToolpathToGcode;
use gcodekit5_designer::model::Point;
use gcodekit5_designer::ops::safe_parallel_offset;
use gcodekit5_designer::{DxfParser, Toolpath, ToolpathSegment, ToolpathSegmentType};

/// A two-segment cutting toolpath.
fn sample_toolpath() -> Toolpath {
    let mut tp = Toolpath::new(3.0, -1.0);
    for (a, b) in [((0.0, 0.0), (10.0, 0.0)), ((10.0, 0.0), (10.0, 10.0))] {
        tp.add_segment(ToolpathSegment::new(
            ToolpathSegmentType::LinearMove,
            Point::new(a.0, a.1),
            Point::new(b.0, b.1),
            600.0,
            12000,
        ));
    }
    tp
}

/// Returns the public settings of a generator for comparison.
fn settings(g: &ToolpathToGcode) -> String {
    format!(
        "{} {} {} {} {} {} {} {:?}",
        g.safe_z,
        g.num_axes,
        g.is_laser_2d,
        g.min_point_distance,
        g.curve_simplification_tolerance,
        g.machine_limits.is_some(),
        g.continuous_z_between_passes,
        g.z_min_limit
    )
}

#[test]
fn test_with_line_numbers_matches_new_apart_from_numbering() {
    let plain = ToolpathToGcode::new(Units::MM, 7.5);
    let off = ToolpathToGcode::with_line_numbers(Units::MM, 7.5, false);
    let on = ToolpathToGcode::with_line_numbers(Units::MM, 7.5, true);
    assert_eq!(settings(&plain), settings(&off));
    assert_eq!(settings(&plain), settings(&on));

    let tp = sample_toolpath();
    assert_eq!(plain.generate(&tp), off.generate(&tp));
    let numbered = on.generate(&tp);
    assert!(numbered.lines().any(|l| l.starts_with('N')));
    assert!(!plain.generate(&tp).lines().any(|l| l.starts_with('N')));
}

/// A closed CCW 10 x 10 square.
fn square() -> Polyline {
    let mut pline = Polyline::new();
    for (x, y) in [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)] {
        pline.add_vertex(PlineVertex::new(x, y, 0.0));
    }
    pline.set_is_closed(true);
    pline
}

#[test]
fn test_safe_parallel_offset_matches_cavalier_offset() {
    let pline = square();
    for distance in [1.0, -1.0, 2.5] {
        let expected = pline.parallel_offset(distance);
        let actual = safe_parallel_offset(&pline, distance);
        assert_eq!(actual.len(), expected.len());
        for (a, e) in actual.iter().zip(expected.iter()) {
            assert_eq!(a.vertex_count(), e.vertex_count());
            for i in 0..a.vertex_count() {
                let (va, ve) = (a.at(i), e.at(i));
                assert_eq!((va.x, va.y, va.bulge), (ve.x, ve.y, ve.bulge));
            }
        }
    }
}

/// A minimal DXF with the given HEADER body and one LINE entity.
fn dxf(header: &str) -> String {
    format!(
        "0\nSECTION\n2\nHEADER\n{header}0\nENDSEC\n0\nSECTION\n2\nENTITIES\n\
         0\nLINE\n8\n0\n10\n1\n20\n2\n11\n3\n21\n4\n0\nENDSEC\n0\nEOF\n"
    )
}

#[test]
fn test_dxf_header_extents_are_parsed() {
    let content = dxf("9\n$EXTMIN\n10\n5.0\n20\n10.0\n30\n0.0\n\
         9\n$EXTMAX\n10\n25.0\n20\n40.0\n30\n0.0\n");
    let file = DxfParser::parse(&content).expect("dxf parses");
    let (min, max) = file.bounds();
    assert_eq!((min.x, min.y, max.x, max.y), (5.0, 10.0, 25.0, 40.0));
    assert_eq!(file.entity_count(), 1);
    assert_eq!(file.replaced_numbers, 0);
}

#[test]
fn test_dxf_extents_default_when_missing_or_empty_drawing() {
    let file = DxfParser::parse(&dxf("")).expect("dxf parses");
    let (min, max) = file.bounds();
    assert_eq!((min.x, min.y, max.x, max.y), (0.0, 0.0, 100.0, 100.0));

    // An empty drawing stores an inverted box; keep the defaults.
    let content = dxf("9\n$EXTMIN\n10\n1e20\n20\n1e20\n30\n1e20\n\
         9\n$EXTMAX\n10\n-1e20\n20\n-1e20\n30\n-1e20\n");
    let file = DxfParser::parse(&content).expect("dxf parses");
    let (min, max) = file.bounds();
    assert_eq!((min.x, min.y, max.x, max.y), (0.0, 0.0, 100.0, 100.0));
}
