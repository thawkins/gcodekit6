//! # Parametric Shape Generators
//!
//! Provides generators for complex mechanical and structural shapes.

use crate::model::Point;
use lyon::math::point;
use lyon::path::Path;
use std::f64::consts::PI;

/// Generate a spur gear path
pub fn generate_spur_gear(
    center: Point,
    module: f64,
    teeth: usize,
    pressure_angle_deg: f64,
    hole_radius: f64,
) -> Path {
    let mut builder = Path::builder();

    let pitch_radius = (module * teeth as f64) / 2.0;
    let addendum = module;
    let dedendum = 1.25 * module;
    let outer_radius = pitch_radius + addendum;
    let root_radius = pitch_radius - dedendum;
    let base_radius = pitch_radius * pressure_angle_deg.to_radians().cos();

    let angle_per_tooth = 2.0 * PI / teeth as f64;
    let _half_tooth_angle = angle_per_tooth / 4.0;

    // Involute curve calculation
    // x = r_b * (cos(t) + t * sin(t))
    // y = r_b * (sin(t) - t * cos(t))

    let mut points = Vec::new();

    for i in 0..teeth {
        let tooth_center_angle = i as f64 * angle_per_tooth;

        // We need to find the angle 't' where the involute reaches the outer radius
        // r^2 = r_b^2 * (1 + t^2) => t = sqrt((r/r_b)^2 - 1)
        let t_max = ((outer_radius / base_radius).powi(2) - 1.0).sqrt();
        let steps = 5;

        // Left side of tooth (involute)
        let mut tooth_points = Vec::new();
        for j in 0..=steps {
            let t = (j as f64 / steps as f64) * t_max;
            let x = base_radius * (t.cos() + t * t.sin());
            let y = base_radius * (t.sin() - t * t.cos());

            // The involute starts at the base circle. If root circle is smaller, we add a radial line.
            let r = (x * x + y * y).sqrt();
            let phi = y.atan2(x);

            // Rotate to tooth position
            // We need to offset the involute so the tooth has the correct thickness at the pitch circle.
            // Thickness at pitch circle is pitch_radius * half_tooth_angle * 2? No, it's circular pitch / 2.
            // Circular pitch = PI * module. Tooth thickness = PI * module / 2.
            // Angle for tooth thickness = (PI * module / 2) / pitch_radius = PI / teeth.
            // So half thickness angle = PI / (2 * teeth).

            // Angle of involute at pitch radius
            let t_pitch = ((pitch_radius / base_radius).powi(2) - 1.0).sqrt();
            let phi_pitch = (t_pitch.sin() - t_pitch * t_pitch.cos())
                .atan2(t_pitch.cos() + t_pitch * t_pitch.sin());

            let angle = tooth_center_angle - (PI / (2.0 * teeth as f64)) - phi_pitch + phi;

            tooth_points.push(Point::new(
                center.x + r * angle.cos(),
                center.y + r * angle.sin(),
            ));
        }

        // If root radius is smaller than base radius, add a point at root
        if root_radius < base_radius {
            let p0 = tooth_points[0];
            let _r0 = (p0.x - center.x).hypot(p0.y - center.y);
            let angle0 = (p0.y - center.y).atan2(p0.x - center.x);
            points.push(Point::new(
                center.x + root_radius * angle0.cos(),
                center.y + root_radius * angle0.sin(),
            ));
        }

        points.extend(tooth_points.clone());

        // Top of tooth (arc)
        // Right side of tooth (mirrored involute)
        let mut right_points = Vec::new();
        for j in (0..=steps).rev() {
            let t = (j as f64 / steps as f64) * t_max;
            let x = base_radius * (t.cos() + t * t.sin());
            let y = -(base_radius * (t.sin() - t * t.cos())); // Mirror Y

            let r = (x * x + y * y).sqrt();
            let phi = y.atan2(x);

            let t_pitch = ((pitch_radius / base_radius).powi(2) - 1.0).sqrt();
            let phi_pitch = (t_pitch.sin() - t_pitch * t_pitch.cos())
                .atan2(t_pitch.cos() + t_pitch * t_pitch.sin());

            let angle = tooth_center_angle + (PI / (2.0 * teeth as f64)) + phi_pitch + phi;

            right_points.push(Point::new(
                center.x + r * angle.cos(),
                center.y + r * angle.sin(),
            ));
        }
        points.extend(right_points);

        if root_radius < base_radius
            && let Some(p_last) = points.last()
        {
            let angle_last = (p_last.y - center.y).atan2(p_last.x - center.x);
            points.push(Point::new(
                center.x + root_radius * angle_last.cos(),
                center.y + root_radius * angle_last.sin(),
            ));
        }
    }

    // Build the path
    if !points.is_empty() {
        builder.begin(point(points[0].x as f32, points[0].y as f32));
        for p in points.iter().skip(1) {
            builder.line_to(point(p.x as f32, p.y as f32));
        }
        builder.close();
    }

    // Add center hole if requested
    if hole_radius > 0.0 {
        builder.add_circle(
            point(center.x as f32, center.y as f32),
            hole_radius as f32,
            lyon::path::Winding::Negative,
        );
    }

    builder.build()
}

