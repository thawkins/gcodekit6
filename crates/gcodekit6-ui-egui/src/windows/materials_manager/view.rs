//! Drawing the Materials Manager page: the searchable, category-filtered material list,
//! the questions, the action bar and the five-tab edit form. The state and what the
//! buttons do live in the parent module.
//!
//! Dependencies: `egui`, `gcodekit6-app` (form choices, translations).

use gcodekit6_app::i18n::Translator;
use gcodekit6_app::material_form::{
    ABRASIVENESS, CATEGORIES, CHIP_TYPES, DENSITY_RANGE, HAZARD_LEVELS, HEAT_SENSITIVITIES,
    MACHINABILITY_RANGE, MaterialForm, SURFACE_FINISHES, abrasiveness_label, category_label,
    chip_type_label, hazard_level_label, heat_sensitivity_label, surface_finish_label,
};

use crate::icons::Icon;
use crate::theme;
use crate::windows::help::HelpTopic;
use crate::windows::manager_ui::{self, SIDEBAR_WIDTH, icon_text_button};

use super::{MaterialsManagerWindow, Next, Question, Tab};

/// What the action bar asked for this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Save,
    Cancel,
    Delete,
}

impl MaterialsManagerWindow {
    /// Draws the Materials page into `ui`.
    ///
    /// # Arguments
    /// * `ui` - The page's `Ui`.
    /// * `tr` - Translates the labels.
    pub fn show(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        // The material list sidebar | the edit form.
        ui.horizontal_top(|ui| {
            let size = egui::vec2(SIDEBAR_WIDTH, ui.available_height());
            ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
                self.sidebar(ui, tr);
            });
            ui.separator();
            ui.add_space(12.0);
            ui.vertical(|ui| self.details(ui, tr));
        });
    }

    /// Search, category filter, New Material and the list.
    fn sidebar(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        manager_ui::sidebar_header(
            ui,
            &tr.tr(n_!("Materials")),
            HelpTopic::MaterialsManager,
            tr,
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.filter.search)
                .hint_text(tr.tr(n_!("Search materials...")))
                .desired_width(f32::INFINITY),
        );
        let all = tr.tr(n_!("All Categories"));
        egui::ComboBox::from_id_salt("gcodekit6-materials-category-filter")
            .width(ui.available_width())
            .selected_text(
                self.filter
                    .category
                    .map_or(all.clone(), |c| tr.tr(category_label(c))),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.filter.category, None, all);
                for c in CATEGORIES {
                    ui.selectable_value(
                        &mut self.filter.category,
                        Some(c),
                        tr.tr(category_label(c)),
                    );
                }
            });
        ui.add_space(4.0);
        // New Material sits under the list.
        let new_height = ui.spacing().interact_size.y + 12.0;
        egui::ScrollArea::vertical()
            .id_salt("gcodekit6-materials-manager-list")
            .auto_shrink([false, false])
            .max_height((ui.available_height() - new_height).max(80.0))
            .show(ui, |ui| self.list(ui, tr));
        ui.add_space(6.0);
        let new = theme::primary_icon_button(Icon::Add.image(), tr.tr(n_!("New Material")))
            .min_size(egui::vec2(ui.available_width(), 0.0));
        if ui.add_enabled(self.backend.is_some(), new).clicked() {
            self.request(Next::New);
        }
    }

    fn list(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        let materials = self.materials();
        let selected = self
            .edit
            .as_ref()
            .and_then(|e| e.original.as_ref())
            .map(|m| m.id.0.clone());
        let listed = self.filter.apply(&materials);
        if listed.is_empty() {
            // No empty frame under the message.
            ui.weak(tr.tr(n_!("No materials found")));
            return;
        }
        let machinability = tr.tr(n_!("Machinability:"));
        let mut clicked = None;
        manager_ui::boxed_list(ui, |ui| {
            for (index, material) in listed.into_iter().enumerate() {
                let is_selected = selected.as_deref() == Some(material.id.0.as_str());
                let row = manager_ui::list_row(ui, index == 0, is_selected, &material.name, |ui| {
                    ui.weak(format!(
                        "{} - {}",
                        tr.tr(category_label(material.category)),
                        material.subcategory
                    ));
                    ui.label(format!(
                        "{machinability} {}/10",
                        material.machinability_rating
                    ));
                });
                if row.clicked() {
                    clicked = Some(material.id.0.clone());
                }
            }
        });
        if let Some(id) = clicked
            && selected.as_deref() != Some(id.as_str())
        {
            self.request(Next::Select(id));
        }
    }

    /// The notice, the open question, the action bar and the form.
    fn details(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        manager_ui::notice(ui, self.notice.get());
        self.ask(ui, tr);
        if self.edit.is_none() {
            manager_ui::placeholder(
                ui,
                &tr.tr(n_!("Please select or create a material to edit")),
            );
            return;
        }
        let action = self.action_bar(ui, tr);
        ui.add_space(8.0);
        if let Some(edit) = self.edit.as_mut() {
            let tabs: Vec<_> = Tab::ALL
                .iter()
                .map(|tab| (*tab, tr.tr(tab.msgid()).into_owned()))
                .collect();
            manager_ui::tab_switcher(ui, &mut edit.tab, &tabs);
            ui.add_space(8.0);
            let creating = edit.original.is_none();
            let form = &mut edit.form;
            egui::ScrollArea::vertical()
                .id_salt("gcodekit6-materials-manager-form")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    manager_ui::form_group(ui, None, |ui| match edit.tab {
                        Tab::Basic => basic(ui, form, creating, tr),
                        Tab::Properties => properties(ui, form, tr),
                        Tab::Machining => machining(ui, form, tr),
                        Tab::Safety => safety(ui, form, tr),
                        Tab::Notes => notes(ui, form, tr),
                    });
                });
        }
        match action {
            Some(Action::Save) => {
                self.save(tr);
            }
            Some(Action::Cancel) => self.request(Next::Close),
            Some(Action::Delete) => self.ask_delete(),
            None => {}
        }
    }

    fn action_bar(&self, ui: &mut egui::Ui, tr: &Translator) -> Option<Action> {
        let asking = self.question.is_some();
        let dirty = self.is_dirty();
        let custom = self
            .edit
            .as_ref()
            .and_then(|e| e.original.as_ref())
            .is_some_and(|m| m.custom);
        let mut action = None;
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!asking, |ui| {
                let save = theme::primary_icon_button(Icon::Save.image(), tr.tr(n_!("Save")));
                if ui.add_enabled(dirty, save).clicked() {
                    action = Some(Action::Save);
                }
                if ui
                    .add(icon_text_button(ui, Icon::Close, tr.tr(n_!("Cancel"))))
                    .clicked()
                {
                    action = Some(Action::Cancel);
                }
                let delete =
                    theme::destructive_icon_button(ui, Icon::Trash.image(), tr.tr(n_!("Delete")));
                let delete = ui
                    .add_enabled(custom, delete)
                    .on_disabled_hover_text(tr.tr(n_!("Only custom materials can be deleted.")));
                if delete.clicked() {
                    action = Some(Action::Delete);
                }
            });
        });
        action
    }

    /// The open question (delete, unsaved changes), if any.
    fn ask(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        let Some(question) = self.question.clone() else {
            return;
        };
        let (title, text, choices): (_, String, &[&str]) = match &question {
            Question::Delete(_, name) => (
                n_!("Delete material?"),
                tr.tr(n_!("Delete '{}'? This cannot be undone."))
                    .replacen("{}", name, 1),
                &[n_!("Yes"), n_!("No")],
            ),
            Question::Unsaved(_) => (
                n_!("Unsaved changes"),
                tr.tr(n_!(
                    "The material has unsaved changes. Save them before continuing?"
                ))
                .into_owned(),
                &[n_!("Save"), n_!("Discard"), n_!("Cancel")],
            ),
        };
        let choices: Vec<String> = choices.iter().map(|c| tr.tr(c).into_owned()).collect();
        let answer = manager_ui::question(ui, &tr.tr(title), &text, &choices);
        let Some(answer) = answer else {
            return;
        };
        self.question = None;
        match (question, answer) {
            (Question::Delete(id, name), 0) => self.delete(&id, &name, tr),
            (Question::Unsaved(next), 0) => {
                if self.save(tr) {
                    self.go(next);
                }
            }
            (Question::Unsaved(next), 1) => self.go(next),
            (Question::Delete(..) | Question::Unsaved(_), _) => {}
        }
    }
}

