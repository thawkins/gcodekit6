//! The icon catalog: every icon has SVG data that egui's loader decodes, and an icon button
//! exposes its label to accessibility tools.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit6_ui_egui::icons::{self, Icon};

#[test]
fn test_icons_every_icon_has_svg() {
    for icon in Icon::ALL {
        let svg = std::str::from_utf8(icon.svg()).expect("svg is utf-8");
        assert!(svg.contains("<svg"), "{icon:?} is not an svg");
        assert!(
            svg.contains("<path") || svg.contains("<circle"),
            "{icon:?} is empty"
        );
    }
}

#[test]
fn test_icons_every_icon_decodes_in_loader() {
    let mut harness = Harness::new_ui(|ui| {
        icons::install(ui.ctx());
        ui.horizontal_wrapped(|ui| {
            for icon in Icon::ALL {
                ui.add(icon.image().fit_to_exact_size(egui::vec2(24.0, 24.0)));
            }
        });
    });
    harness.run();
    harness.run();
    let ctx = harness.ctx.clone();
    for icon in Icon::ALL {
        let result = icon.image().load_for_size(&ctx, egui::vec2(24.0, 24.0));
        assert!(
            matches!(result, Ok(egui::load::TexturePoll::Ready { .. })),
            "{icon:?} did not decode: {:?}",
            result.as_ref().err()
        );
    }
}

#[test]
fn test_icons_install_is_idempotent() {
    let ctx = egui::Context::default();
    icons::install(&ctx);
    icons::install(&ctx);
}

#[test]
fn test_icons_icon_button_exposes_label() {
    let mut clicked = false;
    let mut harness = Harness::new_ui(|ui| {
        icons::install(ui.ctx());
        if icons::icon_button(ui, Icon::Home, "Home").clicked() {
            clicked = true;
        }
        icons::icon_only_button(ui, Icon::Unlock, "Unlock");
    });
    harness.run();
    harness.get_by_label("Home").click();
    harness.run();
    harness.get_by_label("Unlock");
    drop(harness);
    assert!(clicked);
}

#[test]
fn test_icons_tint_follows_theme_text_colour() {
    for dark in [false, true] {
        let mut seen = None;
        let mut harness = Harness::new_ui(|ui| {
            ui.ctx().set_visuals(if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
            seen = Some((icons::tint(ui), ui.visuals().text_color()));
        });
        harness.run();
        drop(harness);
        let (tint, text) = seen.expect("ui ran");
        assert_eq!(tint, text);
    }
}

/// Every app SVG the designer toolbox, the shape gallery and the objects list draw.
const APP_ICONS_USED: [&str; 18] = [
    "select.svg",
    "grab.svg",
    "rectangle.svg",
    "circle.svg",
    "line.svg",
    "ellipse.svg",
    "triangle.svg",
    "poligon.svg",
    "polyline.svg",
    "text.svg",
    "gear.svg",
    "sprocket.svg",
    "sprocket1.svg",
    "timingpulley.svg",
    "slot.svg",
    "Lsupport.svg",
    "Usupport.svg",
    "move_to_top.svg",
];

#[test]
fn test_icons_every_used_app_icon_decodes() {
    let mut names: Vec<&str> = APP_ICONS_USED.to_vec();
    names.push("move_to_bottom.svg");
    names.extend(
        gcodekit6_ui_egui::panes::designer::shape_gallery::TEMPLATES
            .iter()
            .map(|t| t.icon),
    );
    names.extend(
        gcodekit6_ui_egui::panes::designer::canvas::DesignerTool::ALL
            .iter()
            .map(|t| t.icon()),
    );
    let mut images = Vec::new();
    let mut harness = Harness::new_ui(|ui| {
        images = names
            .iter()
            .map(|name| icons::app_icon(ui, name, 24.0))
            .collect();
        for image in images.iter().flatten() {
            ui.add(image.clone());
        }
    });
    harness.run();
    harness.run();
    let ctx = harness.ctx.clone();
    drop(harness);
    for (name, image) in names.iter().zip(&images) {
        let image = image
            .as_ref()
            .unwrap_or_else(|| panic!("{name} is not embedded"));
        let result = image.load_for_size(&ctx, egui::vec2(24.0, 24.0));
        assert!(
            matches!(result, Ok(egui::load::TexturePoll::Ready { .. })),
            "{name} did not decode: {:?}",
            result.as_ref().err()
        );
    }
}

#[test]
fn test_icons_app_icon_button_exposes_its_tooltip() {
    let mut harness = Harness::new_ui(|ui| {
        icons::app_icon_button(ui, "circle.svg", 40.0, true, "Circle");
    });
    harness.run();
    let node = harness.get_by_label("Circle");
    assert!((node.rect().width() - 40.0).abs() < 0.5);
}