/// Generate Sprocket
pub fn generate_sprocket(
    center: Point,
    pitch: f64,
    teeth: usize,
    roller_diameter: f64,
    hole_radius: f64,
) -> Path {
    let mut builder = Path::builder();
    let pitch_radius = pitch / (2.0 * (PI / teeth as f64).sin());
    let roller_radius = roller_diameter / 2.0;
    let angle_per_tooth = 2.0 * PI / teeth as f64;

    // The standard outside radius is usually the pitch radius + a % of the roller.
    // but truncated so that it does not end in a point.
    let outer_radius = pitch_radius + (roller_radius * 0.6);

    let mut points = Vec::new();

    for i in 0..teeth {
        let angle = i as f64 * angle_per_tooth;

        // 1. VALLEY ARCH (Roller)
        let steps = 8;
        let arc_sweep = PI * 0.7;

        for j in 0..=steps {
            let t = (j as f64 / steps as f64) - 0.5;
            let a = angle + PI - (t * arc_sweep);
            let rc_x = center.x + pitch_radius * angle.cos();
            let rc_y = center.y + pitch_radius * angle.sin();

            points.push(Point::new(
                rc_x + roller_radius * a.cos(),
                rc_y + roller_radius * a.sin(),
            ));
        }

        // 2. FLAT CREST (Outer Diameter)
        // Instead of one point, we created two points on either side of the center of the tooth
        let flat_width = 0.05; // Adjust this value for a wider or narrower ridge

        // Ridge point 1 (before the centre)
        let tip_1 = angle + angle_per_tooth * (0.5 - flat_width);
        points.push(Point::new(
            center.x + outer_radius * tip_1.cos(),
            center.y + outer_radius * tip_1.sin(),
        ));

        // Point 2 of the ridge (after the center)
        let tip_2 = angle + angle_per_tooth * (0.5 + flat_width);
        points.push(Point::new(
            center.x + outer_radius * tip_2.cos(),
            center.y + outer_radius * tip_2.sin(),
        ));
    }

    // Closing the path and inserting points
    if !points.is_empty() {
        builder.begin(point(points[0].x as f32, points[0].y as f32));
        for p in points.iter().skip(1) {
            builder.line_to(point(p.x as f32, p.y as f32));
        }
        builder.close();
    }

    if hole_radius > 0.0 {
        builder.add_circle(
            point(center.x as f32, center.y as f32),
            hole_radius as f32,
            lyon::path::Winding::Negative,
        );
    }

    builder.build()
}

/// Generate a timing pulley (XL series approximation)
pub fn generate_timing_pulley(
    center: Point,
    pitch: f64,
    teeth: usize,
    _belt_width: f64,
    hole_radius: f64,
) -> Path {
    let mut builder = Path::builder();

    // XL belt parameters
    let tooth_height = 1.27; // mm

    // Radius: on timing pulleys the pitch radius comes from the circular pitch
    let pitch_radius = (pitch * teeth as f64) / (2.0 * PI);
    let outer_radius = pitch_radius;
    let root_radius = outer_radius - tooth_height;

    let mut points = Vec::new();
    let angle_per_tooth = 2.0 * PI / teeth as f64;

    for i in 0..teeth {
        let base_angle = i as f64 * angle_per_tooth;

        // 1. Left root of the tooth (lowest point before rising)
        let valley_left = base_angle - angle_per_tooth * 0.25;
        points.push(Point::new(
            center.x + root_radius * valley_left.cos(),
            center.y + root_radius * valley_left.sin(),
        ));

        // 2. Left flank of the tooth (rises to the crest)
        let tooth_left = base_angle - angle_per_tooth * 0.15;
        points.push(Point::new(
            center.x + outer_radius * tooth_left.cos(),
            center.y + outer_radius * tooth_left.sin(),
        ));

        // 3. Right flank of the tooth (holds the crest before falling)
        let tooth_right = base_angle + angle_per_tooth * 0.15;
        points.push(Point::new(
            center.x + outer_radius * tooth_right.cos(),
            center.y + outer_radius * tooth_right.sin(),
        ));

        // 4. Right root of the tooth (falls to the next valley)
        let valley_right = base_angle + angle_per_tooth * 0.25;
        points.push(Point::new(
            center.x + root_radius * valley_right.cos(),
            center.y + root_radius * valley_right.sin(),
        ));
    }

    if !points.is_empty() {
        builder.begin(point(points[0].x as f32, points[0].y as f32));
        for p in points.iter().skip(1) {
            builder.line_to(point(p.x as f32, p.y as f32));
        }
        builder.close();
    }

    if hole_radius > 0.0 {
        builder.add_circle(
            point(center.x as f32, center.y as f32),
            hole_radius as f32,
            lyon::path::Winding::Negative,
        );
    }

    builder.build()
}

