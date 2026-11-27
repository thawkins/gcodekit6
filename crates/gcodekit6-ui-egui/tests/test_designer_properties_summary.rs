//! The panel's selection summary: the one selected shape is the first shape's copy (the
//! selection is copied once per frame), and there is none when several are selected.

use gcodekit6_designer::model::{DesignRectangle, Shape};
use gcodekit6_ui_egui::panes::designer::properties::update::Summary;

use crate::test_designer_properties::design;

/// Two rectangles, the first `select` of them selected.
fn rects(select: usize) -> Summary {
    let designer = design(
        vec![
            Shape::Rectangle(DesignRectangle::new(10.0, 10.0, 20.0, 10.0)),
            Shape::Rectangle(DesignRectangle::new(50.0, 10.0, 20.0, 10.0)),
        ],
        select,
    );
    let pane = designer.borrow();
    Summary::of(pane.state()).expect("a selection")
}

#[test]
fn test_designer_summary_single_is_the_first_shape() {
    let summary = rects(1);
    let single = summary.single().expect("one shape selected");
    assert!(std::ptr::eq(single, &summary.first.shape));
    assert!(matches!(single, Shape::Rectangle(_)));
    assert_eq!(summary.size, (20.0, 10.0));
}

#[test]
fn test_designer_summary_has_no_single_for_several_shapes() {
    let summary = rects(2);
    assert!(summary.single().is_none());
    assert_eq!(summary.ids.len(), 2);
    assert!(!summary.rotated());
}
