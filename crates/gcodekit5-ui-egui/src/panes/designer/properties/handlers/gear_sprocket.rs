//! Gear (module, teeth, pressure angle), Sprocket (pitch, teeth, roller diameter) and
//! Timing Pulley (pitch, teeth, belt width, hole radius).
//!
//! The values are millimetres and degrees, shown unconverted as in GTK. GTK's limits apply:
//! a gear's module above 0, at least 3 teeth and a pressure angle between 0 and 45 degrees;
//! a sprocket's pitch and roller above 0 and at least 3 teeth (a new pitch also sets the
//! standard roller diameter for it, in the same step); a pulley's pitch and belt width
//! above 0, at least 6 teeth and a hole radius of 0 or more. The other parameters keep the
//! shape's values.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model).

use gcodekit5_designer::model::{DesignSprocket, Shape};

use super::super::builders::{
    Check, E_RANGE, entry, non_negative, number, positive, section, whole,
};
use super::Ctx;

/// The msgids of these sections.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Gear"),
        n_!("Module:"),
        n_!("Teeth:"),
        n_!("Pressure Angle:"),
        n_!("Sprocket"),
        n_!("Pitch:"),
        n_!("Roller Dia:"),
        n_!("Timing Pulley"),
        n_!("Belt Width:"),
        n_!("Hole Radius:"),
    ]
}

/// A number above 0.
fn pos(text: &str) -> Result<f64, &'static str> {
    number(text).and_then(positive)
}

/// Applies the committed `text` of field `key`, if it is one of these sections'.
///
/// # Arguments
/// * `cx` - The frame's context, for the shapes the edit belongs to.
/// * `key` - The field.
/// * `text` - The committed text.
pub fn apply(cx: &mut Ctx<'_>, key: &str, text: &str) -> Option<Check> {
    let single = cx.summary.single.clone();
    let pulley = cx.summary.timing_pulley();
    let state = cx.state();
    let result = match (key, &single) {
        ("gear_module", Some(Shape::Gear(g))) => {
            pos(text).map(|m| state.set_selected_gear_properties(m, g.teeth, g.pressure_angle_deg))
        }
        ("gear_teeth", Some(Shape::Gear(g))) => whole(text, 3, E_RANGE)
            .map(|n| state.set_selected_gear_properties(g.module, n, g.pressure_angle_deg)),
        ("gear_pa", Some(Shape::Gear(g))) => number(text)
            .and_then(|a| {
                if a > 0.0 && a < 45.0 {
                    Ok(a)
                } else {
                    Err(E_RANGE)
                }
            })
            .map(|a| state.set_selected_gear_properties(g.module, g.teeth, a)),
        ("sprocket_pitch", Some(Shape::Sprocket(s))) => pos(text).map(|p| {
            // A new pitch also takes its standard roller, in the same step.
            let roller = DesignSprocket::standard_roller_diameter(p);
            state.set_selected_sprocket_properties(p, s.teeth, roller);
        }),
        ("sprocket_teeth", Some(Shape::Sprocket(s))) => whole(text, 3, E_RANGE)
            .map(|n| state.set_selected_sprocket_properties(s.pitch, n, s.roller_diameter)),
        ("sprocket_roller", Some(Shape::Sprocket(s))) => {
            pos(text).map(|r| state.set_selected_sprocket_properties(s.pitch, s.teeth, r))
        }
        (k, _) if k.starts_with("pulley_") => {
            let Some((pitch, teeth, belt, hole)) = pulley else {
                return Some(Ok(()));
            };
            match k {
                "pulley_pitch" => pos(text).map(|p| (p, teeth, belt, hole)),
                "pulley_teeth" => whole(text, 6, E_RANGE).map(|n| (pitch, n, belt, hole)),
                "pulley_belt" => pos(text).map(|b| (pitch, teeth, b, hole)),
                _ => number(text)
                    .and_then(non_negative)
                    .map(|h| (pitch, teeth, belt, h)),
            }
            .map(|(p, n, b, h)| state.set_selected_timing_pulley_properties(p, n, b, h))
        }
        (k, _) if k.starts_with("gear_") || k.starts_with("sprocket_") => Ok(()),
        _ => return None,
    };
    Some(result)
}

/// One numeric row: label msgid, key and shown value.
fn row(ui: &mut egui::Ui, cx: &mut Ctx<'_>, msgid: &str, key: &'static str, shown: &str) {
    let tr = cx.tr;
    entry(ui, &mut cx.fields(), key, &tr.tr(msgid), shown, "", true);
}

/// The Gear section.
pub fn gear(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let Some(Shape::Gear(g)) = &cx.summary.single else {
        return;
    };
    let (module, teeth, angle) = (g.module, g.teeth, g.pressure_angle_deg);
    let tr = cx.tr;
    section(ui, &tr.tr("Gear"), |ui| {
        row(ui, cx, "Module:", "gear_module", &format!("{module:.2}"));
        row(ui, cx, "Teeth:", "gear_teeth", &teeth.to_string());
        row(ui, cx, "Pressure Angle:", "gear_pa", &format!("{angle:.1}"));
    });
}

/// The Sprocket section.
pub fn sprocket(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let Some(Shape::Sprocket(s)) = &cx.summary.single else {
        return;
    };
    let (pitch, teeth, roller) = (s.pitch, s.teeth, s.roller_diameter);
    let tr = cx.tr;
    section(ui, &tr.tr("Sprocket"), |ui| {
        row(ui, cx, "Pitch:", "sprocket_pitch", &format!("{pitch:.2}"));
        row(ui, cx, "Teeth:", "sprocket_teeth", &teeth.to_string());
        row(
            ui,
            cx,
            "Roller Dia:",
            "sprocket_roller",
            &format!("{roller:.2}"),
        );
    });
}

/// The Timing Pulley section.
pub fn timing_pulley(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let Some((pitch, teeth, belt, hole)) = cx.summary.timing_pulley() else {
        return;
    };
    let tr = cx.tr;
    section(ui, &tr.tr("Timing Pulley"), |ui| {
        row(ui, cx, "Pitch:", "pulley_pitch", &format!("{pitch:.2}"));
        row(ui, cx, "Teeth:", "pulley_teeth", &teeth.to_string());
        row(ui, cx, "Belt Width:", "pulley_belt", &format!("{belt:.2}"));
        row(ui, cx, "Hole Radius:", "pulley_hole", &format!("{hole:.2}"));
    });
}
