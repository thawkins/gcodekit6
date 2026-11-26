//! Drawing the Device Manager page: the searchable device list, the action bar, the
//! delete question and the four-tab edit form. The state and what the buttons do live in
//! the parent module.
//!
//! Dependencies: `egui`, `gcodekit5-app` (form choices, translations), `gcodekit5-core`
//! (unit labels), `gcodekit5-settings` (the user's units).

use gcodekit5_app::device_profiles::{
    BAUD_RATES, CONNECTION_TYPES, CONTROLLER_CHOICES, DEVICE_TYPES, ProfileForm, controller_label,
    device_type_label,
};
use gcodekit5_app::i18n::Translator;
use gcodekit5_core::units::get_unit_label;
use gcodekit5_settings::config::Config;

use super::{DeviceManagerWindow, Edit, Tab};
use crate::machine::Machine;
use crate::windows::help::{HelpTopic, help_row};

/// What the action bar asked for this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Save,
    Cancel,
    Delete,
    Sync,
    SetActive,
}

impl DeviceManagerWindow {
    /// Draws the Device Manager page into `ui`.
    ///
    /// # Arguments
    /// * `ui` - The page's `Ui`.
    /// * `machine` - The app's machine connection (job running, connected).
    /// * `config` - The settings (the user's length and feed units).
    /// * `tr` - Translates the labels.
    pub fn show(&mut self, ui: &mut egui::Ui, machine: &Machine, config: &Config, tr: &Translator) {
        let (units, feed_units) = (config.ui.measurement_system, config.ui.feed_rate_units);
        help_row(ui, HelpTopic::DeviceManager, tr);
        ui.horizontal_top(|ui| {
            let size = egui::vec2(250.0, ui.available_height());
            ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.strong(tr.tr(n_!("Devices")));
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text(tr.tr(n_!("Search devices...")))
                        .desired_width(f32::INFINITY),
                );
                if ui
                    .add_enabled(
                        self.store.is_some(),
                        egui::Button::new(tr.tr(n_!("Add Device"))),
                    )
                    .clicked()
                {
                    self.add(units, feed_units, tr);
                }
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("gcodekit5-device-manager-list")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.list(ui, units, feed_units, tr));
            });
            ui.separator();
            ui.vertical(|ui| self.details(ui, machine, tr));
        });
    }

    /// The notice, the delete question, the action bar and the form.
    fn details(&mut self, ui: &mut egui::Ui, machine: &Machine, tr: &Translator) {
        match &self.notice {
            Some((text, true)) => {
                ui.colored_label(ui.visuals().error_fg_color, text);
            }
            Some((text, false)) => {
                ui.label(text);
            }
            None => {}
        }
        self.confirm(ui, tr);
        if self.edit.is_none() {
            ui.centered_and_justified(|ui| {
                ui.weak(tr.tr(n_!("Please select or create a device entry to edit")));
            });
            return;
        }
        let action = self.action_bar(ui, machine, tr);
        ui.separator();
        if let Some(edit) = self.edit.as_mut() {
            form(ui, edit, tr);
        }
        match action {
            Some(Action::Save) => self.save(machine.job_running(), tr),
            Some(Action::Cancel) => {
                self.edit = None;
                self.confirm_delete = None;
            }
            Some(Action::Delete) => self.ask_delete(tr),
            Some(Action::Sync) => self.sync_requested = true,
            Some(Action::SetActive) => self.set_active(machine.job_running(), tr),
            None => {}
        }
    }

    fn list(
        &mut self,
        ui: &mut egui::Ui,
        units: gcodekit5_core::units::MeasurementSystem,
        feed_units: gcodekit5_core::units::FeedRateUnits,
        tr: &Translator,
    ) {
        let active = self.active_id();
        let selected = self.edit.as_ref().map(|e| e.id.clone());
        let mut clicked = None;
        for profile in self.profiles().iter().filter(|p| self.listed(p)) {
            let is_selected = selected.as_deref() == Some(profile.id.as_str());
            if ui.selectable_label(is_selected, &profile.name).clicked() {
                clicked = Some(profile.id.clone());
            }
            ui.horizontal(|ui| {
                ui.weak(format!(
                    "{} - {}",
                    tr.tr(device_type_label(&profile.device_type)),
                    profile.controller_type
                ));
                if active.as_deref() == Some(profile.id.as_str()) {
                    ui.colored_label(ui.visuals().selection.bg_fill, tr.tr(n_!("Active")));
                }
            });
            ui.add_space(4.0);
        }
        if let Some(id) = clicked {
            self.select(&id, units, feed_units);
        }
    }

    /// The delete question, while one is asked.
    fn confirm(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        let Some(confirm) = &self.confirm_delete else {
            return;
        };
        let question =
            tr.tr(n_!("Delete '{}'? This cannot be undone."))
                .replacen("{}", &confirm.name, 1);
        let mut answer = None;
        ui.group(|ui| {
            ui.strong(tr.tr(n_!("Delete device?")));
            ui.label(question);
            ui.horizontal(|ui| {
                if ui.button(tr.tr(n_!("Yes"))).clicked() {
                    answer = Some(true);
                }
                if ui.button(tr.tr(n_!("No"))).clicked() {
                    answer = Some(false);
                }
            });
        });
        match answer {
            Some(true) => self.delete(tr),
            Some(false) => self.confirm_delete = None,
            None => {}
        }
    }

    fn action_bar(&self, ui: &mut egui::Ui, machine: &Machine, tr: &Translator) -> Option<Action> {
        let is_active = self
            .edit
            .as_ref()
            .is_some_and(|e| self.active_id().as_deref() == Some(e.id.as_str()));
        let asking = self.confirm_delete.is_some();
        let mut action = None;
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!asking, |ui| {
                if ui.button(tr.tr(n_!("Save"))).clicked() {
                    action = Some(Action::Save);
                }
                if ui.button(tr.tr(n_!("Cancel"))).clicked() {
                    action = Some(Action::Cancel);
                }
                if ui.button(tr.tr(n_!("Delete"))).clicked() {
                    action = Some(Action::Delete);
                }
                let sync = ui
                    .add_enabled(
                        machine.status().connected,
                        egui::Button::new(tr.tr(n_!("Sync from Device"))),
                    )
                    .on_hover_text(tr.tr(n_!("Update device information from connected device")));
                if sync.clicked() {
                    action = Some(Action::Sync);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let set_active =
                        ui.add_enabled(!is_active, egui::Button::new(tr.tr(n_!("Set Active"))));
                    if set_active.clicked() {
                        action = Some(Action::SetActive);
                    }
                });
            });
        });
        action
    }
}

