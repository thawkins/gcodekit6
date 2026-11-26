//! Drawing the Firmware Settings page: its toolbar, filter, notice, list and the
//! editor of the clicked row. The state and what the buttons do live in the parent module.
//!
//! Dependencies: `egui`, `gcodekit5-app` (settings model, translations),
//! `gcodekit5-devicedb` (the active profile's controller type).

use std::time::Duration;

use gcodekit5_app::device_identity::DeviceIdentity;
use gcodekit5_app::firmware_settings::{
    ConfigSettingRow, GrblFamily, REPETIER_OFFSET, SETTING_CATEGORIES, setting_id,
};
use gcodekit5_app::i18n::Translator;
use gcodekit5_devicedb::ControllerType;

use super::{Edit, FileRequest, FirmwareSettingsWindow};
use crate::icons::Icon;
use crate::machine::Machine;
use crate::windows::help::{self, HelpTopic};
use crate::windows::manager_ui::{self, icon_text_button};

/// GTK's column widths of the settings list: ID, Name, Value, Unit and Category; the
/// Description takes the rest.
const COLUMNS: [f32; 5] = [50.0, 200.0, 100.0, 80.0, 150.0];
/// Height of a row of the settings list.
const ROW_HEIGHT: f32 = 30.0;
/// Room left of the first column, inside the list's frame.
const LIST_MARGIN: f32 = 10.0;

/// One cell of the settings list (or its header): `width` points wide, its contents
/// vertically centred and cut off at its right edge.
fn cell(ui: &mut egui::Ui, width: f32, add: impl FnOnce(&mut egui::Ui)) {
    let size = egui::vec2(width, ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(
        size,
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_width(width - ui.spacing().item_spacing.x);
            ui.set_clip_rect(ui.max_rect().intersect(ui.clip_rect()));
            add(ui);
        },
    );
}

