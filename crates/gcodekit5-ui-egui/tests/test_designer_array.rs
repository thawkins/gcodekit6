//! Designer Array dialog: the context menu's "Array..." opens it; Create copies the selection
//! in a line, a grid or around a centre as one undo step; invalid values keep it open with
//! an error. The parsing and the size limit are tested directly.

use egui::{Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_core::units::MeasurementSystem;
use gcodekit5_designer::model::DesignerShape;
use gcodekit5_ui_egui::panes::designer::DesignerPane;
use gcodekit5_ui_egui::panes::designer::canvas::array_dialog::{
    ArrayInput, ArrayKind, ArraySpec, MAX_ARRAY_OBJECTS, check_array_size, parse_array_input,
};

use crate::test_designer_canvas::{click, click_with, harness, ids, pane_with_rects, screen};

/// The default field texts (GTK's), with `kind`.
fn input(kind: ArrayKind) -> ArrayInput {
    ArrayInput {
        kind,
        ..ArrayInput::default()
    }
}

#[test]
fn test_parse_array_input_reads_each_kind() {
    let metric = MeasurementSystem::Metric;
    assert_eq!(
        parse_array_input(&input(ArrayKind::Linear), metric),
        Ok(ArraySpec::Linear {
            count: 3,
            dx: 10.0,
            dy: 0.0
        })
    );
    assert_eq!(
        parse_array_input(&input(ArrayKind::Grid), metric),
        Ok(ArraySpec::Grid {
            rows: 2,
            cols: 2,
            dx: 10.0,
            dy: 0.0
        })
    );
    assert_eq!(
        parse_array_input(&input(ArrayKind::Circular), metric),
        Ok(ArraySpec::Circular {
            count: 3,
            cx: 0.0,
            cy: 0.0,
            angle: 360.0
        })
    );
    // Lengths are read in the display units.
    let inches = ArrayInput {
        dx: "1".into(),
        ..input(ArrayKind::Linear)
    };
    match parse_array_input(&inches, MeasurementSystem::Imperial) {
        Ok(ArraySpec::Linear { dx, .. }) => assert!((dx - 25.4).abs() < 1e-3, "{dx}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn test_parse_array_input_rejects_bad_values() {
    let metric = MeasurementSystem::Metric;
    for bad in [
        ArrayInput {
            count: "1".into(),
            ..input(ArrayKind::Linear)
        },
        ArrayInput {
            count: "two".into(),
            ..input(ArrayKind::Linear)
        },
        ArrayInput {
            dx: "abc".into(),
            ..input(ArrayKind::Linear)
        },
        ArrayInput {
            rows: "0".into(),
            ..input(ArrayKind::Grid)
        },
        ArrayInput {
            angle: "0".into(),
            ..input(ArrayKind::Circular)
        },
    ] {
        assert!(parse_array_input(&bad, metric).is_err(), "{bad:?}");
    }
}

#[test]
fn test_array_size_is_limited() {
    let ok = ArraySpec::Grid {
        rows: 100,
        cols: 100,
        dx: 1.0,
        dy: 1.0,
    };
    assert!(check_array_size(&ok, 1).is_ok());
    assert!(check_array_size(&ok, 2).is_err());
    let huge = ArraySpec::Linear {
        count: MAX_ARRAY_OBJECTS + 1,
        dx: 1.0,
        dy: 0.0,
    };
    assert!(check_array_size(&huge, 1).is_err());
}

/// A pane with one 20 x 10 mm rectangle at (40, 40), selected, and the Array dialog opened
/// from its context menu.
fn array_dialog_open() -> Harness<'static, DesignerPane> {
    let mut h = harness(pane_with_rects(&[(40.0, 40.0, 20.0, 10.0)]));
    let p = screen(&h, 50.0, 45.0);
    click(&mut h, p);
    assert_eq!(h.state().state().selected_count(), 1);
    click_with(&mut h, p, PointerButton::Secondary, Modifiers::NONE);
    h.get_by_label("Array...").click();
    h.run();
    assert!(h.state().array_dialog_open());
    h
}

/// Replaces the text of the field labelled `label` with `text`.
fn set_field(h: &mut Harness<'_, DesignerPane>, label: &str, text: &str) {
    let field = h.get_by_role_and_label(egui::accesskit::Role::TextInput, label);
    field.focus();
    h.run();
    h.key_combination_modifiers(Modifiers::COMMAND, &[egui::Key::A]);
    h.event(egui::Event::Text(text.to_string()));
    h.run();
}

#[test]
fn test_array_dialog_creates_n_copies_as_one_undo_step() {
    let mut h = array_dialog_open();
    set_field(&mut h, "Count", "4");
    set_field(&mut h, "Spacing X", "30");
    h.get_by_label("Create").click();
    h.run();
    assert!(!h.state().array_dialog_open());
    assert_eq!(
        ids(h.state()).len(),
        4,
        "count 4 = the original and 3 copies"
    );
    let mut lefts: Vec<f64> = h
        .state()
        .state()
        .canvas
        .shapes()
        .map(|o| o.shape.bounds().0.round())
        .collect();
    lefts.sort_by(f64::total_cmp);
    assert_eq!(lefts, [40.0, 70.0, 100.0, 130.0]);
    assert!(h.state().is_modified());
    h.state_mut().undo();
    h.run();
    assert_eq!(ids(h.state()).len(), 1, "one undo removes every copy");
}

#[test]
fn test_array_dialog_grid_and_circular() {
    let mut h = array_dialog_open();
    h.get_by_label("Grid").click();
    h.run();
    set_field(&mut h, "Rows", "2");
    set_field(&mut h, "Columns", "3");
    set_field(&mut h, "Spacing Y", "20");
    h.get_by_label("Create").click();
    h.run();
    assert_eq!(ids(h.state()).len(), 6);

    let mut h = array_dialog_open();
    h.get_by_label("Circular").click();
    h.run();
    set_field(&mut h, "Count", "4");
    set_field(&mut h, "Centre X", "50");
    set_field(&mut h, "Centre Y", "0");
    h.get_by_label("Create").click();
    h.run();
    assert_eq!(ids(h.state()).len(), 4);
}

#[test]
fn test_array_dialog_keeps_invalid_values_open_and_cancel_adds_nothing() {
    let mut h = array_dialog_open();
    set_field(&mut h, "Count", "1");
    h.get_by_label("Create").click();
    h.run();
    assert!(h.state().array_dialog_open(), "invalid values keep it open");
    assert!(
        h.query_by_label_contains("Enter a whole number of 2 or more")
            .is_some()
    );
    assert_eq!(ids(h.state()).len(), 1);
    h.get_by_label("Cancel").click();
    h.run();
    assert!(!h.state().array_dialog_open());
    assert_eq!(ids(h.state()).len(), 1);
}
