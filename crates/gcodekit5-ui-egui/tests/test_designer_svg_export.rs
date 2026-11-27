//! SVG export draws each shape where the canvas shows it and the G-code cuts it.

use gcodekit5_designer::canvas::DrawingObject;
use gcodekit5_designer::model::{DesignLine, DesignPath, DesignText, Shape};
use gcodekit5_ui_egui::panes::designer::svg_export::design_svg;

/// The X coordinates of every `M`/`L` point in `svg`'s path data.
fn path_xs(svg: &str) -> Vec<f64> {
    let d = svg
        .split("d=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("a path element");
    let mut xs = Vec::new();
    let mut words = d.split_whitespace();
    while let Some(word) = words.next() {
        if word == "M" || word == "L" {
            let x = words.next().and_then(|x| x.parse().ok()).expect("x");
            xs.push(x);
        }
    }
    xs
}

/// A 20 x 10 mm rectangle path rotated 90 degrees exports rotated: x spans 5..15.
#[test]
fn test_designer_svg_export_rotated_path() {
    let mut b = lyon::path::Path::builder();
    b.begin(lyon::math::point(0.0, 0.0));
    b.line_to(lyon::math::point(20.0, 0.0));
    b.line_to(lyon::math::point(20.0, 10.0));
    b.line_to(lyon::math::point(0.0, 10.0));
    b.end(true);
    let mut path = DesignPath::from_lyon_path(&b.build());
    path.rotation = 90.0;
    let object = DrawingObject::new(1, Shape::Path(path));
    let svg = design_svg(&[&object]).expect("an SVG");
    let xs = path_xs(&svg);
    assert!(!xs.is_empty());
    assert!(
        xs.iter().all(|x| (4.99..=15.01).contains(x)),
        "path not rotated: {xs:?}"
    );
}

/// `shape`'s exported element (the line after the `<svg ...>` opening tag).
fn element(shape: Shape) -> String {
    let object = DrawingObject::new(1, shape);
    let svg = design_svg(&[&object]).expect("an SVG");
    svg.lines()
        .skip_while(|line| !line.starts_with("<svg"))
        .nth(1)
        .expect("an element")
        .to_string()
}

/// A 45-degree line exports its endpoints only, with no second rotation.
#[test]
fn test_designer_svg_export_line_not_rotated_twice() {
    let line = DesignLine::new(
        gcodekit5_designer::model::Point::new(0.0, 0.0),
        gcodekit5_designer::model::Point::new(10.0, 10.0),
    );
    let element = element(Shape::Line(line));
    assert!(element.starts_with("<line "), "{element}");
    assert!(!element.contains("transform"), "{element}");
}

/// Text is anchored at its centre, as the model places it.
#[test]
fn test_designer_svg_export_text_anchored_at_centre() {
    let text = DesignText::new("Hi".to_string(), 10.0, 20.0, 5.0);
    let element = element(Shape::Text(text));
    assert!(element.contains("text-anchor:middle"), "{element}");
}

/// A raster image exports as a grey box: no inline `fill:none` overrides the fill.
#[test]
fn test_designer_svg_export_raster_box_filled() {
    let raster = gcodekit5_designer::model::RasterImage {
        width_mm: 10.0,
        height_mm: 5.0,
        ..Default::default()
    };
    let element = element(Shape::RasterImage(raster));
    assert!(element.contains("fill:gray"), "{element}");
    assert!(!element.contains("fill:none"), "{element}");
}
