//! Designer Text tool: a click opens the Add Text dialog for the snapped point; Add or Enter
//! places the text (one undo step, selected), Cancel, Escape or empty text place nothing;
//! the dialog remembers the size; the placed text is edited in the properties panel; text
//! is painted as glyph outlines.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit6_designer::model::{DesignText, DesignerShape, Shape};
use gcodekit6_designer::toolpath::{ToolpathGenerator, ToolpathSegment, ToolpathSegmentType};
use gcodekit6_ui_egui::panes::designer::DesignerPane;
use gcodekit6_ui_egui::panes::designer::canvas::DesignerTool;
use gcodekit6_ui_egui::panes::designer::canvas::text_outline::text_path;
use gcodekit6_ui_egui::panes::designer::canvas::text_tool::{
    MM_PER_PT, format_points, parse_points_mm,
};

use crate::test_designer_canvas::{click, close, harness, ids, press, screen, selected};
use crate::test_designer_properties::{both, field_text, set_field};

/// Clicks world `(x, y)` with the Text tool.
fn click_text_tool(h: &mut Harness<'_, DesignerPane>, x: f32, y: f32) {
    h.state_mut().set_tool(DesignerTool::Text);
    let pos = screen(h, x, y);
    click(h, pos);
}

/// Types `text` into the focused field.
fn type_in(h: &mut Harness<'_, DesignerPane>, text: &str) {
    h.event(egui::Event::Text(text.to_string()));
    h.run();
}