/// Generate an L-bracket
pub fn generate_l_bracket(
    center: Point,
    width: f64,
    height: f64,
    thickness: f64,
    hole_diameter: f64,
    hole_spacing: f64,
) -> Path {
    let mut builder = Path::builder();

    let half_width = width / 2.0;
    let half_height = height / 2.0;

    // Create L-shape
    let points = [
        Point::new(center.x - half_width, center.y - half_height),
        Point::new(center.x + half_width, center.y - half_height),
        Point::new(center.x + half_width, center.y - half_height + thickness),
        Point::new(
            center.x - half_width + thickness,
            center.y - half_height + thickness,
        ),
        Point::new(center.x - half_width + thickness, center.y + half_height),
        Point::new(center.x - half_width, center.y + half_height),
    ];

    builder.begin(point(points[0].x as f32, points[0].y as f32));
    for p in points.iter().skip(1) {
        builder.line_to(point(p.x as f32, p.y as f32));
    }
    builder.close();

    // Add mounting holes
    let hole_radius = hole_diameter / 2.0;
    let hole_centers = vec![
        Point::new(
            center.x - half_width + thickness / 2.0,
            center.y - half_height + thickness / 2.0,
        ),
        Point::new(
            center.x + half_width - hole_spacing,
            center.y - half_height + thickness / 2.0,
        ),
        Point::new(
            center.x - half_width + thickness / 2.0,
            center.y + half_height - hole_spacing,
        ),
    ];

    for hc in hole_centers {
        builder.add_circle(
            point(hc.x as f32, hc.y as f32),
            hole_radius as f32,
            lyon::path::Winding::Negative,
        );
    }

    builder.build()
}

/// Generate a U-bracket/channel
pub fn generate_u_bracket(
    center: Point,
    length: f64,
    width: f64,
    thickness: f64,
    hole_diameter: f64,
    hole_spacing: f64,
) -> Path {
    let mut builder = Path::builder();

    let half_length = length / 2.0;
    let half_width = width / 2.0;

    // Create U-shape
    let points = [
        Point::new(center.x - half_length, center.y - half_width),
        Point::new(center.x + half_length, center.y - half_width),
        Point::new(center.x + half_length, center.y + half_width),
        Point::new(center.x + half_length - thickness, center.y + half_width),
        Point::new(
            center.x + half_length - thickness,
            center.y - half_width + thickness,
        ),
        Point::new(
            center.x - half_length + thickness,
            center.y - half_width + thickness,
        ),
        Point::new(center.x - half_length + thickness, center.y + half_width),
        Point::new(center.x - half_length, center.y + half_width),
    ];

    builder.begin(point(points[0].x as f32, points[0].y as f32));
    for p in points.iter().skip(1) {
        builder.line_to(point(p.x as f32, p.y as f32));
    }
    builder.close();

    // Add mounting holes
    let hole_radius = hole_diameter / 2.0;
    let hole_centers = vec![
        Point::new(
            center.x - half_length + thickness / 2.0,
            center.y - half_width + thickness / 2.0,
        ),
        Point::new(
            center.x + half_length - thickness / 2.0,
            center.y - half_width + thickness / 2.0,
        ),
        Point::new(
            center.x - half_length + thickness / 2.0,
            center.y + half_width - hole_spacing,
        ),
        Point::new(
            center.x + half_length - thickness / 2.0,
            center.y + half_width - hole_spacing,
        ),
    ];

    for hc in hole_centers {
        builder.add_circle(
            point(hc.x as f32, hc.y as f32),
            hole_radius as f32,
            lyon::path::Winding::Negative,
        );
    }

    builder.build()
}
