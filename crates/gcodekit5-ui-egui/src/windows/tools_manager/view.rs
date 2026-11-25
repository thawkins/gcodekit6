//! Drawing the CNC Tools window: the searchable, filtered tool list, the library actions,
//! the questions, the action bar and the four-tab edit form. The state and what the
//! buttons do live in the parent module.
//!
//! Dependencies: `egui`, `gcodekit5-app` (form choices, translations).

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::tool_form::{
    COATINGS, ShankChoice, TOOL_MATERIALS, TOOL_TYPES, ToolForm, coating_label, has_corner_radius,
    has_tip_angle, tool_material_label, tool_summary, tool_type_label,
};

use super::{GtcFile, Next, Question, Tab, ToolsFileRequest, ToolsManagerWindow};

/// What the action bar asked for this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Save,
    Cancel,
    Delete,
}

impl ToolsManagerWindow {
    /// Draws the window while it is open.
    ///
    /// # Arguments
    /// * `ctx` - The egui context.
    /// * `tr` - Translates the labels.
    pub fn show(&mut self, ctx: &egui::Context, tr: &Translator) {
        if !self.open {
            return;
        }
        let mut open = self.open;
        egui::Window::new(tr.tr(n_!("CNC Tools")))
            .id(egui::Id::unique("gcodekit5-tools-manager"))
            .open(&mut open)
            .default_size([1000.0, 600.0])
            .show(ctx, |ui| self.contents(ui, tr));
        self.open = open;
        if !self.open {
            self.question = None;
        }
    }

