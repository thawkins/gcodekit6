//! Designer properties panel, common sections: position, size and lock aspect, rotation,
//! commit on Enter or focus loss (one undo step per edit, nothing per keystroke), invalid
//! text, a selection change during an edit, multi-selection, display units, keys while a
//! field has focus, and the shared helpers. Shape-specific sections are in
//! `test_designer_properties_shapes.rs`, CAM/laser/image in
//! `test_designer_properties_cam.rs`.

use std::cell::RefCell;
use std::rc::Rc;

use egui::Pos2;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use gcodekit6_app::i18n::Translator;
use gcodekit6_core::units::MeasurementSystem;
use gcodekit6_designer::model::{DesignerShape, Shape};
use gcodekit6_ui_egui::panes::designer::DesignerPane;
use gcodekit6_ui_egui::panes::designer::properties::PropertiesPane;
use gcodekit6_ui_egui::workspace::{Pane, PaneContext};

use crate::test_designer_canvas::{STEP_DT, click, close, ids, pane_with_rects, press, selected};

/// The Designer and its Properties pane side by side, over one shared design.
pub struct Both {
    pub designer: Rc<RefCell<DesignerPane>>,
    pub props: PropertiesPane,
}

/// A design holding `shapes`, the first `select` of them selected.
pub fn design(shapes: Vec<Shape>, select: usize) -> Rc<RefCell<DesignerPane>> {
    let mut pane = DesignerPane::new();
    let mut added = Vec::new();
    for shape in shapes {
        added.push(pane.state_mut().add_shape_with_undo(shape));
    }
    let state = pane.state_mut();
    state.canvas.deselect_all();
    for id in added.iter().take(select) {
        state.canvas.select_shape(*id, true);
    }
    Rc::new(RefCell::new(pane))
}

/// Switches the design to CNC mode (the model starts in laser mode).
pub fn cnc(designer: &Rc<RefCell<DesignerPane>>) {
    designer
        .borrow_mut()
        .state_mut()
        .set_machine_mode(gcodekit6_designer::designer_state::MachineMode::Cnc3D);
}

/// Two 20 x 10 mm rectangles at (10, 10) and (50, 10) with the aspect unlocked (new shapes
/// start locked), the first selected, in CNC mode.
pub fn two_rects() -> Rc<RefCell<DesignerPane>> {
    let mut pane = pane_with_rects(&[(10.0, 10.0, 20.0, 10.0), (50.0, 10.0, 20.0, 10.0)]);
    let all = ids(&pane);
    for id in &all {
        if let Some(obj) = pane.state_mut().canvas.get_shape_mut(*id) {
            obj.lock_aspect_ratio = false;
        }
    }
    pane.state_mut().canvas.select_shape(all[0], false);
    let designer = Rc::new(RefCell::new(pane));
    cnc(&designer);
    designer
}

/// Both panes in a harness at `ppp` pixels per point, without a GPU renderer.
pub fn both_at(designer: Rc<RefCell<DesignerPane>>, ppp: f32) -> Harness<'static, Both> {
    let tr = Translator::new("en");
    let props = PropertiesPane::new(Rc::clone(&designer));
    let mut h = Harness::builder()
        .with_size(egui::vec2(1000.0, 1100.0))
        .with_pixels_per_point(ppp)
        .with_step_dt(STEP_DT)
        .build_ui_state(
            move |ui, both: &mut Both| {
                let mut requested = Vec::new();
                let mut ctx = PaneContext::new(&tr, &mut requested);
                egui::Panel::right("properties")
                    .default_size(340.0)
                    .show(ui, |ui| both.props.ui(ui, &mut ctx));
                both.designer.borrow_mut().ui(ui, &mut ctx);
            },
            Both { designer, props },
        );
    h.run();
    h
}

/// Both panes at 1 pixel per point.
pub fn both(designer: Rc<RefCell<DesignerPane>>) -> Harness<'static, Both> {
    both_at(designer, 1.0)
}

/// The centre of the text field labelled `label`.
pub fn field_pos(h: &Harness<'_, Both>, label: &str) -> Pos2 {
    h.get_by_role_and_label(Role::TextInput, label)
        .rect()
        .center()
}

/// The text the field labelled `label` shows.
pub fn field_text(h: &Harness<'_, Both>, label: &str) -> String {
    h.get_by_role_and_label(Role::TextInput, label)
        .value()
        .unwrap_or_default()
}

/// Whether the field labelled `label` can be edited.
pub fn field_enabled(h: &Harness<'_, Both>, label: &str) -> bool {
    !h.get_by_role_and_label(Role::TextInput, label)
        .accesskit_node()
        .is_disabled()
}

/// Clicks the field labelled `label`, replaces its text with `text`, without committing.
pub fn type_text(h: &mut Harness<'_, Both>, label: &str, text: &str) {
    let pos = field_pos(h, label);
    click(h, pos);
    h.key_combination_modifiers(egui::Modifiers::COMMAND, &[egui::Key::A]);
    h.event(egui::Event::Text(text.to_string()));
    h.run();
}

