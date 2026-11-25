//! The Text tool's Add Text dialog, as GTK's `open_text_tool_dialog`: a click with the Text
//! tool opens it for the snapped point; it asks for the text, the font family, the size in
//! points and bold/italic. Add (or Enter in the text field) places a text shape centred on
//! the point as one undo step and selects it; Cancel or Escape places nothing, and empty
//! text places nothing. The font, size and style are remembered for the next text.
//!
//! Sizes are typed in points and stored in millimetres (1 pt = 25.4 / 72 mm), as in GTK.
//! The font list is "Sans" followed by the system's families (`font_manager`), read once.
//!
//! Dependencies: `egui`, `gcodekit5-designer` (model, fonts), `gcodekit5-app`
//! (translations).

use std::sync::OnceLock;

use gcodekit5_app::i18n::Translator;
use gcodekit5_designer::DesignerState;
use gcodekit5_designer::font_manager;
use gcodekit5_designer::model::{DesignText, Shape};

/// Millimetres per typographic point.
pub const MM_PER_PT: f64 = 25.4 / 72.0;
/// GTK's default text size, in points.
const DEFAULT_SIZE_PT: f64 = 20.0;

/// The msgids the dialog shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Add Text"),
        n_!("Text"),
        n_!("Enter text"),
        n_!("Font:"),
        n_!("Size:"),
        n_!("Style:"),
        n_!("Bold"),
        n_!("Italic"),
        n_!("Cancel"),
        n_!("Add"),
    ]
}

/// `mm` in points.
pub fn mm_to_pt(mm: f64) -> f64 {
    mm / MM_PER_PT
}

/// A size in millimetres shown in points with 2 decimals, as GTK shows it.
pub fn format_points(mm: f64) -> String {
    format!("{:.2}", mm_to_pt(mm))
}

/// The millimetres of a size typed in points ("12", "12pt", "10,5"); `None` unless positive.
///
/// # Arguments
/// * `text` - The typed size.
pub fn parse_points_mm(text: &str) -> Option<f64> {
    let cleaned = text.trim().trim_end_matches("pt").trim().replace(',', ".");
    let pt = cleaned.parse::<f64>().ok()?;
    (pt > 0.0 && pt.is_finite()).then_some(pt * MM_PER_PT)
}

/// "Sans" followed by the system's font families (read once).
pub fn font_families() -> &'static [String] {
    static FAMILIES: OnceLock<Vec<String>> = OnceLock::new();
    FAMILIES.get_or_init(|| {
        let mut families = vec!["Sans".to_string()];
        families.extend(
            font_manager::list_font_families()
                .into_iter()
                .filter(|f| f != "Sans"),
        );
        families
    })
}

/// A font family picker listing [`font_families`]; returns whether the user picked one.
///
/// # Arguments
/// * `ui` - Where to show it.
/// * `id_salt` - Distinguishes it from other pickers.
/// * `family` - The family shown, updated with the pick.
/// * `label_id` - The label naming the picker.
pub fn font_combo(
    ui: &mut egui::Ui,
    id_salt: &str,
    family: &mut String,
    label_id: egui::Id,
) -> bool {
    let mut picked = false;
    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(family.as_str())
        .show_ui(ui, |ui| {
            for f in font_families() {
                if ui.selectable_label(family == f, f).clicked() {
                    picked = *family != *f;
                    family.clone_from(f);
                }
            }
        })
        .response
        .labelled_by(label_id);
    picked
}

/// The Add Text dialog and the font settings it remembers.
#[derive(Clone, Debug)]
pub struct TextTool {
    /// The point the dialog is open for (world mm), or `None` when it is closed.
    at: Option<(f64, f64)>,
    text: String,
    family: String,
    bold: bool,
    italic: bool,
    /// The size field's text, in points.
    size: String,
    /// The size of the last text added, mm.
    last_size_mm: f64,
    /// Whether the text field takes the keyboard on the next frame (just opened).
    focus_text: bool,
}

impl Default for TextTool {
    fn default() -> Self {
        let last_size_mm = DEFAULT_SIZE_PT * MM_PER_PT;
        Self {
            at: None,
            text: String::new(),
            family: "Sans".to_string(),
            bold: false,
            italic: false,
            size: format_points(last_size_mm),
            last_size_mm,
            focus_text: false,
        }
    }
}

impl TextTool {
    /// Opens the dialog for world point `at`, with empty text and the last font settings.
    ///
    /// # Arguments
    /// * `at` - Where the text's centre goes, mm.
    pub fn open(&mut self, at: (f64, f64)) {
        self.at = Some(at);
        self.focus_text = true;
        self.text.clear();
        self.size = format_points(self.last_size_mm);
    }

    /// Whether the dialog is open.
    pub fn is_open(&self) -> bool {
        self.at.is_some()
    }

    /// Shows the dialog while it is open; Add places the text in `state`.
    ///
    /// # Arguments
    /// * `ctx` - The egui context (the dialog is modal).
    /// * `state` - The design.
    /// * `tr` - Translator for the current UI language.
    pub fn show(&mut self, ctx: &egui::Context, state: &mut DesignerState, tr: &Translator) {
        let Some(at) = self.at else {
            return;
        };
        let mut add = false;
        let mut cancel = false;
        let response =
            egui::Modal::new(egui::Id::unique("gcodekit5-designer-add-text")).show(ctx, |ui| {
                ui.heading(tr.tr("Add Text"));
                let text_label = ui.label(tr.tr("Text"));
                let field = ui
                    .add(
                        egui::TextEdit::singleline(&mut self.text)
                            .hint_text(tr.tr("Enter text"))
                            .desired_width(260.0),
                    )
                    .labelled_by(text_label.id);
                // GTK focuses the text entry when the dialog opens.
                if std::mem::take(&mut self.focus_text) {
                    field.request_focus();
                }
                // GTK's entry activates the default (Add) button.
                add = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                egui::Grid::new("gcodekit5-add-text-grid")
                    .num_columns(2)
                    .show(ui, |ui| {
                        let font_label = ui.label(tr.tr("Font:"));
                        font_combo(
                            ui,
                            "gcodekit5-add-text-font",
                            &mut self.family,
                            font_label.id,
                        );
                        ui.end_row();
                        let size_label = ui.label(tr.tr("Size:"));
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut self.size).desired_width(60.0))
                                .labelled_by(size_label.id);
                            ui.label("pt");
                        });
                        ui.end_row();
                        ui.label(tr.tr("Style:"));
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.bold, tr.tr("Bold"));
                            ui.checkbox(&mut self.italic, tr.tr("Italic"));
                        });
                        ui.end_row();
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    cancel = ui.button(tr.tr("Cancel")).clicked();
                    add |= ui.button(tr.tr("Add")).clicked();
                });
            });
        if add {
            self.add(state, at);
        }
        if add || cancel || response.should_close() {
            self.at = None;
        }
    }

    /// Places the typed text at `at` (nothing for empty text), as one undo step, selected.
    fn add(&mut self, state: &mut DesignerState, at: (f64, f64)) {
        let text = self.text.trim().to_string();
        if text.is_empty() {
            return;
        }
        let size_mm = parse_points_mm(&self.size).unwrap_or(DEFAULT_SIZE_PT * MM_PER_PT);
        self.last_size_mm = size_mm;
        let mut shape = DesignText::new(text, at.0, at.1, size_mm);
        shape.font_family.clone_from(&self.family);
        shape.bold = self.bold;
        shape.italic = self.italic;
        let id = state.add_shape_with_undo(Shape::Text(shape));
        state.canvas.deselect_all();
        state.canvas.select_shape(id, false);
    }
}
