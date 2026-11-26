//! The icon catalog: every icon has SVG data that egui's loader decodes, and an icon button
//! exposes its label to accessibility tools.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gcodekit5_ui_egui::icons::{self, Icon};

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