/// A labelled single-line field in a grid row.
fn field(ui: &mut egui::Ui, label: &str, text: &mut String, enabled: bool) {
    let label = ui.label(label);
    ui.add_enabled(enabled, egui::TextEdit::singleline(text))
        .labelled_by(label.id);
}

/// A labelled length or feed field with its unit.
fn unit_field(ui: &mut egui::Ui, label: &str, text: &mut String, unit: &str) {
    field(ui, label, text, true);
    ui.weak(unit);
}

/// The tab switcher and the open tab.
fn form(ui: &mut egui::Ui, edit: &mut Edit, tr: &Translator) {
    ui.horizontal(|ui| {
        for tab in Tab::ALL {
            ui.selectable_value(&mut edit.tab, tab, tr.tr(tab.msgid()));
        }
    });
    ui.separator();
    let unit = get_unit_label(edit.units);
    let feed_unit = edit.feed_units.to_string();
    let f = &mut edit.form;
    egui::ScrollArea::vertical()
        .id_salt("gcodekit5-device-manager-form")
        .auto_shrink([false, false])
        .show(ui, |ui| match edit.tab {
            Tab::General => general(ui, f, tr),
            Tab::Connection => connection(ui, f, tr),
            Tab::Dimensions => dimensions(ui, f, unit, tr),
            Tab::Capabilities => capabilities(ui, f, &feed_unit, tr),
        });
}

fn general(ui: &mut egui::Ui, f: &mut ProfileForm, tr: &Translator) {
    egui::Grid::new("gcodekit5-device-general")
        .num_columns(2)
        .show(ui, |ui| {
            field(ui, &tr.tr(n_!("Name:")), &mut f.name, true);
            ui.end_row();
            field(ui, &tr.tr(n_!("Description:")), &mut f.description, true);
            ui.end_row();
            let label = ui.label(tr.tr(n_!("Device Type:")));
            egui::ComboBox::from_id_salt("gcodekit5-device-type")
                .selected_text(tr.tr(device_type_label(&f.device_type)))
                .show_ui(ui, |ui| {
                    for t in DEVICE_TYPES {
                        let text = tr.tr(device_type_label(&t));
                        ui.selectable_value(&mut f.device_type, t, text);
                    }
                })
                .response
                .labelled_by(label.id);
            ui.end_row();
            let label = ui.label(tr.tr(n_!("Controller:")));
            egui::ComboBox::from_id_salt("gcodekit5-device-controller")
                .selected_text(tr.tr(controller_label(&f.controller_type)))
                .show_ui(ui, |ui| {
                    for (c, text) in CONTROLLER_CHOICES {
                        ui.selectable_value(&mut f.controller_type, c, tr.tr(text));
                    }
                })
                .response
                .labelled_by(label.id);
            ui.end_row();
        });
}

