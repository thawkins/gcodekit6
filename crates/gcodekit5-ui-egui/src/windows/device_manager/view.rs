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
use crate::icons::Icon;
use crate::machine::Machine;
use crate::theme;
use crate::windows::help::HelpTopic;
use crate::windows::manager_ui::{self, SIDEBAR_WIDTH, icon_text_button};

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
        // GTK: the device list sidebar | the edit form.
        ui.horizontal_top(|ui| {
            let size = egui::vec2(SIDEBAR_WIDTH, ui.available_height());
            ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
                manager_ui::sidebar_header(
                    ui,
                    &tr.tr(n_!("Devices")),
                    HelpTopic::DeviceManager,
                    tr,
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text(tr.tr(n_!("Search devices...")))
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(4.0);
                // Add Device sits under the list, as in GTK.
                let add_height = ui.spacing().interact_size.y + 12.0;
                egui::ScrollArea::vertical()
                    .id_salt("gcodekit5-device-manager-list")
                    .auto_shrink([false, false])
                    .max_height((ui.available_height() - add_height).max(80.0))
                    .show(ui, |ui| self.list(ui, units, feed_units, tr));
                ui.add_space(6.0);
                let add = theme::primary_icon_button(Icon::Add.image(), tr.tr(n_!("Add Device")))
                    .min_size(egui::vec2(ui.available_width(), 0.0));
                if ui.add_enabled(self.store.is_some(), add).clicked() {
                    self.add(units, feed_units, tr);
                }
            });
            ui.separator();
            ui.add_space(12.0);
            ui.vertical(|ui| self.details(ui, machine, tr));
        });
    }

    /// The notice, the delete question, the action bar and the form.
    fn details(&mut self, ui: &mut egui::Ui, machine: &Machine, tr: &Translator) {
        manager_ui::notice(ui, self.notice.as_ref());
        self.confirm(ui, tr);
        if self.edit.is_none() {
            manager_ui::placeholder(
                ui,
                &tr.tr(n_!("Please select or create a device entry to edit")),
            );
            return;
        }
        let action = self.action_bar(ui, machine, tr);
        ui.add_space(8.0);
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
        manager_ui::boxed_list(ui, |ui| {
            let profiles = self.profiles();
            for (index, profile) in profiles.iter().filter(|p| self.listed(p)).enumerate() {
                let is_selected = selected.as_deref() == Some(profile.id.as_str());
                let row = manager_ui::list_row(ui, index == 0, is_selected, &profile.name, |ui| {
                    ui.horizontal(|ui| {
                        ui.weak(format!(
                            "{} - {}",
                            tr.tr(device_type_label(&profile.device_type)),
                            profile.controller_type
                        ));
                        if active.as_deref() == Some(profile.id.as_str()) {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    theme::badge(ui, &tr.tr(n_!("Active")));
                                },
                            );
                        }
                    });
                });
                if row.clicked() {
                    clicked = Some(profile.id.clone());
                }
            }
        });
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
        let choices = [
            tr.tr(n_!("Yes")).into_owned(),
            tr.tr(n_!("No")).into_owned(),
        ];
        let answer = manager_ui::question(ui, &tr.tr(n_!("Delete device?")), &question, &choices)
            .map(|index| index == 0);
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
        // GTK's action bar: Save (suggested), Cancel, Delete (destructive), Sync, and Set
        // Active at the right end.
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!asking, |ui| {
                let save = theme::primary_icon_button(Icon::Save.image(), tr.tr(n_!("Save")));
                if ui.add(save).clicked() {
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
                if ui.add(delete).clicked() {
                    action = Some(Action::Delete);
                }
                let sync = ui
                    .add_enabled(
                        machine.status().connected,
                        icon_text_button(ui, Icon::Refresh, tr.tr(n_!("Sync from Device"))),
                    )
                    .on_hover_text(tr.tr(n_!("Update device information from connected device")));
                if sync.clicked() {
                    action = Some(Action::Sync);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let set_active = ui.add_enabled(
                        !is_active,
                        icon_text_button(ui, Icon::Ok, tr.tr(n_!("Set Active"))),
                    );
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
    let tabs: Vec<_> = Tab::ALL
        .iter()
        .map(|tab| (*tab, tr.tr(tab.msgid()).into_owned()))
        .collect();
    manager_ui::tab_switcher(ui, &mut edit.tab, &tabs);
    ui.add_space(8.0);
    let unit = get_unit_label(edit.units);
    let feed_unit = edit.feed_units.to_string();
    let f = &mut edit.form;
    egui::ScrollArea::vertical()
        .id_salt("gcodekit5-device-manager-form")
        .auto_shrink([false, false])
        .show(ui, |ui| match edit.tab {
            Tab::General => manager_ui::form_group(ui, None, |ui| general(ui, f, tr)),
            Tab::Connection => manager_ui::form_group(ui, None, |ui| connection(ui, f, tr)),
            Tab::Dimensions => manager_ui::form_group(ui, None, |ui| dimensions(ui, f, unit, tr)),
            Tab::Capabilities => capabilities(ui, f, &feed_unit, tr),
        });
}

fn general(ui: &mut egui::Ui, f: &mut ProfileForm, tr: &Translator) {
    egui::Grid::new("gcodekit5-device-general")
        .num_columns(2)
        .spacing([12.0, 8.0])
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
        .spacing([12.0, 8.0])
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
    crate::theme::checkbox(ui, &mut f.auto_reconnect, tr.tr(n_!("Auto Reconnect")));
}

fn dimensions(ui: &mut egui::Ui, f: &mut ProfileForm, unit: &str, tr: &Translator) {
    egui::Grid::new("gcodekit5-device-dimensions")
        .num_columns(7)
        .spacing([12.0, 8.0])
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
    manager_ui::form_group(ui, Some(&tr.tr(n_!("Limits"))), |ui| {
        egui::Grid::new("gcodekit5-device-limits")
            .num_columns(3)
            .spacing([12.0, 8.0])
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
    });
    manager_ui::form_group(ui, Some(&tr.tr(n_!("Capabilities"))), |ui| {
        egui::Grid::new("gcodekit5-device-capabilities")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                field(ui, &tr.tr(n_!("No of Axes:")), &mut f.num_axes, true);
                ui.end_row();
                crate::theme::checkbox(ui, &mut f.has_spindle, tr.tr(n_!("Has Spindle")));
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
                crate::theme::checkbox(ui, &mut f.has_laser, tr.tr(n_!("Has Laser")));
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
        crate::theme::checkbox(ui, &mut f.has_coolant, tr.tr(n_!("Has Coolant")));
        crate::theme::checkbox(
            ui,
            &mut f.pen_mode,
            tr.tr(n_!("Pen Mode (Repetier: M3/M5 become pen down/up Z moves)")),
        );
    });
}