/// A labelled single-line field in a grid row.
fn field(ui: &mut egui::Ui, label: &str, text: &mut String, enabled: bool, hint: &str) {
    let label = ui.label(label);
    ui.add_enabled(enabled, egui::TextEdit::singleline(text).hint_text(hint))
        .labelled_by(label.id);
    ui.end_row();
}

/// A labelled multi-line field.
fn text_area(ui: &mut egui::Ui, label: &str, text: &mut String) {
    let label = ui.label(label);
    ui.add(
        egui::TextEdit::multiline(text)
            .desired_rows(6)
            .desired_width(f32::INFINITY),
    )
    .labelled_by(label.id);
}

/// A labelled combo box over `choices` in a grid row.
fn choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    label: &str,
    id: &str,
    value: &mut T,
    choices: &[T],
    name: impl Fn(T) -> String,
) {
    let label = ui.label(label);
    egui::ComboBox::from_id_salt(id)
        .selected_text(name(*value))
        .show_ui(ui, |ui| {
            for c in choices {
                ui.selectable_value(value, *c, name(*c));
            }
        })
        .response
        .labelled_by(label.id);
    ui.end_row();
}

fn basic(ui: &mut egui::Ui, f: &mut MaterialForm, creating: bool, tr: &Translator) {
    egui::Grid::new("gcodekit6-materials-basic")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            field(ui, &tr.tr(n_!("ID:")), &mut f.id, creating, "unique_id");
            field(ui, &tr.tr(n_!("Name:")), &mut f.name, true, "");
            choice(
                ui,
                &tr.tr(n_!("Category:")),
                "gcodekit6-materials-category",
                &mut f.category,
                &CATEGORIES,
                |c| tr.tr(category_label(c)).into_owned(),
            );
            field(
                ui,
                &tr.tr(n_!("Subcategory:")),
                &mut f.subcategory,
                true,
                &tr.tr(n_!("e.g., Hardwood, Alloy")),
            );
        });
    text_area(ui, &tr.tr(n_!("Description:")), &mut f.description);
}

