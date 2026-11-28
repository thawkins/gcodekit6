//! Tabbed box panel gap: the gap between neighbouring panels grows with the burn (tool
//! diameter) so that the cuts of neighbouring panels cannot overlap, and the default 0.1 mm
//! burn keeps the historical 5 mm gap.

use gcodekit6_camtools::tabbed_box::{BoxParameters, MIN_PANEL_GAP_MM, TabbedBoxMaker, panel_gap};

/// The bounding box (min x, min y, max x, max y) of every generated path.
fn bounding_boxes(p: BoxParameters) -> Vec<(f64, f64, f64, f64)> {
    let mut maker = TabbedBoxMaker::new(p).expect("valid parameters");
    maker.generate().expect("generates");
    maker
        .paths()
        .iter()
        .map(|path| {
            path.iter().fold(
                (
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                ),
                |(x0, y0, x1, y1), pt| (x0.min(pt.x), y0.min(pt.y), x1.max(pt.x), y1.max(pt.y)),
            )
        })
        .collect()
}

#[test]
fn test_tabbed_box_panel_gap_default_burn_keeps_five_mm() {
    assert_eq!(panel_gap(BoxParameters::default().burn), MIN_PANEL_GAP_MM);
    assert_eq!(panel_gap(0.0), 5.0);
    assert_eq!(panel_gap(2.0), 5.0);
}

#[test]
fn test_tabbed_box_panel_gap_grows_with_tool_diameter() {
    assert_eq!(panel_gap(3.0), 7.0);
    assert_eq!(panel_gap(6.0), 13.0);
}

/// With a 3 mm router bit and the optimised layout, the tool-centre outlines of every two
/// panels are at least one tool diameter apart on one axis, so the cuts cannot meet.
#[test]
fn test_tabbed_box_panel_gap_three_mm_bit_cuts_do_not_overlap() {
    let burn = 3.0;
    let p = BoxParameters {
        burn,
        optimize_layout: true,
        ..BoxParameters::default()
    };
    let boxes = bounding_boxes(p);
    assert_eq!(boxes.len(), 6, "one outline per panel");
    for (i, a) in boxes.iter().enumerate() {
        for b in &boxes[i + 1..] {
            let gap_x = (b.0 - a.2).max(a.0 - b.2);
            let gap_y = (b.1 - a.3).max(a.1 - b.3);
            assert!(
                gap_x.max(gap_y) >= burn,
                "panels {a:?} and {b:?} are {gap_x} / {gap_y} mm apart, under {burn} mm"
            );
        }
    }
}
