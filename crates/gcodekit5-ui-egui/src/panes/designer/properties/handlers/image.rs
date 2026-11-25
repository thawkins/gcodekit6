//! Laser Engraving, for one raster image: feed and travel rates (mm/s), minimum and
//! maximum power (%), resolution (PPI), scan direction, bidirectional, invert, dithering
//! and the halftone threshold.
//!
//! Every change applies to all selected raster images as one undo step. GTK's limits
//! apply: rates 0 or more, powers 0 to 100, resolution 1 to 1000 PPI, threshold 0 to 255.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model).

use gcodekit5_designer::model::{RasterImage, Shape};

use super::super::builders::{self, Check, E_RANGE, entry, number, section};
use super::{Ctx, change_selected, set_f64, set_value};

/// The scan directions: model value and label msgid.
const SCANS: [(&str, &str); 2] = [
    ("horizontal", n_!("Horizontal")),
    ("vertical", n_!("Vertical")),
];
/// The dithering methods: model value and label msgid.
const DITHERS: [(&str, &str); 5] = [
    ("none", n_!("None")),
    ("threshold", n_!("Threshold")),
    ("floyd", n_!("Floyd-Ste.")),
    ("atkinson", n_!("Atkinson")),
    ("bayer", n_!("Bayer")),
];

/// The msgids of this section.
pub fn msgids() -> Vec<&'static str> {
    let mut ids = vec![
        n_!("Laser Engraving"),
        n_!("Feed Rate:"),
        n_!("Travel Rate:"),
        n_!("Min Power:"),
        n_!("Max Power:"),
        n_!("Resolution:"),
        n_!("Scan:"),
        n_!("Bidirectional:"),
        n_!("Invert:"),
        n_!("Dithering:"),
        n_!("Halftone:"),
    ];
    ids.extend(SCANS.iter().map(|(_, m)| *m));
    ids.extend(DITHERS.iter().map(|(_, m)| *m));
    ids
}

/// Applies `modify` to every selected raster image as one undo step named `name`.
fn change_images(cx: &mut Ctx<'_>, name: &str, modify: impl Fn(&mut RasterImage) -> bool) {
    change_selected(cx.state(), name, |obj| match &mut obj.shape {
        Shape::RasterImage(img) => modify(img),
        _ => false,
    });
}

/// A numeric field row: commits a clamped value through `store`.
struct Numeric {
    key: &'static str,
    msgid: &'static str,
    unit: &'static str,
    undo: &'static str,
    value: f64,
    clamp: fn(f64) -> f64,
    store: fn(&mut RasterImage, f64) -> bool,
}

/// The numeric fields of image `r`.
fn numerics(r: &RasterImage) -> [Numeric; 5] {
    [
        Numeric {
            key: "image_feed",
            msgid: n_!("Feed Rate:"),
            unit: "mm/s",
            undo: "Change Image Feed Rate",
            value: r.feed_rate,
            clamp: |v| v.max(0.0),
            store: |img, v| set_f64(&mut img.feed_rate, v),
        },
        Numeric {
            key: "image_travel",
            msgid: n_!("Travel Rate:"),
            unit: "mm/s",
            undo: "Change Image Travel Rate",
            value: r.travel_rate,
            clamp: |v| v.max(0.0),
            store: |img, v| set_f64(&mut img.travel_rate, v),
        },
        Numeric {
            key: "image_min_power",
            msgid: n_!("Min Power:"),
            unit: "%",
            undo: "Change Image Min Power",
            value: r.min_power,
            clamp: |v| v.clamp(0.0, 100.0),
            store: |img, v| set_f64(&mut img.min_power, v),
        },
        Numeric {
            key: "image_max_power",
            msgid: n_!("Max Power:"),
            unit: "%",
            undo: "Change Image Max Power",
            value: r.max_power,
            clamp: |v| v.clamp(0.0, 100.0),
            store: |img, v| set_f64(&mut img.max_power, v),
        },
        Numeric {
            key: "image_ppi",
            msgid: n_!("Resolution:"),
            unit: "PPI",
            undo: "Change Image PPI",
            value: r.ppi,
            clamp: |v| v.clamp(1.0, 1000.0),
            store: |img, v| set_f64(&mut img.ppi, v),
        },
    ]
}

/// Applies the committed `text` of field `key`, if it is one of this section's: a number,
/// clamped to its field's range, or a halftone threshold of 0 to 255.
///
/// # Arguments
/// * `cx` - The frame's context, for the shapes the edit belongs to.
/// * `key` - The field.
/// * `text` - The committed text.
pub fn apply(cx: &mut Ctx<'_>, key: &str, text: &str) -> Option<Check> {
    if key == "image_halftone" {
        return Some(text.trim().parse::<u8>().map_err(|_| E_RANGE).map(|t| {
            change_images(cx, "Change Halftone Threshold", |img| {
                set_value(&mut img.halftone_threshold, t)
            });
        }));
    }
    if !key.starts_with("image_") {
        return None;
    }
    let Some(Shape::RasterImage(r)) = &cx.summary.single else {
        return Some(Ok(()));
    };
    let n = numerics(r).into_iter().find(|n| n.key == key)?;
    Some(number(text).map(|v| {
        let v = (n.clamp)(v);
        change_images(cx, n.undo, |img| (n.store)(img, v));
    }))
}

/// The Laser Engraving section.
pub fn show(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let Some(Shape::RasterImage(r)) = &cx.summary.single else {
        return;
    };
    let tr = cx.tr;
    let numerics = numerics(r);
    section(ui, &tr.tr("Laser Engraving"), |ui| {
        for n in numerics {
            entry(
                ui,
                &mut cx.fields(),
                n.key,
                &tr.tr(n.msgid),
                &n.value.to_string(),
                n.unit,
                true,
            );
        }
        let scans: Vec<String> = SCANS.iter().map(|(_, m)| tr.tr(m).into_owned()).collect();
        let scan = SCANS
            .iter()
            .position(|(v, _)| *v == r.scan_direction)
            .unwrap_or(0);
        if let Some(i) = builders::combo(ui, &tr.tr("Scan:"), &scans, scan, true) {
            let value = SCANS[i].0.to_string();
            change_images(cx, "Change Scan Direction", |img| {
                set_value(&mut img.scan_direction, value.clone())
            });
        }
        if let Some(on) = builders::check(ui, &tr.tr("Bidirectional:"), r.bidirectional, true) {
            change_images(cx, "Change Bidirectional", |img| {
                set_value(&mut img.bidirectional, on)
            });
        }
        if let Some(on) = builders::check(ui, &tr.tr("Invert:"), r.invert, true) {
            change_images(cx, "Change Invert", |img| set_value(&mut img.invert, on));
        }
        let dithers: Vec<String> = DITHERS.iter().map(|(_, m)| tr.tr(m).into_owned()).collect();
        let dither = DITHERS
            .iter()
            .position(|(v, _)| *v == r.dithering)
            .unwrap_or(0);
        if let Some(i) = builders::combo(ui, &tr.tr("Dithering:"), &dithers, dither, true) {
            let value = DITHERS[i].0.to_string();
            change_images(cx, "Change Dithering", |img| {
                set_value(&mut img.dithering, value.clone())
            });
        }
        entry(
            ui,
            &mut cx.fields(),
            "image_halftone",
            &tr.tr("Halftone:"),
            &r.halftone_threshold.to_string(),
            "(0-255)",
            true,
        );
    });
}
