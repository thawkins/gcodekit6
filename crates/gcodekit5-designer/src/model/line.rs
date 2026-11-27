//! # Line Shape
//!
//! A line segment design shape defined by start and end points.
//! Supports rotation and CSG sketch generation.

use lyon::math::{Transform, point};
use lyon::path::Path;
use serde::{Deserialize, Serialize};

use csgrs::sketch::Sketch;
use csgrs::traits::CSG;
use nalgebra::{Matrix4, Vector3};

use super::{DesignerShape, LaserParams, Point, Property, PropertyValue};
use crate::geometry::point_segment_distance;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignLine {
    pub start: Point,
    pub end: Point,
    pub rotation: f64,
    pub laser_params: LaserParams,
}

impl DesignLine {
    pub fn new(start: Point, end: Point) -> Self {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let rotation = dy.atan2(dx).to_degrees();

        Self {
            start,
            end,
            rotation,
            laser_params: LaserParams::default(),
        }
    }

    pub fn distance_to_point(&self, point: &Point) -> f64 {
        point_segment_distance(point, &self.start, &self.end, 0.0)
    }

    pub fn current_angle_degrees(&self) -> f64 {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        dy.atan2(dx).to_degrees()
    }

    pub fn rotate_about(&mut self, angle_deg: f64, cx: f64, cy: f64) {
        let angle_rad = angle_deg.to_radians();
        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();

        let rotate_point = |p: &Point| -> Point {
            let dx = p.x - cx;
            let dy = p.y - cy;
            Point::new(dx * cos_a - dy * sin_a + cx, dx * sin_a + dy * cos_a + cy)
        };

        self.start = rotate_point(&self.start);
        self.end = rotate_point(&self.end);
        self.rotation = self.current_angle_degrees();
    }

    // Obtener el rectángulo contenedor de la línea SIN aplicar rotación
    // Este es el que se muestra en los entry de dimensiones
    pub fn get_bounds_dimensions(&self) -> (f64, f64, f64, f64) {
        // Usar coordenadas originales sin rotación
        let min_x = self.start.x.min(self.end.x);
        let min_y = self.start.y.min(self.end.y);
        let max_x = self.start.x.max(self.end.x);
        let max_y = self.start.y.max(self.end.y);

        (min_x, min_y, max_x - min_x, max_y - min_y)
    }

    // Establecer las dimensiones del rectángulo contenedor manteniendo la dirección
    // Redimensionar la línea desde los entry de dimensiones
}

impl DesignerShape for DesignLine {
    fn render(&self) -> Path {
        // `start`/`end` are absolute geometry; `rotation` is derived from them
        // (see `current_angle_degrees`), so it must not be applied again here.
        let mut builder = Path::builder();
        builder.begin(point(self.start.x as f32, self.start.y as f32));
        builder.line_to(point(self.end.x as f32, self.end.y as f32));
        builder.end(false);
        builder.build()
    }

    fn as_csg(&self) -> Sketch<()> {
        // Lines have no area, so return empty sketch or degenerate polygon
        // For now, let's return a very thin rectangle to allow selection/boolean ops?
        // Or just a 2-point polygon (which might be invalid for boolean ops)
        // Let's try a thin rectangle (width 0.1mm)
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        let len = (dx * dx + dy * dy).sqrt();
        let angle = dy.atan2(dx);

        let sketch = Sketch::rectangle(len, 0.1, None);

        // Rotate and translate
        let center_x = (self.start.x + self.end.x) / 2.0;
        let center_y = (self.start.y + self.end.y) / 2.0;

        let center_fix = Matrix4::new_translation(&Vector3::new(-len / 2.0, -0.05, 0.0));
        let rotation = Matrix4::new_rotation(Vector3::new(0.0, 0.0, angle));
        let translation = Matrix4::new_translation(&Vector3::new(center_x, center_y, 0.0));

        sketch.transform(&(translation * rotation * center_fix))
    }

    fn bounds(&self) -> (f64, f64, f64, f64) {
        // Para los bounds, usar las coordenadas originales SIN rotación
        // porque la rotación ya está almacenada como propiedad
        let (x, y, width, height) = self.get_bounds_dimensions();
        (x, y, x + width, y + height)
    }

    fn transform(&mut self, t: &Transform) {
        let p1 = t.transform_point(point(self.start.x as f32, self.start.y as f32));
        self.start = Point::new(p1.x as f64, p1.y as f64);
        let p2 = t.transform_point(point(self.end.x as f32, self.end.y as f32));
        self.end = Point::new(p2.x as f64, p2.y as f64);
        self.rotation = self.current_angle_degrees(); // Update rotation based on new positions
    }

    fn properties(&self) -> Vec<Property> {
        // Obtener las dimensiones del rectángulo contenedor SIN rotación
        let (x, y, width, height) = self.get_bounds_dimensions();

        vec![
            Property {
                name: "X".to_string(),
                value: PropertyValue::Number(x),
            },
            Property {
                name: "Y".to_string(),
                value: PropertyValue::Number(y),
            },
            Property {
                name: "Width".to_string(),
                value: PropertyValue::Number(width),
            },
            Property {
                name: "Height".to_string(),
                value: PropertyValue::Number(height),
            },
            Property {
                name: "Rotation".to_string(),
                value: PropertyValue::Number(self.rotation),
            },
            Property {
                name: "Start X".to_string(),
                value: PropertyValue::Number(self.start.x),
            },
            Property {
                name: "Start Y".to_string(),
                value: PropertyValue::Number(self.start.y),
            },
            Property {
                name: "End X".to_string(),
                value: PropertyValue::Number(self.end.x),
            },
            Property {
                name: "End Y".to_string(),
                value: PropertyValue::Number(self.end.y),
            },
        ]
    }

    fn contains_point(&self, p: Point, tolerance: f64) -> bool {
        // Endpoints are absolute geometry, so test against the segment directly.
        self.distance_to_point(&p) <= tolerance
    }

    fn resize(&mut self, handle: usize, dx: f64, dy: f64) {
        match handle {
            0 => {
                // Esquina superior izquierda - mover inicio
                self.start.x += dx;
                self.start.y += dy;
                self.rotation = self.current_angle_degrees();
            }
            1 => {
                // Esquina inferior derecha - mover fin
                self.end.x += dx;
                self.end.y += dy;
                self.rotation = self.current_angle_degrees();
            }
            2 => {
                // Esquina superior derecha - mover inicio
                self.start.x += dx;
                self.start.y += dy;
                self.rotation = self.current_angle_degrees();
            }
            3 => {
                // Esquina inferior izquierda - mover fin
                self.end.x += dx;
                self.end.y += dy;
                self.rotation = self.current_angle_degrees();
            }
            4 => {
                // Centro - mover toda la línea
                self.translate(dx, dy);
            }
            _ => {}
        }
    }
}
