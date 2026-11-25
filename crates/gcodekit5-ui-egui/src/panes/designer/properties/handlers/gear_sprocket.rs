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

use super::super::builders::{FieldEvent, entry, parse_number, section};
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

/// A committed number from `event` that passes `valid`.
fn number(event: FieldEvent, valid: impl Fn(f64) -> bool) -> Option<f64> {
    match event {
        FieldEvent::Committed(text) => parse_number(&text).filter(|v| valid(*v)),
        FieldEvent::None | FieldEvent::Typed(_) => None,
    }
}

/// A committed whole number from `event` of at least `min`.
fn count(event: FieldEvent, min: usize) -> Option<usize> {
    match event {
        FieldEvent::Committed(text) => text.trim().parse::<usize>().ok().filter(|n| *n >= min),
        FieldEvent::None | FieldEvent::Typed(_) => None,
    }
}

/// One numeric row: label msgid, key, shown value and unit.
fn row(
    ui: &mut egui::Ui,
    cx: &mut Ctx<'_>,
    msgid: &str,
    key: &'static str,
    shown: &str,
) -> FieldEvent {
    let tr = cx.tr;
    entry(ui, &mut cx.fields(), key, &tr.tr(msgid), shown, "", true)
}

/// The Gear section.
pub fn gear(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let Some(Shape::Gear(g)) = &cx.summary.single else {
        return;
    };
    let (module, teeth, angle) = (g.module, g.teeth, g.pressure_angle_deg);
    let tr = cx.tr;
    section(ui, &tr.tr("Gear"), |ui| {
        let e = row(ui, cx, "Module:", "gear_module", &format!("{module:.2}"));
        if let Some(m) = number(e, |v| v > 0.0) {
            cx.state().set_selected_gear_properties(m, teeth, angle);
        }
        let e = row(ui, cx, "Teeth:", "gear_teeth", &teeth.to_string());
        if let Some(n) = count(e, 3) {
            cx.state().set_selected_gear_properties(module, n, angle);
        }
        let e = row(ui, cx, "Pressure Angle:", "gear_pa", &format!("{angle:.1}"));
        if let Some(a) = number(e, |v| v > 0.0 && v < 45.0) {
            cx.state().set_selected_gear_properties(module, teeth, a);
        }
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
        let e = row(ui, cx, "Pitch:", "sprocket_pitch", &format!("{pitch:.2}"));
        if let Some(p) = number(e, |v| v > 0.0) {
            let standard = DesignSprocket::standard_roller_diameter(p);
            cx.state()
                .set_selected_sprocket_properties(p, teeth, standard);
        }
        let e = row(ui, cx, "Teeth:", "sprocket_teeth", &teeth.to_string());
        if let Some(n) = count(e, 3) {
            cx.state()
                .set_selected_sprocket_properties(pitch, n, roller);
        }
        let e = row(
            ui,
            cx,
            "Roller Dia:",
            "sprocket_roller",
            &format!("{roller:.2}"),
        );
        if let Some(r) = number(e, |v| v > 0.0) {
            cx.state().set_selected_sprocket_properties(pitch, teeth, r);
        }
    });
}

/// The Timing Pulley section.
pub fn timing_pulley(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let Some((pitch, teeth, belt, hole)) = cx.summary.timing_pulley() else {
        return;
    };
    let tr = cx.tr;
    section(ui, &tr.tr("Timing Pulley"), |ui| {
        let e = row(ui, cx, "Pitch:", "pulley_pitch", &format!("{pitch:.2}"));
        if let Some(p) = number(e, |v| v > 0.0) {
            cx.state()
                .set_selected_timing_pulley_properties(p, teeth, belt, hole);
        }
        let e = row(ui, cx, "Teeth:", "pulley_teeth", &teeth.to_string());
        if let Some(n) = count(e, 6) {
            cx.state()
                .set_selected_timing_pulley_properties(pitch, n, belt, hole);
        }
        let e = row(ui, cx, "Belt Width:", "pulley_belt", &format!("{belt:.2}"));
        if let Some(b) = number(e, |v| v > 0.0) {
            cx.state()
                .set_selected_timing_pulley_properties(pitch, teeth, b, hole);
        }
        let e = row(ui, cx, "Hole Radius:", "pulley_hole", &format!("{hole:.2}"));
        if let Some(h) = number(e, |v| v >= 0.0) {
            cx.state()
                .set_selected_timing_pulley_properties(pitch, teeth, belt, h);
        }
    });
}
