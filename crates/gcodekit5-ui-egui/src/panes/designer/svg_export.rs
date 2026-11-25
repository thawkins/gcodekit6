//! The design as an SVG document (File > Export SVG).
//!
//! Ported from the GTK `designer/file_ops.rs` `export_svg`: the page is the shapes' effective
//! bounds plus 10 mm, in millimetres, with Y flipped (the design's Y points up, SVG's down).
//! Rectangles, circles, ellipses and lines are SVG elements with their rotation; text is a
//! `<text>` element (its content XML-escaped, which GTK did not do); a raster image is a grey
//! box; every other shape is its rendered outline as a `<path>`. GTK added a second rotation
//! to paths whose rendered outline already carries it; the outline is used as rendered here.
//!
//! Dependencies: `gcodekit5-designer` (model), `lyon` (path events).

use gcodekit5_designer::canvas::DrawingObject;
use gcodekit5_designer::model::{DesignerShape, Shape};
use lyon::path::Path;

/// The stroke every outline is drawn with.
const STYLE: &str = "fill:none;stroke:black;stroke-width:0.5";
/// Space around the shapes, mm.
const PADDING: f64 = 10.0;

/// The SVG document of `objects`, or `None` when there are none.
///
/// # Arguments
/// * `objects` - The shapes to export, in draw order.
pub fn design_svg(objects: &[&DrawingObject]) -> Option<String> {
    let shapes: Vec<Shape> = objects.iter().map(|o| o.get_effective_shape()).collect();
    let (min_x, min_y, max_x, max_y) = shapes
        .iter()
        .map(DesignerShape::bounds)
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))?;
    let (min_x, min_y, max_x, max_y) = (
        min_x - PADDING,
        min_y - PADDING,
        max_x + PADDING,
        max_y + PADDING,
    );
    let (width, height) = (max_x - min_x, max_y - min_y);
    let flip = |y: f64| (min_y + max_y) - y;
    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n\
         <svg width=\"{width:.2}mm\" height=\"{height:.2}mm\" \
         viewBox=\"{min_x:.2} {:.2} {width:.2} {height:.2}\" \
         xmlns=\"http://www.w3.org/2000/svg\">\n",
        flip(max_y)
    );
    for shape in &shapes {
        svg.push_str(&element(shape, &flip));
        svg.push('\n');
    }
    svg.push_str("</svg>\n");
    Some(svg)
}

/// One shape's SVG element; `flip` maps a design Y to an SVG Y.
fn element(shape: &Shape, flip: &dyn Fn(f64) -> f64) -> String {
    match shape {
        Shape::Rectangle(r) => {
            let top = flip(r.center.y + r.height / 2.0);
            format!(
                "<rect x=\"{:.2}\" y=\"{top:.2}\" width=\"{:.2}\" height=\"{:.2}\" rx=\"{:.2}\" \
                 style=\"{STYLE}\" transform=\"rotate({:.2} {:.2} {:.2})\" />",
                r.center.x - r.width / 2.0,
                r.width,
                r.height,
                r.effective_corner_radius(),
                -r.rotation,
                r.center.x,
                flip(r.center.y)
            )
        }
        Shape::Circle(c) => format!(
            "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"{:.2}\" style=\"{STYLE}\" />",
            c.center.x,
            flip(c.center.y),
            c.radius
        ),
        Shape::Ellipse(e) => {
            let (cx, cy) = (e.center.x, flip(e.center.y));
            format!(
                "<ellipse cx=\"{cx:.2}\" cy=\"{cy:.2}\" rx=\"{:.2}\" ry=\"{:.2}\" style=\"{STYLE}\" \
                 transform=\"rotate({:.2} {cx:.2} {cy:.2})\" />",
                e.rx, e.ry, -e.rotation
            )
        }
        Shape::Line(l) => {
            let (x1, y1, x2, y2) = (l.start.x, flip(l.start.y), l.end.x, flip(l.end.y));
            format!(
                "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" style=\"{STYLE}\" \
                 transform=\"rotate({:.2} {:.2} {:.2})\" />",
                -l.rotation,
                (x1 + x2) / 2.0,
                (y1 + y2) / 2.0
            )
        }
        Shape::Text(t) => {
            let y = flip(t.y);
            format!(
                "<text x=\"{:.2}\" y=\"{y:.2}\" font-size=\"{:.2}\" style=\"fill:black;stroke:none\" \
                 transform=\"rotate({:.2} {:.2} {y:.2})\">{}</text>",
                t.x,
                t.font_size,
                -t.rotation,
                t.x,
                escape(&t.text)
            )
        }
        Shape::RasterImage(r) => {
            let (x1, y1, x2, y2) = r.bounds();
            format!(
                "<rect x=\"{x1:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" style=\"{STYLE}\" \
                 fill=\"gray\" stroke=\"black\" stroke-width=\"0.5\" />",
                flip(y2),
                x2 - x1,
                flip(y1) - flip(y2)
            )
        }
        Shape::Path(_)
        | Shape::Triangle(_)
        | Shape::Polygon(_)
        | Shape::Gear(_)
        | Shape::Sprocket(_) => format!(
            "<path d=\"{}\" style=\"{STYLE}\" />",
            path_data(&shape.render(), flip)
        ),
    }
}

/// The `d` attribute of `path` with Y flipped.
fn path_data(path: &Path, flip: &dyn Fn(f64) -> f64) -> String {
    let xy = |p: lyon::math::Point| format!("{:.2} {:.2}", p.x, flip(f64::from(p.y)));
    let mut d = String::new();
    for event in path.iter() {
        let part = match event {
            lyon::path::Event::Begin { at } => format!("M {} ", xy(at)),
            lyon::path::Event::Line { to, .. } => format!("L {} ", xy(to)),
            lyon::path::Event::Quadratic { ctrl, to, .. } => {
                format!("Q {} {} ", xy(ctrl), xy(to))
            }
            lyon::path::Event::Cubic {
                ctrl1, ctrl2, to, ..
            } => format!("C {} {} {} ", xy(ctrl1), xy(ctrl2), xy(to)),
            lyon::path::Event::End { close: true, .. } => "Z ".to_string(),
            lyon::path::Event::End { close: false, .. } => String::new(),
        };
        d.push_str(&part);
    }
    d.trim_end().to_string()
}

/// `text` with the XML special characters escaped.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
