//! # Spoilboard Surfacing Generator
//!
//! Generates G-code for flattening/surfacing CNC spoilboards using a
//! fly cutter or large end mill. Produces a raster pattern with
//! configurable stepover, depth, and spindle speed.
//!
//! Homing is optional (off by default) and handled by [`crate::job_preamble`]: when it is on,
//! the G54 work offsets are kept (the Z work offset is the spoilboard surface) and every Z
//! travel/retract is a `G53` machine-coordinate move that never rises above
//! the homed position.

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct SpoilboardSurfacingParameters {
    pub width: f64,
    pub height: f64,
    pub tool_diameter: f64,
    pub feed_rate: f64,
    pub spindle_speed: f64,
    pub cut_depth: f64,
    pub stepover_percent: f64,
    pub safe_z: f64,
    /// Home (`$H`) before surfacing, keeping the G54 work offsets (default `false`).
    pub home_before_job: bool,
}

pub struct SpoilboardSurfacingGenerator {
    params: SpoilboardSurfacingParameters,
}

/// How close (mm) a row must be to the far edge to count as the last row.
const EDGE_TOLERANCE_MM: f64 = 1e-6;

impl SpoilboardSurfacingGenerator {
    pub fn new(params: SpoilboardSurfacingParameters) -> Self {
        Self { params }
    }

    pub fn generate(&self) -> Result<String> {
        let mut gcode = String::new();
        let p = &self.params;

        let step_dist = p.tool_diameter * (p.stepover_percent / 100.0);
        if !(step_dist.is_finite() && step_dist > 0.0) {
            anyhow::bail!("Tool diameter and stepover must be positive");
        }
        if !(p.width > 0.0 && p.height >= 0.0) {
            anyhow::bail!("Surfacing width must be positive and height non-negative");
        }

        // Header
        gcode.push_str("; Spoilboard Surfacing Toolpath\n");
        gcode.push_str(&format!(
            "; Dimensions: {:.1} x {:.1} mm\n",
            p.width, p.height
        ));
        gcode.push_str(&format!("; Tool Diameter: {:.1} mm\n", p.tool_diameter));
        gcode.push_str(&format!("; Cut Depth: {:.1} mm\n", p.cut_depth));

        // Initialization sequence
        crate::job_preamble::push_modal_header(&mut gcode, true);
        gcode.push('\n');

        // Only X/Y are re-zeroed when homing: the Z work offset (spoilboard surface) is kept,
        // so the cut depth is measured from the surface, not from the homed position.
        crate::job_preamble::push_work_setup(&mut gcode, p.home_before_job);
        // Clear the work before any X/Y move.
        gcode.push_str(&crate::job_preamble::travel_z_line(
            p.home_before_job,
            p.safe_z as f32,
            "Move to safe height",
        ));

        // Spindle start
        gcode.push_str(&format!("M3 S{:.0}\n", p.spindle_speed));

        // Move to Start Position (X0 Y0)
        gcode.push_str("G0 X0 Y0\n");

        // Plunge to Cut Depth
        let target_z = -p.cut_depth.abs();
        gcode.push_str(&format!("G1 Z{:.3} F{:.1}\n", target_z, p.feed_rate / 2.0));

        let mut current_y = 0.0;
        let mut going_right = true;

        while current_y <= p.height {
            let target_x = if going_right { p.width } else { 0.0 };
            gcode.push_str(&format!("G1 X{:.3} F{:.1}\n", target_x, p.feed_rate));

            // Summed steps drift (16 x 12.7 is 203.19999999999996, not 203.2), so a row
            // within EDGE_TOLERANCE_MM of the far edge counts as on it; otherwise the drift
            // adds a duplicate pass at the edge.
            if p.height - current_y > EDGE_TOLERANCE_MM {
                current_y += step_dist;
                if current_y > p.height - EDGE_TOLERANCE_MM {
                    current_y = p.height;
                }
                gcode.push_str(&format!("G1 Y{:.3} F{:.1}\n", current_y, p.feed_rate));
                going_right = !going_right;
            } else {
                break;
            }
        }

        gcode.push_str(&crate::job_preamble::travel_z_line(
            p.home_before_job,
            p.safe_z as f32,
            "Retract",
        ));

        // Stop spindle and end program
        gcode.push_str("M5\n");
        gcode.push_str("M30\n");

        Ok(gcode)
    }
}