fn properties(ui: &mut egui::Ui, f: &mut MaterialForm, tr: &Translator) {
    egui::Grid::new("gcodekit6-materials-properties")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            let label = ui.label(tr.tr(n_!("Density (kg/m3):")));
            ui.add(
                egui::DragValue::new(&mut f.density)
                    .range(DENSITY_RANGE)
                    .clamp_existing_to_range(false),
            )
            .labelled_by(label.id);
            ui.end_row();
            let label = ui.label(tr.tr(n_!("Machinability (1-10):")));
            ui.add(
                egui::DragValue::new(&mut f.machinability)
                    .range(MACHINABILITY_RANGE)
                    // A stored value outside the range is kept until the user edits it.
                    .clamp_existing_to_range(false),
            )
            .labelled_by(label.id);
            ui.end_row();
            let optional = tr.tr(n_!("Optional"));
            field(
                ui,
                &tr.tr(n_!("Tensile Strength (MPa):")),
                &mut f.tensile_strength,
                true,
                &optional,
            );
            field(
                ui,
                &tr.tr(n_!("Melting Point (deg C):")),
                &mut f.melting_point,
                true,
                &optional,
            );
        });
}

fn machining(ui: &mut egui::Ui, f: &mut MaterialForm, tr: &Translator) {
    egui::Grid::new("gcodekit6-materials-machining")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            choice(
                ui,
                &tr.tr(n_!("Chip Type:")),
                "gcodekit6-materials-chip",
                &mut f.chip_type,
                &CHIP_TYPES,
                |c| tr.tr(chip_type_label(c)).into_owned(),
            );
            choice(
                ui,
                &tr.tr(n_!("Heat Sensitivity:")),
                "gcodekit6-materials-heat",
                &mut f.heat_sensitivity,
                &HEAT_SENSITIVITIES,
                |h| tr.tr(heat_sensitivity_label(h)).into_owned(),
            );
            choice(
                ui,
                &tr.tr(n_!("Abrasiveness:")),
                "gcodekit6-materials-abrasiveness",
                &mut f.abrasiveness,
                &ABRASIVENESS,
                |a| tr.tr(abrasiveness_label(a)).into_owned(),
            );
            choice(
                ui,
                &tr.tr(n_!("Surface Finish:")),
                "gcodekit6-materials-finish",
                &mut f.surface_finish,
                &SURFACE_FINISHES,
                |s| tr.tr(surface_finish_label(s)).into_owned(),
            );
        });
}

fn safety(ui: &mut egui::Ui, f: &mut MaterialForm, tr: &Translator) {
    egui::Grid::new("gcodekit6-materials-safety")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            choice(
                ui,
                &tr.tr(n_!("Dust Hazard:")),
                "gcodekit6-materials-dust",
                &mut f.dust_hazard,
                &HAZARD_LEVELS,
                |h| tr.tr(hazard_level_label(h)).into_owned(),
            );
            choice(
                ui,
                &tr.tr(n_!("Fume Hazard:")),
                "gcodekit6-materials-fume",
                &mut f.fume_hazard,
                &HAZARD_LEVELS,
                |h| tr.tr(hazard_level_label(h)).into_owned(),
            );
        });
    crate::theme::checkbox(ui, &mut f.coolant_required, tr.tr(n_!("Coolant Required")));
}

fn notes(ui: &mut egui::Ui, f: &mut MaterialForm, tr: &Translator) {
    text_area(ui, &tr.tr(n_!("Additional Notes and Tips:")), &mut f.notes);
}