/// Types `text` into the field labelled `label` and presses Enter.
pub fn set_field(h: &mut Harness<'_, Both>, label: &str, text: &str) {
    type_text(h, label, text);
    press(h, egui::Key::Enter);
}

/// Clicks the widget labelled `label` (a checkbox or button).
pub fn toggle(h: &mut Harness<'_, Both>, label: &str) {
    h.get_by_label(label).click();
    h.run();
}

/// Opens drop-down `index` (in panel order) and picks `option`.
pub fn pick(h: &mut Harness<'_, Both>, index: usize, option: &str) {
    let combos: Vec<Pos2> = h
        .get_all_by_role(Role::ComboBox)
        .map(|n| n.rect().center())
        .collect();
    click(h, combos[index]);
    h.get_by_label(option).click();
    h.run();
}

/// The selected shapes' copies.
pub fn shapes(h: &Harness<'_, Both>) -> Vec<Shape> {
    let d = h.state().designer.borrow();
    selected(&d)
        .into_iter()
        .filter_map(|id| d.state().canvas.get_shape(id).map(|o| o.shape.clone()))
        .collect()
}

/// The first selected shape's copy.
pub fn first(h: &Harness<'_, Both>) -> Shape {
    shapes(h).into_iter().next().expect("a selected shape")
}

/// The number of undo steps.
pub fn undo_depth(h: &Harness<'_, Both>) -> usize {
    let mut copy = h.state().designer.borrow().state().clone();
    let mut depth = 0;
    while copy.can_undo() {
        copy.undo();
        depth += 1;
    }
    depth
}

/// Undoes one step.
pub fn undo(h: &mut Harness<'_, Both>) {
    h.state().designer.borrow_mut().undo();
    h.run();
}

/// The width and height of the first selected rectangle.
fn rect_size(h: &Harness<'_, Both>) -> (f64, f64) {
    match first(h) {
        Shape::Rectangle(r) => (r.width, r.height),
        other => panic!("not a rectangle: {other:?}"),
    }
}

#[test]
fn test_nothing_selected_shows_the_hint() {
    let h = both(Rc::new(RefCell::new(pane_with_rects(&[(
        10.0, 10.0, 20.0, 10.0,
    )]))));
    h.get_by_label("Select a shape to edit its properties");
    assert!(h.query_by_label("Width:").is_none());
}

#[test]
fn test_property_edit_updates_shape() {
    let designer = two_rects();
    let mut h = both(Rc::clone(&designer));
    designer.borrow_mut().state_mut().is_modified = false;
    let before = undo_depth(&h);
    assert_eq!(field_text(&h, "Width:"), "20.00");
    set_field(&mut h, "Width:", "40");
    assert!(close(rect_size(&h).0, 40.0), "{:?}", rect_size(&h));
    assert!(close(rect_size(&h).1, 10.0));
    assert_eq!(undo_depth(&h), before + 1, "one undo step");
    assert!(designer.borrow().is_modified(), "design marked modified");
    assert_eq!(field_text(&h, "Width:"), "40.00");
    undo(&mut h);
    assert!(close(rect_size(&h).0, 20.0));
}

#[test]
fn test_typing_changes_nothing_until_commit_and_focus_loss_commits() {
    let mut h = both(two_rects());
    let before = undo_depth(&h);
    type_text(&mut h, "Width:", "33");
    assert!(close(rect_size(&h).0, 20.0), "no change per keystroke");
    assert_eq!(undo_depth(&h), before);
    // Moving the focus to another field commits.
    let height = field_pos(&h, "Height:");
    click(&mut h, height);
    assert!(close(rect_size(&h).0, 33.0));
    assert_eq!(undo_depth(&h), before + 1);
    // Leaving a field unchanged records nothing.
    let width = field_pos(&h, "Width:");
    click(&mut h, width);
    assert_eq!(undo_depth(&h), before + 1);
}

#[test]
fn test_invalid_text_applies_nothing_and_restores_the_value() {
    let mut h = both(two_rects());
    let before = undo_depth(&h);
    set_field(&mut h, "Width:", "abc");
    set_field(&mut h, "Height:", "");
    set_field(&mut h, "Width:", "-5");
    assert!(close(rect_size(&h).0, 20.0));
    assert!(close(rect_size(&h).1, 10.0));
    assert_eq!(undo_depth(&h), before);
    assert_eq!(field_text(&h, "Width:"), "20.00");
}

#[test]
fn test_position_moves_the_centre() {
    let mut h = both(two_rects());
    assert_eq!(field_text(&h, "X:"), "20.00");
    assert_eq!(field_text(&h, "Y:"), "15.00");
    set_field(&mut h, "X:", "100");
    set_field(&mut h, "Y:", "50,5");
    let (x0, y0, x1, y1) = first(&h).bounds();
    assert!(close((x0 + x1) / 2.0, 100.0) && close((y0 + y1) / 2.0, 50.5));
    assert!(close(x1 - x0, 20.0), "size kept");
}