/// The text shapes of the design.
fn texts(pane: &DesignerPane) -> Vec<DesignText> {
    pane.state()
        .canvas
        .shapes()
        .filter_map(|o| match &o.shape {
            Shape::Text(t) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn test_text_tool_click_add_places_selected_text_at_snapped_point() {
    let mut h = harness(DesignerPane::new());
    click_text_tool(&mut h, 40.3, 60.2);
    assert!(h.state().text_dialog_open());
    h.get_by_label("Add Text");
    type_in(&mut h, "GCode");
    h.get_by_label("Add").click();
    h.run();
    assert!(!h.state().text_dialog_open());
    let placed = texts(h.state());
    assert_eq!(placed.len(), 1);
    let t = &placed[0];
    assert_eq!(t.text, "GCode");
    // Snapped to the 1 mm step; the glyph box is centred there.
    assert!(close(t.x, 40.0) && close(t.y, 60.0), "({}, {})", t.x, t.y);
    let (x0, y0, x1, y1) = t.bounds();
    assert!(close((x0 + x1) / 2.0, 40.0) && close((y0 + y1) / 2.0, 60.0));
    assert!(close(t.font_size, 20.0 * MM_PER_PT), "GTK's 20 pt default");
    assert_eq!(
        selected(h.state()),
        ids(h.state()),
        "the new text is selected"
    );
    h.state_mut().undo();
    assert!(texts(h.state()).is_empty(), "one undo step");
}

#[test]
fn test_text_tool_enter_adds_and_cancel_escape_or_empty_add_nothing() {
    let mut h = harness(DesignerPane::new());
    click_text_tool(&mut h, 50.0, 50.0);
    type_in(&mut h, "A");
    press(&mut h, egui::Key::Enter);
    assert_eq!(texts(h.state()).len(), 1, "Enter adds");
    click_text_tool(&mut h, 80.0, 50.0);
    type_in(&mut h, "B");
    h.get_by_label("Cancel").click();
    h.run();
    click_text_tool(&mut h, 80.0, 50.0);
    type_in(&mut h, "C");
    press(&mut h, egui::Key::Escape);
    assert!(!h.state().text_dialog_open());
    click_text_tool(&mut h, 80.0, 50.0);
    type_in(&mut h, "   ");
    h.get_by_label("Add").click();
    h.run();
    assert_eq!(texts(h.state()).len(), 1, "only the first text was placed");
}

#[test]
fn test_text_tool_remembers_size_and_style() {
    let mut h = harness(DesignerPane::new());
    click_text_tool(&mut h, 50.0, 50.0);
    type_in(&mut h, "A");
    let size_field = h
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Size:")
        .rect()
        .center();
    click(&mut h, size_field);
    h.key_combination_modifiers(egui::Modifiers::COMMAND, &[egui::Key::A]);
    type_in(&mut h, "12");
    h.get_by_label("Bold").click();
    h.run();
    h.get_by_label("Add").click();
    h.run();
    let t = &texts(h.state())[0];
    assert!(close(t.font_size, 12.0 * MM_PER_PT) && t.bold);
    click_text_tool(&mut h, 90.0, 50.0);
    type_in(&mut h, "B");
    h.get_by_label("Add").click();
    h.run();
    let t = &texts(h.state())[1];
    assert!(close(t.font_size, 12.0 * MM_PER_PT) && t.bold, "remembered");
}

#[test]
fn test_placed_text_is_edited_in_the_properties_panel() {
    let designer = std::rc::Rc::new(std::cell::RefCell::new(DesignerPane::new()));
    let mut h = both(std::rc::Rc::clone(&designer));
    designer.borrow_mut().set_tool(DesignerTool::Text);
    let pos = designer
        .borrow()
        .view()
        .world_to_screen(glam::Vec2::new(50.0, 50.0));
    click(&mut h, pos);
    h.event(egui::Event::Text("Hi".to_string()));
    h.run();
    h.get_by_label("Add").click();
    h.run();
    assert_eq!(field_text(&h, "Content:"), "Hi");
    assert_eq!(field_text(&h, "Size:"), "20.00");
    set_field(&mut h, "Content:", "Hello");
    assert_eq!(texts(&designer.borrow())[0].text, "Hello");
}

#[test]
fn test_points_round_trip_and_reject_bad_sizes() {
    assert_eq!(format_points(12.0 * MM_PER_PT), "12.00");
    assert!(parse_points_mm("12pt").is_some_and(|mm| close(mm, 12.0 * MM_PER_PT)));
    assert!(parse_points_mm("10,5").is_some_and(|mm| close(mm, 10.5 * MM_PER_PT)));
    assert!(parse_points_mm("0").is_none());
    assert!(parse_points_mm("-3").is_none());
    assert!(parse_points_mm("big").is_none());
}

#[test]
fn test_text_is_painted_as_glyph_outlines_inside_its_bounds() {
    let mut text = DesignText::new("Ag".to_string(), 30.0, 20.0, 10.0);
    // A family no system has: the model's bundled font, the same on every machine.
    text.font_family = "GCodeKit6 Bundled Fallback".to_string();
    // (The text snapshot relies on this: the fallback is the bundled font.)
    assert!(std::ptr::eq(
        gcodekit6_designer::font_manager::get_font_for(&text.font_family, false, false),
        gcodekit6_designer::font_manager::get_font(),
    ));
    let path = text_path(&text).expect("glyphs");
    let (x0, y0, x1, y1) = text.bounds();
    let mut contours = 0;
    for event in path.iter() {
        match event {
            lyon::path::Event::Begin { at } => {
                contours += 1;
                let (x, y) = (f64::from(at.x), f64::from(at.y));
                assert!(x >= x0 - 0.1 && x <= x1 + 0.1 && y >= y0 - 0.1 && y <= y1 + 0.1);
            }
            lyon::path::Event::Line { .. }
            | lyon::path::Event::Quadratic { .. }
            | lyon::path::Event::Cubic { .. }
            | lyon::path::Event::End { .. } => {}
        }
    }
    // "A" has an outer and an inner contour, "g" at least one more.
    assert!(contours >= 3, "{contours} contours");
    assert!(text_path(&DesignText::new(" ".to_string(), 0.0, 0.0, 10.0)).is_none());
}

/// Rotated text with descenders is cut where it is painted: each contour of the G-code
/// starts where the painter's matching contour starts, and each glyph's G-code origin is the
/// layout's glyph origin (P2DES-3).
#[test]
fn test_rotated_text_gcode_glyphs_match_the_painted_glyphs() {
    let mut text = DesignText::new("gyp".to_string(), 30.0, 20.0, 10.0);
    text.font_family = "GCodeKit6 Bundled Fallback".to_string();
    text.rotation = 30.0;

    let painted: Vec<(f64, f64)> = text_path(&text)
        .expect("glyphs")
        .iter()
        .filter_map(|event| match event {
            lyon::path::Event::Begin { at } => Some((f64::from(at.x), f64::from(at.y))),
            lyon::path::Event::Line { .. }
            | lyon::path::Event::Quadratic { .. }
            | lyon::path::Event::Cubic { .. }
            | lyon::path::Event::End { .. } => None,
        })
        .collect();

    let toolpaths = ToolpathGenerator::new().generate_text_toolpath(&text, 0.0);
    let segments = &toolpaths.first().expect("a pass").segments;
    let is_rapid = |s: &ToolpathSegment| s.segment_type == ToolpathSegmentType::RapidMove;
    // A rapid followed by a cut starts a contour; a rapid followed by a rapid goes to the
    // next glyph's origin.
    let mut contour_starts = Vec::new();
    let mut glyph_origins = Vec::new();
    for pair in segments.windows(2) {
        if is_rapid(&pair[0]) {
            let end = (pair[0].end.x, pair[0].end.y);
            if is_rapid(&pair[1]) {
                glyph_origins.push(end);
            } else {
                contour_starts.push(end);
            }
        }
    }

    let near = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).hypot(a.1 - b.1) < 0.01;
    assert_eq!(painted.len(), contour_starts.len());
    for (p, g) in painted.iter().zip(&contour_starts) {
        assert!(near(*p, *g), "painted {p:?} vs G-code {g:?}");
    }

    let layout = text.layout().expect("layout");
    let expected: Vec<(f64, f64)> = layout
        .glyphs
        .iter()
        .skip(1)
        .map(|g| {
            let o = layout.glyph_origin(g);
            (o.x, o.y)
        })
        .collect();
    assert_eq!(glyph_origins.len(), expected.len());
    for (g, e) in glyph_origins.iter().zip(&expected) {
        assert!(near(*g, *e), "G-code origin {g:?} vs layout {e:?}");
    }
}

#[test]
fn test_enter_in_the_size_field_adds_the_text() {
    let mut h = harness(DesignerPane::new());
    click_text_tool(&mut h, 50.0, 50.0);
    type_in(&mut h, "Z");
    let size_field = h
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Size:")
        .rect()
        .center();
    click(&mut h, size_field);
    h.key_combination_modifiers(egui::Modifiers::COMMAND, &[egui::Key::A]);
    type_in(&mut h, "30");
    press(&mut h, egui::Key::Enter);
    assert!(!h.state().text_dialog_open());
    let placed = texts(h.state());
    assert_eq!(placed.len(), 1);
    assert!(close(placed[0].font_size, 30.0 * MM_PER_PT));
}