fn connection(ui: &mut egui::Ui, f: &mut ProfileForm, tr: &Translator) {
    let serial = f.is_serial();
    egui::Grid::new("gcodekit5-device-connection")
        .num_columns(2)
        .show(ui, |ui| {
            let label = ui.label(tr.tr(n_!("Connection Type:")));
            egui::ComboBox::from_id_salt("gcodekit5-device-connection-type")
                .selected_text(tr.tr(&f.connection_type))
                .show_ui(ui, |ui| {
                    for c in CONNECTION_TYPES {
                        ui.selectable_value(&mut f.connection_type, c.to_string(), tr.tr(c));
                    }
                })
                .response
                .labelled_by(label.id);
            ui.end_row();
            field(ui, &tr.tr(n_!("Port:")), &mut f.port, serial);
            ui.end_row();
            let label = ui.label(tr.tr(n_!("Baud Rate:")));
            ui.add_enabled_ui(serial, |ui| {
                egui::ComboBox::from_id_salt("gcodekit5-device-baud")
                    .selected_text(f.baud_rate.to_string())
                    .show_ui(ui, |ui| {
                        for rate in BAUD_RATES {
                            ui.selectable_value(&mut f.baud_rate, rate, rate.to_string());
                        }
                    })
                    .response
                    .labelled_by(label.id);
            });
            ui.end_row();
            field(ui, &tr.tr(n_!("TCP Host:")), &mut f.tcp_host, !serial);
            ui.end_row();
            field(ui, &tr.tr(n_!("TCP Port:")), &mut f.tcp_port, !serial);
            ui.end_row();
            field(ui, &tr.tr(n_!("Timeout (ms):")), &mut f.timeout_ms, true);
            ui.end_row();
        });
    ui.checkbox(&mut f.auto_reconnect, tr.tr(n_!("Auto Reconnect")));
}

fn dimensions(ui: &mut egui::Ui, f: &mut ProfileForm, unit: &str, tr: &Translator) {
    egui::Grid::new("gcodekit5-device-dimensions")
        .num_columns(7)
        .show(ui, |ui| {
            let axes = [
                (n_!("X Axis:"), &mut f.x_min, &mut f.x_max),
                (n_!("Y Axis:"), &mut f.y_min, &mut f.y_max),
                (n_!("Z Axis:"), &mut f.z_min, &mut f.z_max),
            ];
            for (axis, min, max) in axes {
                let axis = tr.tr(axis);
                ui.strong(&*axis);
                // Each field is named with its axis, so "Min:" is not ambiguous.
                unit_field(ui, &format!("{axis} {}", tr.tr(n_!("Min:"))), min, unit);
                unit_field(ui, &format!("{axis} {}", tr.tr(n_!("Max:"))), max, unit);
                ui.end_row();
            }
        });
}

fn capabilities(ui: &mut egui::Ui, f: &mut ProfileForm, feed_unit: &str, tr: &Translator) {
    ui.strong(tr.tr(n_!("Limits")));
    egui::Grid::new("gcodekit5-device-limits")
        .num_columns(3)
        .show(ui, |ui| {
            unit_field(
                ui,
                &tr.tr(n_!("Max Feed Rate:")),
                &mut f.max_feed_rate,
                feed_unit,
            );
            ui.end_row();
            field(ui, &tr.tr(n_!("Max S-Value:")), &mut f.max_s_value, true);
            ui.end_row();
        });
    ui.separator();
    ui.strong(tr.tr(n_!("Capabilities")));
    egui::Grid::new("gcodekit5-device-capabilities")
        .num_columns(2)
        .show(ui, |ui| {
            field(ui, &tr.tr(n_!("No of Axes:")), &mut f.num_axes, true);
            ui.end_row();
            ui.checkbox(&mut f.has_spindle, tr.tr(n_!("Has Spindle")));
            ui.end_row();
            let spindle = f.has_spindle;
            field(
                ui,
                &tr.tr(n_!("Spindle Power (W):")),
                &mut f.spindle_watts,
                spindle,
            );
            ui.end_row();
            field(
                ui,
                &tr.tr(n_!("Max Spindle Speed (RPM):")),
                &mut f.max_spindle_speed_rpm,
                spindle,
            );
            ui.end_row();
            ui.checkbox(&mut f.has_laser, tr.tr(n_!("Has Laser")));
            ui.end_row();
            let laser = f.has_laser;
            field(
                ui,
                &tr.tr(n_!("Laser Power (W):")),
                &mut f.laser_watts,
                laser,
            );
            ui.end_row();
        });
    ui.checkbox(&mut f.has_coolant, tr.tr(n_!("Has Coolant")));
    ui.checkbox(
        &mut f.pen_mode,
        tr.tr(n_!("Pen Mode (Repetier: M3/M5 become pen down/up Z moves)")),
    );
}
