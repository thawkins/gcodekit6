//! Laser Config (Object), laser mode only: whether the shapes use the global laser values,
//! and their own feed rate, power (%) and passes.
//!
//! The values shown resolve "use global values" to the toolbox's settings. Several shapes
//! show each value they share and "---" where they differ; the fields are editable when one
//! shape uses its own values. A change applies to every selected shape except raster
//! images as one undo step, in both copies the model keeps (the shape's own parameters, which the
//! 2D toolpaths read, and the object's, which are saved).
//!
//! Committing one field of several shapes keeps each shape's other values
//! (a "---" field is not written); a
//! feed rate of 0 or less and passes below 1 are rejected with an inline error (not turned
//! into defaults).
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model).

use gcodekit5_designer::model::{LaserParams, Shape};

use super::super::builders::{
    self, Check, E_COUNT, MIXED, entry, number, positive, section, whole,
};
use super::super::update::{resolve_laser, shape_laser_params};
use super::{Ctx, change_selected};

/// The msgids of this section.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Laser Config (Object)"),
        n_!("Use global values"),
        n_!("Feed rate:"),
        n_!("Power:"),
        n_!("Passes:"),
    ]
}

/// A change to the laser settings: the global switch, or one of the values.
#[derive(Clone, Copy, Default)]
struct Change {
    use_global: Option<bool>,
    feed: Option<f64>,
    power: Option<f64>,
    passes: Option<u32>,
}

/// Writes `params` into the laser parameters embedded in `shape`.
fn set_shape_params(shape: &mut Shape, params: LaserParams) {
    match shape {
        Shape::Rectangle(s) => s.laser_params = params,
        Shape::Circle(s) => s.laser_params = params,
        Shape::Ellipse(s) => s.laser_params = params,
        Shape::Line(s) => s.laser_params = params,
        Shape::Path(s) => s.laser_params = params,
        Shape::Polygon(s) => s.laser_params = params,
        Shape::Triangle(s) => s.laser_params = params,
        Shape::Text(s) => s.laser_params = params,
        Shape::Gear(s) => s.laser_params = params,
        Shape::Sprocket(s) => s.laser_params = params,
        Shape::RasterImage(_) => {}
    }
}

/// Applies `change` to every selected shape that has laser parameters (one undo step).
fn apply_change(cx: &mut Ctx<'_>, change: Change) {
    let state = cx.state();
    let global = state.tool_settings.clone();
    change_selected(state, "Change Laser Override", |obj| {
        let Some(old) = shape_laser_params(&obj.shape) else {
            return false;
        };
        let use_global = change.use_global.unwrap_or(old.use_global);
        let (feed, power, passes) = resolve_laser(&old, &global);
        let params = if use_global {
            let (feed, power, passes) = resolve_laser(
                &LaserParams {
                    use_global: true,
                    ..old
                },
                &global,
            );
            LaserParams {
                feed_rate: feed,
                power_percent: power,
                passes,
                use_global: true,
            }
        } else {
            LaserParams {
                feed_rate: change.feed.unwrap_or(feed),
                power_percent: change.power.unwrap_or(power).clamp(0.0, 100.0),
                passes: change.passes.unwrap_or(passes),
                use_global: false,
            }
        };
        let changed = (params.feed_rate - old.feed_rate).abs() > f64::EPSILON
            || (params.power_percent - old.power_percent).abs() > f64::EPSILON
            || params.passes != old.passes
            || params.use_global != old.use_global
            || obj.use_global_laser != use_global;
        obj.use_global_laser = use_global;
        obj.laser_params = params;
        set_shape_params(&mut obj.shape, params);
        changed
    });
}

/// Applies the committed `text` of field `key`, if it is one of this section's: a feed rate
/// above 0, a power (clamped to 0..100 %), or whole passes of 1 or more.
///
/// # Arguments
/// * `cx` - The frame's context, for the shapes the edit belongs to.
/// * `key` - The field.
/// * `text` - The committed text.
pub fn apply(cx: &mut Ctx<'_>, key: &str, text: &str) -> Option<Check> {
    // A value edit takes the switch as shown (off whenever the fields are editable).
    let mut change = Change {
        use_global: Some(cx.summary.laser.is_some_and(|s| s.all_use_global)),
        ..Change::default()
    };
    let parsed = match key {
        "laser_feed" => number(text)
            .and_then(positive)
            .map(|v| change.feed = Some(v)),
        "laser_power" => number(text).map(|v| change.power = Some(v)),
        "laser_passes" => whole(text, 1, E_COUNT)
            .map(|n| change.passes = Some(u32::try_from(n).unwrap_or(u32::MAX))),
        _ => return None,
    };
    Some(parsed.map(|()| apply_change(cx, change)))
}

/// The Laser Config (Object) section.
pub fn show(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let Some(sum) = cx.summary.laser else {
        return;
    };
    let tr = cx.tr;
    let editable = if cx.summary.single().is_some() {
        !sum.all_use_global
    } else {
        sum.any_override
    };
    let text = |v: Option<String>| v.unwrap_or_else(|| MIXED.to_string());
    let feed = text(sum.feed_rate.map(|v| v.to_string()));
    let power = text(sum.power_percent.map(|v| v.to_string()));
    let passes = text(sum.passes.map(|v| v.to_string()));
    section(ui, &tr.tr("Laser Config (Object)"), |ui| {
        if let Some(global) =
            builders::check(ui, &tr.tr("Use global values"), sum.all_use_global, true)
        {
            apply_change(
                cx,
                Change {
                    use_global: Some(global),
                    ..Change::default()
                },
            );
        }
        for (key, msgid, shown) in [
            ("laser_feed", n_!("Feed rate:"), &feed),
            ("laser_power", n_!("Power:"), &power),
            ("laser_passes", n_!("Passes:"), &passes),
        ] {
            entry(
                ui,
                &mut cx.fields(),
                key,
                &tr.tr(msgid),
                shown,
                "",
                editable,
            );
        }
    });
}