#[test]
fn test_z_is_start_depth_in_cnc_and_read_only_in_laser() {
    let designer = two_rects();
    let mut h = both(Rc::clone(&designer));
    set_field(&mut h, "(-Z:)", "2.5");
    let depth = |d: &Rc<RefCell<DesignerPane>>| {
        let d = d.borrow();
        d.state()
            .canvas
            .shapes()
            .find(|o| o.selected)
            .map(|o| o.start_depth)
    };
    assert_eq!(depth(&designer), Some(2.5));
    designer
        .borrow_mut()
        .state_mut()
        .set_machine_mode(gcodekit6_designer::designer_state::MachineMode::Laser2D);
    h.run();
    assert_eq!(field_text(&h, "(-Z:)"), "0");
    assert!(!field_enabled(&h, "(-Z:)"));
}

#[test]
fn test_lock_aspect_keeps_the_ratio_and_is_undoable() {
    let designer = two_rects();
    let mut h = both(Rc::clone(&designer));
    let before = undo_depth(&h);
    toggle(&mut h, "Lock Aspect:");
    assert_eq!(undo_depth(&h), before + 1);
    let locked = |d: &Rc<RefCell<DesignerPane>>| {
        d.borrow()
            .state()
            .canvas
            .shapes()
            .find(|o| o.selected)
            .is_some_and(|o| o.lock_aspect_ratio)
    };
    assert!(locked(&designer));
    set_field(&mut h, "Width:", "40");
    assert!(close(rect_size(&h).0, 40.0) && close(rect_size(&h).1, 20.0));
    set_field(&mut h, "Height:", "5");
    assert!(close(rect_size(&h).0, 10.0) && close(rect_size(&h).1, 5.0));
    undo(&mut h);
    undo(&mut h);
    undo(&mut h);
    assert!(!locked(&designer));
}

#[test]
fn test_rotation_is_absolute_for_one_shape_and_locks_its_size() {
    let mut h = both(two_rects());
    assert_eq!(field_text(&h, "Angle:"), "0.0");
    set_field(&mut h, "Angle:", "30");
    assert!(close(first(&h).rotation(), 30.0));
    set_field(&mut h, "Angle:", "45");
    assert!(close(first(&h).rotation(), 45.0), "absolute, not 75");
    h.get_by_label("Rotated object: Dimensions locked");
    assert!(!field_enabled(&h, "Width:"));
    assert!(!field_enabled(&h, "Height:"));
    assert!(
        h.get_by_label("Lock Aspect:")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn test_several_shapes_edit_the_common_box_and_rotate_relatively() {
    let designer = two_rects();
    {
        let mut d = designer.borrow_mut();
        let all = ids(&d);
        d.state_mut().canvas.select_shape(all[1], true);
    }
    let mut h = both(Rc::clone(&designer));
    // The box spans x 10..70, y 10..20.
    assert_eq!(field_text(&h, "Width:"), "60.00");
    assert_eq!(field_text(&h, "X:"), "40.00");
    let before = undo_depth(&h);
    set_field(&mut h, "X:", "140");
    assert_eq!(undo_depth(&h), before + 1, "one step for both shapes");
    let xs: Vec<f64> = shapes(&h).iter().map(|s| s.bounds().0).collect();
    assert!(close(xs[0], 110.0) && close(xs[1], 150.0), "{xs:?}");
    set_field(&mut h, "Angle:", "90");
    assert_eq!(field_text(&h, "Angle:"), "0.0", "relative: back to 0");
    set_field(&mut h, "Angle:", "90");
    let rotations: Vec<f64> = shapes(&h).iter().map(Shape::rotation).collect();
    assert!(
        rotations.iter().all(|r| close(r.rem_euclid(360.0), 180.0)),
        "{rotations:?}"
    );
}

#[test]
fn test_lengths_follow_the_display_units() {
    let designer = two_rects();
    designer.borrow_mut().set_units(MeasurementSystem::Imperial);
    let mut h = both(designer);
    // 20 mm is 0.79 in.
    assert_eq!(field_text(&h, "Width:"), "0.79");
    set_field(&mut h, "Width:", "1 1/2");
    assert!(close(rect_size(&h).0, 38.1), "{:?}", rect_size(&h));
}

#[test]
fn test_delete_in_a_field_edits_text_not_the_design() {
    let mut h = both(two_rects());
    let shapes_before = ids(&h.state().designer.borrow()).len();
    let pos = field_pos(&h, "Width:");
    click(&mut h, pos);
    press(&mut h, egui::Key::Backspace);
    press(&mut h, egui::Key::Delete);
    assert_eq!(ids(&h.state().designer.borrow()).len(), shapes_before);
    assert_eq!(selected(&h.state().designer.borrow()).len(), 1);
}