impl FirmwareSettingsWindow {
    /// Keeps the settings' reads and writes going. The app calls this once every frame,
    /// whether or not the Device Config page is shown.
    ///
    /// # Arguments
    /// * `ctx` - The egui context.
    /// * `identity` - What the controller reported about itself (Repetier or GRBL).
    /// * `profile_controller` - The active device profile's controller type, if any.
    /// * `tr` - Translates the notices.
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        identity: &DeviceIdentity,
        profile_controller: Option<ControllerType>,
        tr: &Translator,
    ) {
        self.tick(tr);
        self.family = GrblFamily::detect(identity, profile_controller);
        if let Some(write) = &self.write {
            ctx.request_repaint_after(write.pending.time_left());
        }
        if self.retrieval.is_some() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    /// Draws the Device Config page (and the open editor) into `ui`.
    ///
    /// # Arguments
    /// * `ui` - The page's `Ui`.
    /// * `machine` - The app's machine connection, for reading and writing settings.
    /// * `identity` - What the controller reported about itself (Repetier or GRBL).
    /// * `profile_controller` - The active device profile's controller type, if any.
    /// * `tr` - Translates the labels.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        machine: &Machine,
        identity: &DeviceIdentity,
        profile_controller: Option<ControllerType>,
        tr: &Translator,
    ) {
        let profile_repetier = profile_controller == Some(ControllerType::Repetier);
        self.contents(ui, machine, identity, profile_repetier, tr);
        self.show_editor(ui.ctx(), machine, tr);
    }

    fn contents(
        &mut self,
        ui: &mut egui::Ui,
        machine: &Machine,
        identity: &DeviceIdentity,
        profile_repetier: bool,
        tr: &Translator,
    ) {
        let busy = self.retrieval.is_some() || self.write.is_some();
        // GTK's toolbar: Retrieve, Save, Load, and the help button at the right end.
        ui.horizontal(|ui| {
            let retrieve = ui
                .add_enabled(
                    !busy,
                    icon_text_button(ui, Icon::Refresh, tr.tr(n_!("Retrieve"))),
                )
                .on_hover_text(tr.tr(n_!("Retrieve Settings from Device ($$)")));
            if retrieve.clicked() {
                self.retrieve(machine, identity, profile_repetier, tr);
            }
            let save = ui
                .add_enabled(
                    self.table.count() > 0,
                    icon_text_button(ui, Icon::Save, tr.tr(n_!("Save"))),
                )
                .on_hover_text(tr.tr(n_!("Save Settings to File")));
            if save.clicked() {
                self.file_request = Some(FileRequest::Save);
            }
            let load = ui
                .add(icon_text_button(ui, Icon::Open, tr.tr(n_!("Load"))))
                .on_hover_text(tr.tr(n_!("Load Settings from File")));
            if load.clicked() {
                self.file_request = Some(FileRequest::Load);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                help::help_button(ui, HelpTopic::DeviceConfig, tr);
            });
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let label = ui.label(tr.tr(n_!("Filter:")));
            let category_width = 260.0;
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text(tr.tr(n_!("Search settings...")))
                    .desired_width((ui.available_width() - category_width).max(200.0)),
            )
            .labelled_by(label.id);
            let label = ui.label(tr.tr(n_!("Category:")));
            egui::ComboBox::from_id_salt("gcodekit5-firmware-settings-category")
                .selected_text(tr.tr(self.category))
                .show_ui(ui, |ui| {
                    for &category in SETTING_CATEGORIES {
                        ui.selectable_value(&mut self.category, category, tr.tr(category));
                    }
                })
                .response
                .labelled_by(label.id);
        });
        ui.add_space(6.0);
        let rows = self.visible_rows();
        // The list's header row, then the boxed list; GTK's status line sits under it.
        let status_height = ui.spacing().interact_size.y + 8.0;
        ui.horizontal(|ui| {
            ui.add_space(LIST_MARGIN);
            for (heading, width) in [
                n_!("ID"),
                n_!("Name"),
                n_!("Value"),
                n_!("Unit"),
                n_!("Category"),
            ]
            .into_iter()
            .zip(COLUMNS)
            {
                cell(ui, width, |ui| {
                    crate::theme::section_heading(ui, &tr.tr(heading));
                });
            }
            crate::theme::section_heading(ui, &tr.tr(n_!("Description")));
        });
        ui.add_space(4.0);
        let list_height = (ui.available_height() - status_height).max(60.0);
        manager_ui::boxed_list(ui, |ui| {
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .max_height(list_height)
                .show(ui, |ui| self.list(ui, &rows, tr));
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let not_connected = tr.tr(n_!("Not connected"));
            manager_ui::notice(ui, self.notice.as_ref());
            let shown = self.notice.as_ref().map(|(text, _)| text.as_str());
            if !machine.status().connected && shown != Some(&*not_connected) {
                ui.weak(not_connected);
            }
            let count = format!(
                "{} / {} {}",
                rows.len(),
                self.table.count(),
                tr.tr(n_!("settings"))
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak(count);
            });
        });
    }

    fn list(&mut self, ui: &mut egui::Ui, rows: &[ConfigSettingRow], tr: &Translator) {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (index, row) in rows.iter().enumerate() {
            let top = ui.cursor().top();
            let background = ui.painter().add(egui::Shape::Noop);
            ui.horizontal(|ui| {
                ui.set_min_height(ROW_HEIGHT);
                ui.add_space(LIST_MARGIN);
                // GTK's row colours: the name in the accent colour, the value green, the
                // rest dim.
                cell(ui, COLUMNS[0], |ui| {
                    if row.read_only || !self.reported.contains(&row.number) {
                        ui.weak(row.id());
                    } else if ui.selectable_label(false, row.id()).clicked() {
                        self.edit = Some(Edit {
                            number: row.number,
                            value: row.value.clone(),
                        });
                    }
                });
                cell(ui, COLUMNS[1], |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&row.name).color(crate::theme::ACCENT_BLUE),
                        )
                        .truncate(),
                    );
                });
                cell(ui, COLUMNS[2], |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&row.value).color(crate::theme::ACTIVE_BADGE),
                        )
                        .truncate(),
                    );
                });
                cell(ui, COLUMNS[3], |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(&row.unit).weak()).truncate());
                });
                cell(ui, COLUMNS[4], |ui| {
                    let category = tr.tr(&row.category);
                    ui.add(egui::Label::new(egui::RichText::new(category).weak()).truncate());
                });
                ui.add(egui::Label::new(egui::RichText::new(&row.description).weak()).truncate());
            });
            if index % 2 == 1 {
                let rect =
                    egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=ui.cursor().top());
                let fill = ui.visuals().faint_bg_color;
                ui.painter()
                    .set(background, egui::Shape::rect_filled(rect, 0.0, fill));
            }
        }
    }

    /// Draws the editor of the clicked row while it is open.
    fn show_editor(&mut self, ctx: &egui::Context, machine: &Machine, tr: &Translator) {
        // One command at a time: a write's answer is told by the order of the replies.
        let can_apply = self.write.is_none() && self.retrieval.is_none();
        let Some(edit) = self.edit.as_mut() else {
            return;
        };
        let Some(setting) = self.table.get_setting(edit.number).cloned() else {
            self.edit = None;
            return;
        };
        let title = if edit.number >= REPETIER_OFFSET {
            format!(
                "{} {}",
                tr.tr(n_!("Edit EEPROM Pos")),
                edit.number - REPETIER_OFFSET
            )
        } else {
            format!("{} {}", tr.tr(n_!("Edit Setting")), setting_id(edit.number))
        };
        let (mut apply, mut cancel) = (false, false);
        egui::Window::new(title)
            .id(egui::Id::unique("gcodekit5-firmware-settings-edit"))
            // Above the list it edits, which a click would otherwise raise over it.
            .order(egui::Order::Foreground)
            // Centred over the page, clear of its toolbar (Retrieve, Save, Load).
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("gcodekit5-firmware-settings-edit-grid")
                    .num_columns(3)
                    .show(ui, |ui| {
                        ui.strong(tr.tr(n_!("Name:")));
                        ui.label(&setting.name);
                        ui.end_row();
                        ui.strong(tr.tr(n_!("Description:")));
                        ui.weak(&setting.description);
                        ui.end_row();
                        let label = ui.strong(tr.tr(n_!("Value:")));
                        ui.add(egui::TextEdit::singleline(&mut edit.value))
                            .labelled_by(label.id);
                        ui.weak(setting.unit.as_deref().unwrap_or(""));
                        ui.end_row();
                    });
                ui.horizontal(|ui| {
                    cancel = ui.button(tr.tr(n_!("Cancel"))).clicked();
                    apply = ui
                        .add_enabled(can_apply, crate::theme::primary_button(tr.tr(n_!("Apply"))))
                        .clicked();
                });
            });
        if apply {
            self.apply_edit(machine, tr);
        } else if cancel {
            self.edit = None;
        }
    }
}
