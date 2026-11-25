//! The About window: the app's name, version and licence.
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations).

use gcodekit5_app::i18n::Translator;

/// The app's name as the About window shows it.
pub const APP_NAME: &str = "GCodeKit";

/// The About window.
#[derive(Debug, Default)]
pub struct AboutWindow {
    open: bool,
}

impl AboutWindow {
    /// Opens the window.
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Whether the window is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Draws the window when it is open.
    ///
    /// # Arguments
    /// * `ctx` - The egui context.
    /// * `tr` - Translates the labels.
    pub fn show(&mut self, ctx: &egui::Context, tr: &Translator) {
        let mut close = false;
        egui::Window::new(tr.tr(n_!("About")))
            .id(egui::Id::unique("gcodekit5-about"))
            .open(&mut self.open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading(APP_NAME);
                    ui.label(format!(
                        "{} {}",
                        tr.tr(n_!("Version")),
                        env!("CARGO_PKG_VERSION")
                    ));
                    ui.label(format!(
                        "{} {}",
                        tr.tr(n_!("License:")),
                        env!("CARGO_PKG_LICENSE")
                    ));
                    ui.add_space(8.0);
                    if ui.button(tr.tr(n_!("Close"))).clicked() {
                        close = true;
                    }
                });
            });
        if close {
            self.open = false;
        }
    }
}

/// The English msgids this window shows, for the translation test.
pub fn msgids() -> Vec<&'static str> {
    vec![n_!("About"), n_!("Version"), n_!("License:"), n_!("Close")]
}
