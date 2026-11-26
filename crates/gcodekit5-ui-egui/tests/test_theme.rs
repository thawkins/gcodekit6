//! The Adwaita-like theme: a sample of widgets built with the theme helpers, as a snapshot in
//! dark and light, and the global metrics the helpers rely on.

use egui_kittest::kittest::NodeT;
use egui_kittest::{Harness, SnapshotOptions};
use gcodekit5_ui_egui::theme;

/// Snapshot options for the software renderer.
fn snapshot_options() -> SnapshotOptions {
    SnapshotOptions::new()
        .threshold(1.0)
        .max_failed_pixels(40)
        .output_path(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots"))
}

/// A small page of widgets drawn with the theme helpers.
fn sample(ui: &mut egui::Ui, text: &mut String, checked: &mut [bool; 2], choice: &mut usize) {
    egui::Frame::central_panel(ui.style())
        .inner_margin(egui::Margin::same(theme::PAGE_MARGIN))
        .show(ui, |ui| {
            theme::section_heading(ui, "Connection");
            ui.horizontal(|ui| {
                ui.add(theme::primary_button("Connect"));
                let _ = ui.button("Refresh");
                ui.add(theme::destructive_button(ui, "Stop"));
            });
            theme::group_frame(ui).show(ui, |ui| {
                theme::section_heading(ui, "Size");
                ui.horizontal(|ui| {
                    let label = ui.label("Width:");
                    ui.add(egui::TextEdit::singleline(text).desired_width(80.0))
                        .labelled_by(label.id);
                    ui.label("mm");
                });
                theme::checkbox(ui, &mut checked[0], "Lock");
                theme::checkbox(ui, &mut checked[1], "Show grid");
                ui.horizontal(|ui| {
                    ui.radio_value(choice, 0, "Laser 2D");
                    ui.radio_value(choice, 1, "CNC 3D");
                });
                egui::ComboBox::from_label("Scan")
                    .selected_text("Horizontal")
                    .show_ui(ui, |ui| {
                        let _ = ui.selectable_label(true, "Horizontal");
                    });
                let mut value = 5.0_f64;
                ui.add(egui::DragValue::new(&mut value).suffix(" mm"));
            });
        });
}

/// Renders the sample in `mode` and compares it with the reference image `name`.
fn check(mode: egui::Theme, name: &str) {
    let mut state = ("0.000".to_owned(), [true, false], 0_usize);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(360.0, 330.0))
        .with_theme(mode)
        .build_ui(move |ui| {
            theme::install(ui.ctx());
            let (text, checked, choice) = &mut state;
            sample(ui, text, checked, choice);
        });
    harness.run();
    harness.run();
    if let Err(error) = harness.try_snapshot_options(name, &snapshot_options()) {
        panic!("{error}");
    }
}

#[test]
fn test_snapshot_theme_sample_dark() {
    check(egui::Theme::Dark, "theme_sample_dark");
}

#[test]
fn test_snapshot_theme_sample_light() {
    check(egui::Theme::Light, "theme_sample_light");
}

#[test]
fn test_theme_metrics_follow_gtk() {
    for mode in [egui::Theme::Dark, egui::Theme::Light] {
        let style = theme::style(mode);
        assert_eq!(style.spacing.item_spacing.y, 6.0);
        assert_eq!(style.spacing.window_margin, egui::Margin::same(12));
        assert_eq!(
            style.visuals.selection.bg_fill,
            theme::palette(mode).accent_bg
        );
    }
}

#[test]
fn test_theme_accent_and_destructive_colours() {
    assert_eq!(
        theme::ACCENT_BLUE,
        egui::Color32::from_rgb(0x35, 0x84, 0xe4)
    );
    assert_ne!(
        theme::destructive_fill(egui::Theme::Dark),
        theme::ACCENT_BLUE
    );
}

#[test]
fn test_theme_checked_checkbox_keeps_label_and_state() {
    use egui_kittest::kittest::Queryable;
    let mut value = false;
    let mut harness = Harness::new_ui(move |ui| {
        theme::install(ui.ctx());
        theme::checkbox(ui, &mut value, "Lock");
    });
    harness.run();
    harness.get_by_label("Lock").click();
    harness.run();
    assert_eq!(
        harness.get_by_label("Lock").accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
}
