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
use crate::machine::Machine;
use crate::windows::help::{self, HelpTopic};

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
        ui.horizontal(|ui| {
            let retrieve = ui
                .add_enabled(!busy, egui::Button::new(tr.tr(n_!("Retrieve"))))
                .on_hover_text(tr.tr(n_!("Retrieve Settings from Device ($$)")));
            if retrieve.clicked() {
                self.retrieve(machine, identity, profile_repetier, tr);
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
            help::help_button(ui, HelpTopic::DeviceConfig, tr);
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