    fn contents(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        ui.horizontal_top(|ui| {
            let size = egui::vec2(300.0, ui.available_height());
            ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
                self.sidebar(ui, tr);
            });
            ui.separator();
            let width = (ui.available_width() - 220.0).max(300.0);
            let size = egui::vec2(width, ui.available_height());
            ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
                self.details(ui, tr);
            });
            ui.separator();
            ui.vertical(|ui| self.library_actions(ui, tr));
        });
    }

    /// Search, filters and the tool list.
    fn sidebar(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        ui.add(
            egui::TextEdit::singleline(&mut self.filter.search)
                .hint_text(tr.tr(n_!("Search (name, id, type, diameter)...")))
                .desired_width(f32::INFINITY),
        );
        let all_types = tr.tr(n_!("All Types"));
        egui::ComboBox::from_id_salt("gcodekit5-tools-type-filter")
            .width(ui.available_width())
            .selected_text(
                self.filter
                    .tool_type
                    .map_or(all_types.clone(), |t| tr.tr(tool_type_label(t))),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.filter.tool_type, None, all_types);
                for t in TOOL_TYPES {
                    ui.selectable_value(
                        &mut self.filter.tool_type,
                        Some(t),
                        tr.tr(tool_type_label(t)),
                    );
                }
            });
        let all_materials = tr.tr(n_!("All Materials"));
        egui::ComboBox::from_id_salt("gcodekit5-tools-material-filter")
            .width(ui.available_width())
            .selected_text(
                self.filter
                    .material
                    .map_or(all_materials.clone(), |m| tr.tr(tool_material_label(m))),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.filter.material, None, all_materials);
                for m in TOOL_MATERIALS {
                    let text = tr.tr(tool_material_label(m));
                    ui.selectable_value(&mut self.filter.material, Some(m), text);
                }
            });
        ui.label(tr.tr(n_!("Filters")));
        ui.horizontal(|ui| {
            let error = ui.visuals().error_fg_color;
            let min_bad = self.filter.dia_min_invalid();
            let max_bad = self.filter.dia_max_invalid();
            let min = egui::TextEdit::singleline(&mut self.filter.dia_min)
                .hint_text(tr.tr(n_!("Min diameter (mm)")))
                .desired_width(120.0);
            ui.add(if min_bad { min.text_color(error) } else { min });
            let max = egui::TextEdit::singleline(&mut self.filter.dia_max)
                .hint_text(tr.tr(n_!("Max diameter (mm)")))
                .desired_width(120.0);
            ui.add(if max_bad { max.text_color(error) } else { max });
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("gcodekit5-tools-manager-list")
            .auto_shrink([false, false])
            .show(ui, |ui| self.list(ui, tr));
    }

    fn list(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        let tools = self.tools();
        let selected = self
            .edit
            .as_ref()
            .and_then(|e| e.original.as_ref())
            .map(|t| t.id.0.clone());
        let none = tr.tr(n_!("None"));
        let mut clicked = None;
        for tool in self.filter.apply(&tools) {
            let is_selected = selected.as_deref() == Some(tool.id.0.as_str());
            if ui.selectable_label(is_selected, &tool.name).clicked() {
                clicked = Some(tool.id.0.clone());
            }
            let type_name = tr.tr(tool_type_label(tool.tool_type));
            ui.add(
                egui::Label::new(egui::RichText::new(tool_summary(tool, &type_name, &none)).weak())
                    .wrap(),
            );
            ui.add_space(4.0);
        }
        if let Some(id) = clicked
            && selected.as_deref() != Some(id.as_str())
        {
            self.request(Next::Select(id));
        }
    }

    /// New Tool and the Library actions (GTK's right-hand sidebar).
    fn library_actions(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        let has_store = self.backend.is_some();
        let new = ui
            .add_enabled(has_store, egui::Button::new(tr.tr(n_!("New Tool"))))
            .on_hover_text(tr.tr(n_!("Create a new custom tool")));
        if new.clicked() {
            self.request(Next::New);
        }
        ui.separator();
        ui.strong(tr.tr(n_!("Library")));
        let buttons = [
            (
                n_!("Import GTC (.zip)..."),
                n_!("Import tool catalog from a GTC zip package"),
                ToolsFileRequest::Import(GtcFile::Package),
            ),
            (
                n_!("Import GTC (.json)..."),
                n_!("Import tool catalog from a GTC JSON file"),
                ToolsFileRequest::Import(GtcFile::Json),
            ),
            (
                n_!("Export Custom Tools..."),
                n_!("Export custom tools to a JSON file"),
                ToolsFileRequest::Export,
            ),
        ];
        for (label, hint, request) in buttons {
            let button = ui
                .add_enabled(has_store, egui::Button::new(tr.tr(label)))
                .on_hover_text(tr.tr(hint));
            if button.clicked() {
                self.file_request = Some(request);
            }
        }
        let reset = ui
            .add_enabled(
                has_store,
                egui::Button::new(tr.tr(n_!("Reset Custom Tools..."))),
            )
            .on_hover_text(tr.tr(n_!(
                "Remove all custom/imported tools and delete local storage"
            )));
        if reset.clicked() {
            self.question = Some(Question::Reset);
        }
    }

    /// The notice, the open question, the action bar and the form.
    fn details(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        match &self.notice {
            Some((text, true)) => {
                ui.colored_label(ui.visuals().error_fg_color, text);
            }
            Some((text, false)) => {
                ui.label(text);
            }
            None => {}
        }
        self.ask(ui, tr);
        if self.edit.is_none() {
            ui.centered_and_justified(|ui| {
                ui.weak(tr.tr(n_!("Please select or create a tool to edit")));
            });
            return;
        }
        let action = self.action_bar(ui, tr);
        ui.separator();
        if let Some(edit) = self.edit.as_mut() {
            ui.horizontal(|ui| {
                for tab in Tab::ALL {
                    ui.selectable_value(&mut edit.tab, tab, tr.tr(tab.msgid()));
                }
            });
            ui.separator();
            let creating = edit.original.is_none();
            let form = &mut edit.form;
            egui::ScrollArea::vertical()
                .id_salt("gcodekit5-tools-manager-form")
                .auto_shrink([false, false])
                .show(ui, |ui| match edit.tab {
                    Tab::Basic => basic(ui, form, creating, tr),
                    Tab::Geometry => geometry(ui, form, tr),
                    Tab::Manufacturer => manufacturer(ui, form, tr),
                    Tab::Notes => notes(ui, form, tr),
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
            .is_some_and(|t| t.custom);
        let mut action = None;
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!asking, |ui| {
                let save = ui
                    .add_enabled(dirty, egui::Button::new(tr.tr(n_!("Save"))))
                    .on_hover_text(tr.tr(n_!("Save changes")));
                if save.clicked() {
                    action = Some(Action::Save);
                }
                let cancel = ui
                    .button(tr.tr(n_!("Cancel")))
                    .on_hover_text(tr.tr(n_!("Discard changes")));
                if cancel.clicked() {
                    action = Some(Action::Cancel);
                }
                let hint = if custom {
                    n_!("Delete this tool")
                } else {
                    n_!("Only custom tools can be deleted.")
                };
                let delete = ui
                    .add_enabled(custom, egui::Button::new(tr.tr(n_!("Delete"))))
                    .on_hover_text(tr.tr(hint))
                    .on_disabled_hover_text(tr.tr(hint));
                if delete.clicked() {
                    action = Some(Action::Delete);
                }
            });
        });
        action
    }

    /// The open question (delete, reset, unsaved changes), if any.
    fn ask(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        let Some(question) = self.question.clone() else {
            return;
        };
        let (title, text, choices): (_, String, &[&str]) = match &question {
            Question::Delete(_, name) => (
                n_!("Delete tool?"),
                tr.tr(n_!("Delete '{}'? This cannot be undone."))
                    .replacen("{}", name, 1),
                &[n_!("Yes"), n_!("No")],
            ),
            Question::Reset => (
                n_!("Reset custom tools?"),
                tr.tr(n_!(
                    "This will delete all custom/imported tools and remove local custom tools storage."
                ))
                .into_owned(),
                &[n_!("Yes"), n_!("No")],
            ),
            Question::Unsaved(_) => (
                n_!("Unsaved changes"),
                tr.tr(n_!(
                    "The tool has unsaved changes. Save them before continuing?"
                ))
                .into_owned(),
                &[n_!("Save"), n_!("Discard"), n_!("Cancel")],
            ),
        };
        let mut answer = None;
        ui.group(|ui| {
            ui.strong(tr.tr(title));
            ui.label(text);
            ui.horizontal(|ui| {
                for (index, choice) in choices.iter().enumerate() {
                    if ui.button(tr.tr(choice)).clicked() {
                        answer = Some(index);
                    }
                }
            });
        });
        let Some(answer) = answer else {
            return;
        };
        self.question = None;
        match (question, answer) {
            (Question::Delete(id, name), 0) => self.delete(&id, &name, tr),
            (Question::Reset, 0) => self.reset(tr),
            (Question::Unsaved(next), 0) => {
                if self.save(tr) {
                    self.go(next);
                }
            }
            (Question::Unsaved(next), 1) => self.go(next),
            (Question::Delete(..) | Question::Reset | Question::Unsaved(_), _) => {}
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

/// A labelled combo box over `choices`.
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

fn basic(ui: &mut egui::Ui, f: &mut ToolForm, creating: bool, tr: &Translator) {
    egui::Grid::new("gcodekit5-tools-basic")
        .num_columns(2)
        .show(ui, |ui| {
            field(ui, &tr.tr(n_!("ID:")), &mut f.id, creating, "tool_id");
            let label = ui.label(tr.tr(n_!("Tool Number:")));
            ui.add(
                egui::DragValue::new(&mut f.number)
                    .range(1..=999)
                    // A stored value outside the range (a loaded tool has number 0) is
                    // kept until the user edits it.
                    .clamp_existing_to_range(false),
            )
            .labelled_by(label.id);
            ui.end_row();
            field(ui, &tr.tr(n_!("Name:")), &mut f.name, true, "");
            let mut tool_type = f.tool_type;
            choice(
                ui,
                &tr.tr(n_!("Tool Type:")),
                "gcodekit5-tools-type",
                &mut tool_type,
                &TOOL_TYPES,
                |t| tr.tr(tool_type_label(t)).into_owned(),
            );
            if tool_type != f.tool_type {
                f.set_tool_type(tool_type);
            }
            choice(
                ui,
                &tr.tr(n_!("Tool Material:")),
                "gcodekit5-tools-material",
                &mut f.material,
                &TOOL_MATERIALS,
                |m| tr.tr(tool_material_label(m)).into_owned(),
            );
            choice(
                ui,
                &tr.tr(n_!("Coating:")),
                "gcodekit5-tools-coating",
                &mut f.coating,
                &COATINGS,
                |c| tr.tr(coating_label(c)).into_owned(),
            );
            choice(
                ui,
                &tr.tr(n_!("Shank:")),
                "gcodekit5-tools-shank",
                &mut f.shank,
                &ShankChoice::ALL,
                |s| tr.tr(s.label()).into_owned(),
            );
        });
}

fn geometry(ui: &mut egui::Ui, f: &mut ToolForm, tr: &Translator) {
    egui::Grid::new("gcodekit5-tools-geometry")
        .num_columns(2)
        .show(ui, |ui| {
            field(ui, &tr.tr(n_!("Diameter (mm):")), &mut f.diameter, true, "");
            field(ui, &tr.tr(n_!("Length (mm):")), &mut f.length, true, "");
            field(
                ui,
                &tr.tr(n_!("Flute Length (mm):")),
                &mut f.flute_length,
                true,
                "",
            );
            field(
                ui,
                &tr.tr(n_!("Shaft Diameter (mm):")),
                &mut f.shaft_diameter,
                true,
                "",
            );
            let label = ui.label(tr.tr(n_!("Number of Flutes:")));
            ui.add(
                egui::DragValue::new(&mut f.flutes)
                    .range(1..=8)
                    // An imported tool may have more flutes; drawing must not change it.
                    .clamp_existing_to_range(false),
            )
            .labelled_by(label.id);
            ui.end_row();
            field(
                ui,
                &tr.tr(n_!("Corner Radius (mm):")),
                &mut f.corner_radius,
                has_corner_radius(f.tool_type),
                &tr.tr(n_!("Only for corner radius end mills")),
            );
            field(
                ui,
                &tr.tr(n_!("Tip Angle (deg):")),
                &mut f.tip_angle,
                has_tip_angle(f.tool_type),
                &tr.tr(n_!("Used for drills / V-bits")),
            );
        });
}

fn manufacturer(ui: &mut egui::Ui, f: &mut ToolForm, tr: &Translator) {
    egui::Grid::new("gcodekit5-tools-manufacturer")
        .num_columns(2)
        .show(ui, |ui| {
            field(
                ui,
                &tr.tr(n_!("Manufacturer:")),
                &mut f.manufacturer,
                true,
                "",
            );
            field(
                ui,
                &tr.tr(n_!("Part Number:")),
                &mut f.part_number,
                true,
                "",
            );
        });
    text_area(ui, &tr.tr(n_!("Description:")), &mut f.description);
}

fn notes(ui: &mut egui::Ui, f: &mut ToolForm, tr: &Translator) {
    text_area(ui, &tr.tr(n_!("Additional Notes:")), &mut f.notes);
}
