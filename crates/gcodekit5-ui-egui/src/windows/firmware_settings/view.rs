//! Drawing the Firmware Settings window: its toolbar, filter, notice, list and the
//! editor of the clicked row. The state and what the buttons do live in the parent module.
//!
//! Dependencies: `egui`, `gcodekit5-app` (settings model, translations).

use std::time::Duration;

use gcodekit5_app::device_identity::DeviceIdentity;
use gcodekit5_app::firmware_settings::{
    ConfigSettingRow, REPETIER_OFFSET, SETTING_CATEGORIES, setting_id,
};
use gcodekit5_app::i18n::Translator;

use super::{Edit, FileRequest, FirmwareSettingsWindow};
use crate::machine::Machine;

impl FirmwareSettingsWindow {
    /// Draws the window (and the open editor) while it is open.
    ///
    /// # Arguments
    /// * `ctx` - The egui context.
    /// * `machine` - The app's machine connection, for reading and writing settings.
    /// * `identity` - What the controller reported about itself (Repetier or GRBL).
    /// * `tr` - Translates the labels.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        machine: &Machine,
        identity: &DeviceIdentity,
        tr: &Translator,
    ) {
        self.tick(tr);
        if let Some(write) = &self.write {
            ctx.request_repaint_after(write.pending.time_left());
        }
        if self.retrieval.is_some() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        if !self.open {
            return;
        }
        let mut open = self.open;
        egui::Window::new(tr.tr(n_!("Firmware Settings")))
            .id(egui::Id::unique("gcodekit5-firmware-settings"))
            .open(&mut open)
            .default_size([900.0, 560.0])
            .show(ctx, |ui| self.contents(ui, machine, identity, tr));
        self.open = open;
        if !self.open {
            self.edit = None;
        }
        self.show_editor(ctx, machine, tr);
    }

    fn contents(
        &mut self,
        ui: &mut egui::Ui,
        machine: &Machine,
        identity: &DeviceIdentity,
        tr: &Translator,
    ) {
        let busy = self.retrieval.is_some() || self.write.is_some();
        ui.horizontal(|ui| {
            let retrieve = ui
                .add_enabled(!busy, egui::Button::new(tr.tr(n_!("Retrieve"))))
                .on_hover_text(tr.tr(n_!("Retrieve Settings from Device ($$)")));
            if retrieve.clicked() {
                self.retrieve(machine, identity, tr);
            }
            let save = ui
                .add_enabled(
                    self.table.count() > 0,
                    egui::Button::new(tr.tr(n_!("Save"))),
                )
                .on_hover_text(tr.tr(n_!("Save Settings to File")));
            if save.clicked() {
                self.file_request = Some(FileRequest::Save);
            }
            let load = ui
                .button(tr.tr(n_!("Load")))
                .on_hover_text(tr.tr(n_!("Load Settings from File")));
            if load.clicked() {
                self.file_request = Some(FileRequest::Load);
            }
        });
        ui.horizontal(|ui| {
            let label = ui.label(tr.tr(n_!("Filter:")));
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text(tr.tr(n_!("Search settings...")))
                    .desired_width(240.0),
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
        let rows = self.visible_rows();
        ui.horizontal(|ui| {
            let not_connected = tr.tr(n_!("Not connected"));
            match &self.notice {
                Some((text, true)) => {
                    ui.colored_label(ui.visuals().error_fg_color, text);
                }
                Some((text, false)) => {
                    ui.label(text);
                }
                None => {}
            }
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
        ui.separator();
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| self.list(ui, &rows, tr));
    }

    fn list(&mut self, ui: &mut egui::Ui, rows: &[ConfigSettingRow], tr: &Translator) {
        egui::Grid::new("gcodekit5-firmware-settings-list")
            .striped(true)
            .num_columns(6)
            .show(ui, |ui| {
                for heading in [
                    n_!("ID"),
                    n_!("Name"),
                    n_!("Value"),
                    n_!("Unit"),
                    n_!("Category"),
                    n_!("Description"),
                ] {
                    ui.strong(tr.tr(heading));
                }
                ui.end_row();
                for row in rows {
                    if row.read_only || !self.reported.contains(&row.number) {
                        ui.weak(row.id());
                    } else if ui.selectable_label(false, row.id()).clicked() {
                        self.edit = Some(Edit {
                            number: row.number,
                            value: row.value.clone(),
                        });
                    }
                    ui.label(&row.name);
                    ui.label(&row.value);
                    ui.weak(&row.unit);
                    ui.weak(tr.tr(&row.category));
                    ui.add(
                        egui::Label::new(egui::RichText::new(&row.description).weak()).truncate(),
                    );
                    ui.end_row();
                }
            });
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
                        .add_enabled(can_apply, egui::Button::new(tr.tr(n_!("Apply"))))
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
